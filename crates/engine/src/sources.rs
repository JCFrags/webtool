//! Bounded native GitHub source resolution. No cloning or recursive ingestion.
use anyhow::{bail, Context, Result};
use base64::Engine as _;
use futures_util::StreamExt;
use serde_json::{json, Value};
use url::Url;
use webtool_protocol::{Content, Link, Locator, Warning};
use crate::{fetch::Fetched, readers::Parsed};

pub const VERSION: &str = "github-source/2";
const MAX_REF_SPLITS: usize = 8;
const MAX_PATH_DEPTH: usize = 16;

pub struct Details {
    pub filename: String,
    pub metadata: Value,
    pub directory: bool,
    pub readme: bool,
}
pub struct Resolved { pub fetched: Fetched, pub details: Details }
struct Api { bytes: Vec<u8>, status: u16 }
fn endpoint(parts: &[&str]) -> Url {
    let mut u=Url::parse("https://api.github.com/").expect("constant URL");
    u.path_segments_mut().expect("HTTP URL").extend(parts);
    u
}
fn pinned(owner:&str,repo:&str,kind:&str,commit:&str,path:&str)->String {
    let mut u=Url::parse("https://github.com/").expect("constant URL");
    u.path_segments_mut().expect("HTTP URL").extend([owner,repo,kind,commit])
        .extend(path.split('/').filter(|p|!p.is_empty()));
    u.into()
}
async fn api(client:&reqwest::Client,url:&Url,max:usize,missing_ok:bool)->Result<Option<Api>> {
    let response=client.get(url.clone()).header("Accept","application/vnd.github+json")
        .header("X-GitHub-Api-Version","2022-11-28").send().await.context("fetch GitHub API")?;
    let status=response.status().as_u16();
    if missing_ok && matches!(status,404|422) { return Ok(None); }
    if status==429 || (status==403 && response.headers().get("x-ratelimit-remaining").is_some_and(|v|v=="0")) {
        bail!("github_rate_limited: GitHub returned HTTP {status}; retry after the public API rate limit resets");
    }
    if matches!(status,401|403) { bail!("github_access_denied: GitHub returned HTTP {status}; public unauthenticated access is unavailable"); }
    if status==404 { bail!("github_content_unavailable: GitHub returned HTTP 404 for a pinned object"); }
    if !(200..300).contains(&status) { bail!("github_api_error: GitHub returned HTTP {status}"); }
    if response.content_length().is_some_and(|size|size>max as u64) { bail!("GitHub API response exceeds configured byte limit"); }
    let mut stream=response.bytes_stream(); let mut bytes=Vec::new();
    while let Some(chunk)=stream.next().await {
        let chunk=chunk.context("read GitHub API response")?;
        if bytes.len().saturating_add(chunk.len())>max { bail!("GitHub API response exceeds configured byte limit"); }
        bytes.extend_from_slice(&chunk);
    }
    Ok(Some(Api{bytes,status}))
}
fn value(api:&Api)->Result<Value> { serde_json::from_slice(&api.bytes).context("github_invalid_response: expected API JSON") }
fn field<'a>(value:&'a Value,key:&str)->Result<&'a str> {
    value.get(key).and_then(Value::as_str).with_context(||format!("github_invalid_response: missing {key}"))
}
fn decode(part:&str)->Result<String> {
    let mut out=Vec::new(); let mut input=part.bytes();
    while let Some(b)=input.next() {
        if b==b'%' {
            let hi=input.next().and_then(|v|(v as char).to_digit(16)).context("invalid encoded GitHub path")?;
            let lo=input.next().and_then(|v|(v as char).to_digit(16)).context("invalid encoded GitHub path")?;
            out.push((hi*16+lo) as u8);
        } else { out.push(b); }
    }
    let result=String::from_utf8(out).context("GitHub path must be UTF-8")?;
    if result.contains('\0') { bail!("invalid NUL in GitHub path"); }
    Ok(result)
}
fn file_path(parts:&[String])->Result<String> {
    let path=parts.join("/");
    if path.split('/').any(|p|p=="."||p=="..") { bail!("unsupported dot segments in GitHub source path"); }
    if path.split('/').filter(|p|!p.is_empty()).count()>MAX_PATH_DEPTH {
        bail!("github_path_limit: at most {MAX_PATH_DEPTH} path components are supported");
    }
    Ok(path)
}
fn commit_fields(v:&Value)->Result<(String,String)> {
    let sha=field(v,"sha")?;
    if sha.len()!=40 || !sha.bytes().all(|b|b.is_ascii_hexdigit()) { bail!("github_invalid_response: commit is not an immutable SHA"); }
    let tree=v.pointer("/commit/tree/sha").and_then(Value::as_str).context("github_content_unavailable: commit has no root tree")?;
    Ok((sha.into(),tree.into()))
}

