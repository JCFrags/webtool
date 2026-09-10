use anyhow::{Context,Result};
use base64::Engine as _;
use serde_json::Value;
use url::Url;
use webtool_protocol::Warning;
use crate::fetch::{self,Fetched};

/// A repository root resolves to one revision-pinned README, never the whole repository.
pub async fn github_readme(client:&reqwest::Client,url:&Url,max:usize)->Result<Option<Fetched>>{
    if url.host_str()!=Some("github.com"){return Ok(None);}
    let segments:Vec<_>=url.path_segments().into_iter().flatten().filter(|s|!s.is_empty()).collect();
    if segments.len()!=2{return Ok(None);}
    let (owner,repo)=(segments[0],segments[1].trim_end_matches(".git"));
    let info=fetch::http(client,&format!("https://api.github.com/repos/{owner}/{repo}"),max).await?;
    let info:Value=serde_json::from_slice(&info.bytes)?;
    let branch=info["default_branch"].as_str().context("repository has no default branch")?;
    let mut commit_url=Url::parse(&format!("https://api.github.com/repos/{owner}/{repo}/commits"))?;
    commit_url.query_pairs_mut().append_pair("sha",branch).append_pair("per_page","1");
    let commits=fetch::http(client,commit_url.as_str(),max).await?;
    let commits:Value=serde_json::from_slice(&commits.bytes)?;
    let sha=commits[0]["sha"].as_str().context("repository has no readable commit")?.to_string();
    let mut readme_url=Url::parse(&format!("https://api.github.com/repos/{owner}/{repo}/readme"))?;
    readme_url.query_pairs_mut().append_pair("ref",&sha);
    let readme=fetch::http(client,readme_url.as_str(),max).await?;
    let status=readme.status;
    let readme:Value=serde_json::from_slice(&readme.bytes)?;
    let encoded=readme["content"].as_str().context("README content unavailable")?;
    let bytes=base64::engine::general_purpose::STANDARD.decode(encoded.chars().filter(|c|!c.is_whitespace()).collect::<String>())?;
    let path=readme["path"].as_str().unwrap_or("README.md");
    Ok(Some(Fetched{bytes,resolved:format!("https://github.com/{owner}/{repo}/blob/{sha}/{path}"),
        content_type:Some(if path.ends_with(".md") || path.ends_with(".markdown") {"text/markdown"} else {"text/plain"}.into()),status,version:Some(sha),role:"repository_readme".into(),
        warnings:vec![Warning::new("repository_readme_only","Only the revision-pinned README was fetched, not the complete repository.")]}))
}