pub async fn github(client:&reqwest::Client,url:&Url,max:usize)->Result<Option<Resolved>> {
    if url.host_str()!=Some("github.com") { return Ok(None); }
    let raw:Vec<_>=url.path_segments().into_iter().flatten().filter(|s|!s.is_empty()).collect();
    if raw.len()<2 { return Ok(None); }
    let parts=raw.iter().map(|p|decode(p)).collect::<Result<Vec<_>>>()?;
    let owner=&parts[0]; let repo=parts[1].trim_end_matches(".git");
    if owner.is_empty() || repo.is_empty() || owner.contains('/') || repo.contains('/') { bail!("invalid GitHub repository URL"); }
    let root=parts.len()==2;
    if !root && !matches!(parts[2].as_str(),"blob"|"tree") { return Ok(None); }
    let directory=!root && parts[2]=="tree";
    let (requested_ref,commit,mut path,tree)=if root {
        let info=api(client,&endpoint(&["repos",owner,repo]),max,true).await?
            .context("github_repository_unavailable: public repository not found or unavailable")?;
        let info=value(&info)?; let branch=field(&info,"default_branch")?;
        let resolved=api(client,&endpoint(&["repos",owner,repo,"commits",branch]),max,true).await?
            .context("github_reference_not_found: repository default branch has no readable commit")?;
        let (sha,tree)=commit_fields(&value(&resolved)?)?;
        (branch.to_owned(),sha,String::new(),tree)
    } else {
        let tail=&parts[3..];
        if tail.is_empty() || (!directory && tail.len()<2) { bail!("github_path_missing: URL needs a reference and file/directory path"); }
        let explicit=tail[0].contains('/') || (tail[0].len()==40 && tail[0].bytes().all(|b|b.is_ascii_hexdigit()));
        let count=if explicit {1} else {tail.len()-usize::from(!directory)};
        if count>MAX_REF_SPLITS { bail!("github_reference_limit: too many possible ref/path splits; use a full commit SHA or percent-encode slashes in the reference"); }
        let mut candidates=Vec::new();
        // All bounded splits are checked. Never silently pick a different branch
        // when two refs can describe the same web URL. Explicit SHA/encoded ref wins.
        for n in 1..=count {
            let reference=tail[..n].join("/");
            if let Some(response)=api(client,&endpoint(&["repos",owner,repo,"commits",&reference]),max,true).await? {
                let (sha,tree)=commit_fields(&value(&response)?)?;
                candidates.push((reference,sha,file_path(&tail[n..])?,tree));
            }
        }
        if candidates.len()>1 { bail!("github_reference_ambiguous: multiple ref/path splits resolve; use a full commit SHA or percent-encode reference slashes"); }
        candidates.pop().context("github_reference_not_found: no bounded reference candidate resolves; no branch fallback was used")?
    };
    if root {
        let mut u=endpoint(&["repos",owner,repo,"readme"]); u.query_pairs_mut().append_pair("ref",&commit);
        let response=api(client,&u,max,true).await?.context("github_readme_unavailable: no README at the resolved commit")?;
        path=field(&value(&response)?,"path")?.to_owned();
        file_path(&[path.clone()])?;
    }
    let mut warnings=Vec::new();
    let mut current=api(client,&endpoint(&["repos",owner,repo,"git","trees",&tree]),max,false).await?.context("missing root tree")?;
    let components:Vec<_>=path.split('/').filter(|p|!p.is_empty()).collect();
    let mut blob=None;
    for (index,component) in components.iter().enumerate() {
        let listing=value(&current)?;
        let truncated=listing["truncated"].as_bool().unwrap_or(true);
        let entries=listing["tree"].as_array().context("github_invalid_response: tree entries unavailable")?;
        let Some(entry)=entries.iter().find(|e|e["path"].as_str()==Some(component)) else {
            if truncated { bail!("github_listing_incomplete: cannot establish whether the requested path exists in a truncated tree"); }
            bail!("github_path_not_found: {path} is absent at commit {commit}");
        };
        if truncated { warnings.push(Warning::new("github_listing_incomplete","An ancestor tree was truncated; the requested entry was present, but its siblings were not completely listed.")); }
        let kind=field(entry,"type")?; let mode=field(entry,"mode")?;
        if kind=="commit" || mode=="120000" { bail!("github_unsupported_object: submodules and symlinks are not followed"); }
        let last=index+1==components.len();
        if kind=="tree" && (!last || directory) {
            current=api(client,&endpoint(&["repos",owner,repo,"git","trees",field(entry,"sha")?]),max,false).await?.context("missing tree")?;
        } else if kind=="blob" && last && !directory && matches!(mode,"100644"|"100755") {
            blob=Some(field(entry,"sha")?.to_owned());
        } else { bail!("github_unsupported_object: requested URL kind/path does not identify a supported {}",if directory{"directory"}else{"file"}); }
    }
    let resolved=pinned(owner,repo,if directory{"tree"}else{"blob"},&commit,&path);
    let metadata=json!({"repository":format!("{owner}/{repo}"),"owner":owner,"repo":repo,"path":path,
        "requested_ref":requested_ref,"resolved_commit":commit,"resolver":VERSION,
        "kind":if directory{"directory"}else if root{"readme"}else{"file"}});
    let (bytes,status,role,content_type)=if directory {
        (current.bytes,current.status,"repository_directory",Some("application/json".into()))
    } else {
        let sha=blob.context("github_content_unavailable: URL did not resolve to a file blob")?;
        let response=api(client,&endpoint(&["repos",owner,repo,"git","blobs",&sha]),max,false).await?.context("missing blob")?;
        let object=value(&response)?;
        if field(&object,"sha")?!=sha || field(&object,"encoding")?!="base64" { bail!("github_content_unavailable: blob identity or encoding is unavailable"); }
        let bytes=base64::engine::general_purpose::STANDARD.decode(field(&object,"content")?.bytes().filter(|b|!b.is_ascii_whitespace()).collect::<Vec<_>>())
            .context("github_invalid_response: malformed base64 file content")?;
        if bytes.len()>max { bail!("GitHub file exceeds configured byte limit"); }
        if object["size"].as_u64()!=Some(bytes.len() as u64) { bail!("github_content_unavailable: incomplete file blob"); }
        if root { warnings.push(Warning::new("repository_readme_only","Only the revision-pinned README was fetched, not the complete repository.")); }
        (bytes,response.status,if root{"repository_readme"}else{"repository_file"},None)
    };
    Ok(Some(Resolved { fetched:Fetched{bytes,resolved,content_type,status:Some(status),version:Some(commit),role:role.into(),warnings},
        details:Details{filename:path,metadata,directory,readme:root} }))
}

pub fn directory(bytes:&[u8],details:&Details)->Result<Parsed> {
    let source:Value=serde_json::from_slice(bytes)?;
    let entries=source["tree"].as_array().context("github_invalid_response: directory has no entry array")?;
    let m=&details.metadata;
    let mut p=Parsed::new(&format!("{} / {}",m["repository"].as_str().unwrap_or(""),details.filename),"github-directory/2");
    p.push(Content::Paragraph{text:format!("Immediate directory listing at {}. Child file contents were not read.",field(m,"resolved_commit")?)},Locator::Derived{index:1});
    for entry in entries {
        let name=field(entry,"path")?;
        let path=if details.filename.is_empty(){name.into()}else{format!("{}/{name}",details.filename)};
        let kind=field(entry,"type")?; let mode=field(entry,"mode")?;
        let label=if mode=="120000"{"symlink (unsupported)"}else if kind=="commit"{"submodule (unsupported)"}else{kind};
        let link=pinned(field(m,"owner")?,field(m,"repo")?,if kind=="tree"{"tree"}else{"blob"},field(m,"resolved_commit")?,&path);
        p.push(Content::Paragraph{text:format!("{label}: {name}\n{link}")},Locator::Derived{index:p.blocks.len()+1});
        p.links.push(Link{url:link,text:format!("{label}: {name}")});
    }
    if entries.is_empty() { p.push(Content::Paragraph{text:"The API returned an empty directory.".into()},Locator::Derived{index:p.blocks.len()+1}); }
    p.warnings.push(Warning::new("repository_directory_only","This derived listing describes immediate entries only. It does not read their contents."));
    if source["truncated"].as_bool()!=Some(false) { p.warnings.push(Warning::new("github_listing_incomplete","GitHub returned a truncated tree or did not establish listing completeness.")); }
    Ok(p)
}

/// Supplement, not rewrite, the retained README text. This intentionally is not
/// a new Markdown parser. Inline destinations and reference definitions are linked.
pub fn readme_links(bytes:&[u8],details:&Details)->Vec<Link> {
    let Ok(text)=std::str::from_utf8(bytes) else{return Vec::new()};
    let pattern=regex::Regex::new(r"(?m)\[([^\]\n]*)\]\(([^)\s]+)\)|^\[([^\]\n]+)\]:\s*(\S+)").expect("constant regex");
    let m=&details.metadata; let (Some(owner),Some(repo),Some(commit))=(m["owner"].as_str(),m["repo"].as_str(),m["resolved_commit"].as_str())else{return Vec::new()};
    let mut links=Vec::new();
    for capture in pattern.captures_iter(text) {
        let label=capture.get(1).or_else(||capture.get(3)).map(|m|m.as_str()).unwrap_or("");
        let target=capture.get(2).or_else(||capture.get(4)).map(|m|m.as_str()).unwrap_or("").trim_matches(['<','>']);
        let url=if let Ok(u)=Url::parse(target) {
            if !matches!(u.scheme(),"http"|"https"){continue;} u.to_string()
        } else if target.starts_with("//") { format!("https:{target}") }
        else {
            // Resolve against a synthetic repository root first, so relative links
            // cannot erase the immutable commit component in the public URL.
            let mut base=Url::parse("https://repository.invalid/").expect("constant URL");
            base.path_segments_mut().expect("HTTP URL").extend(details.filename.split('/'));
            let Ok(relative)=base.join(target)else{continue};
            let Ok(path)=decode(relative.path().trim_start_matches('/'))else{continue};
            let mut u=Url::parse(&pinned(owner,repo,if path.ends_with('/'){"tree"}else{"blob"},commit,&path)).expect("constructed URL");
            u.set_fragment(relative.fragment());u.set_query(relative.query());u.to_string()
        };
        if !links.iter().any(|l:&Link|l.url==url) { links.push(Link{url,text:label.into()}); }
    }
    links
}
