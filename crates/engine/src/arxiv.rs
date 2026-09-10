//! Official arXiv metadata, immutable PDF identity, and offline preprint citations.
use std::{sync::LazyLock,time::Duration};
use anyhow::{bail,Context,Result};
use chrono::{DateTime,Datelike};
use regex::Regex;
use roxmltree::Node;
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use tokio::{sync::{Mutex,MutexGuard},time::Instant};
use url::Url;
use crate::fetch::{self,Fetched};

pub const VERSION:&str="arxiv-paper/1";
const ATOM:&str="http://www.w3.org/2005/Atom";
const ARXIV:&str="http://arxiv.org/schemas/atom";
static IDS:LazyLock<Regex>=LazyLock::new(||Regex::new(r"^(?P<id>(?:[0-9]{2}(?:0[1-9]|1[0-2])\.[0-9]{4,5}|[a-z][a-z-]*(?:\.[A-Z]{2})?/[0-9]{2}(?:0[1-9]|1[0-2])[0-9]{3}))(?:v(?P<version>[1-9][0-9]{0,8}))?$").expect("constant regex"));
static GATE:LazyLock<Mutex<Instant>>=LazyLock::new(||Mutex::new(Instant::now()));
/// One shared connection, with three seconds after completion/cancellation before
/// the next request. This is deliberately more conservative than start spacing.
pub struct Slot(MutexGuard<'static,Instant>);
impl Drop for Slot{fn drop(&mut self){*self.0=Instant::now()+Duration::from_secs(3);}}
pub async fn slot()->Slot{
    let next=GATE.lock().await;tokio::time::sleep_until(*next).await;Slot(next)
}
pub fn host(url:&Url)->bool{url.host_str().is_some_and(|h|h=="arxiv.org"||h.ends_with(".arxiv.org"))}
#[derive(Debug,Clone)]pub struct Identity{pub id:String,pub version:Option<String>}
impl Identity{pub fn full(&self)->String{format!("{}{}",self.id,self.version.as_ref().map(|v|format!("v{v}")).unwrap_or_default())}}
pub fn identify(url:&Url)->Result<Option<Identity>>{
    if !matches!(url.host_str(),Some("arxiv.org"|"www.arxiv.org"|"export.arxiv.org")){return Ok(None);}
    let Some(path)=url.path().strip_prefix("/abs/").or_else(||url.path().strip_prefix("/pdf/")) else{return Ok(None)};
    let path=path.strip_suffix(".pdf").unwrap_or(path);
    let c=IDS.captures(path).context("arxiv_invalid_identifier: expected a modern or legacy arXiv ID with optional vN")?;
    Ok(Some(Identity{id:c["id"].into(),version:c.name("version").map(|v|v.as_str().into())}))
}
#[derive(Debug,Serialize,Deserialize)]pub struct Paper{
    pub resolver:String,pub id:String,pub version:String,pub versioned_id:String,pub requested_id:String,
    pub title:String,pub authors:Vec<String>,pub abstract_text:String,pub categories:Vec<String>,
    pub primary_category:Option<String>,pub published:String,pub updated:String,
    pub doi:Option<String>,pub journal_reference:Option<String>,pub abstract_url:String,pub pdf_url:String,
}
fn field(node:Node<'_,'_>,ns:&str,name:&str)->Option<String>{
    node.children().find(|n|n.has_tag_name((ns,name))).map(|n|n.descendants().filter(|n|n.is_text()).filter_map(|n|n.text()).collect::<String>().trim().to_owned()).filter(|s|!s.is_empty())
}
fn required(node:Node<'_,'_>,name:&str)->Result<String>{field(node,ATOM,name).with_context(||format!("arxiv_invalid_metadata: missing {name}"))}
async fn get(client:&reqwest::Client,url:&str,max:usize,phase:&str)->Result<Fetched>{
    fetch::http(client,url,max).await.map_err(|e|{
        let message=format!("{e:#}");
        let code=if message.contains("HTTP 429"){"arxiv_rate_limited"}
            else if message.contains("HTTP 404")||message.contains("HTTP 410"){"arxiv_version_unavailable"}
            else if phase=="PDF"{"arxiv_pdf_failed"}else{"arxiv_api_failed"};
        anyhow::anyhow!("{code}: {phase}: {message}")
    })
}
pub async fn resolve(client:&reqwest::Client,wanted:&Identity,max:usize)->Result<(Paper,Fetched)>{
    let mut url=Url::parse("https://export.arxiv.org/api/query")?;
    url.query_pairs_mut().append_pair("id_list",&wanted.full()).append_pair("max_results","1");
    let response=get(client,url.as_str(),max,"API").await?;
    let xml=std::str::from_utf8(&response.bytes).context("arxiv_invalid_metadata: API is not UTF-8")?;
    let tree=roxmltree::Document::parse(xml).context("arxiv_invalid_metadata: API is not valid XML")?;
    if !tree.root_element().has_tag_name((ATOM,"feed")){bail!("arxiv_invalid_metadata: expected Atom feed");}
    let entries:Vec<_>=tree.root_element().children().filter(|n|n.has_tag_name((ATOM,"entry"))).collect();
    if entries.is_empty(){
        if wanted.version.is_some(){bail!("arxiv_version_unavailable: API returned no entry for {}",wanted.full());}
        bail!("arxiv_empty_result: API returned no entry for {}",wanted.id);
    }
    if entries.len()!=1{bail!("arxiv_identity_mismatch: expected exactly one entry");}
    let entry=entries[0];let entry_id=required(entry,"id")?;
    let returned=identify(&fetch::validated_url(&entry_id)?)?.with_context(||format!("arxiv_api_failed: unexpected entry identity {entry_id}; {}",field(entry,ATOM,"summary").unwrap_or_default()))?;
    if returned.id!=wanted.id{bail!("arxiv_identity_mismatch: requested {}, API returned {}",wanted.full(),returned.full());}
    let version=returned.version.as_ref().context("arxiv_identity_mismatch: API did not return an explicit version")?;
    if wanted.version.as_ref().is_some_and(|v|v!=version){bail!("arxiv_version_unavailable: requested {}, API returned {}; latest was not substituted",wanted.full(),returned.full());}
    let authors:Vec<_>=entry.children().filter(|n|n.has_tag_name((ATOM,"author"))).map(|n|required(n,"name")).collect::<Result<_>>()?;
    if authors.is_empty(){bail!("arxiv_invalid_metadata: missing authors");}
    let full=returned.full();
    let paper=Paper{resolver:VERSION.into(),id:returned.id.clone(),version:version.clone(),versioned_id:full.clone(),requested_id:wanted.full(),
        title:required(entry,"title")?.split_whitespace().collect::<Vec<_>>().join(" "),authors,
        abstract_text:required(entry,"summary")?,categories:entry.children().filter(|n|n.has_tag_name((ATOM,"category"))).filter_map(|n|n.attribute("term")).map(str::to_owned).collect(),
        primary_category:entry.children().find(|n|n.has_tag_name((ARXIV,"primary_category"))).and_then(|n|n.attribute("term")).map(str::to_owned),
        published:required(entry,"published")?,updated:required(entry,"updated")?,doi:field(entry,ARXIV,"doi"),journal_reference:field(entry,ARXIV,"journal_ref"),
        abstract_url:format!("https://arxiv.org/abs/{full}"),pdf_url:format!("https://arxiv.org/pdf/{full}")};
    Ok((paper,response))
}
pub async fn pdf(client:&reqwest::Client,paper:&Paper,max:usize)->Result<Fetched>{
    let mut fetched=get(client,&paper.pdf_url,max,"PDF").await?;
    let returned=identify(&fetch::validated_url(&fetched.resolved)?)?.context("arxiv_identity_mismatch: PDF redirected outside supported paper URLs")?;
    if returned.full()!=paper.versioned_id || !Url::parse(&fetched.resolved)?.path().starts_with("/pdf/"){
        bail!("arxiv_identity_mismatch: PDF destination is not the requested pinned version");
    }
    if !fetched.bytes.starts_with(b"%PDF-"){bail!("arxiv_pdf_failed: versioned endpoint did not return PDF bytes");}
    fetched.content_type=Some("application/pdf".into());fetched.role="arxiv_pdf".into();fetched.version=Some(paper.versioned_id.clone());Ok(fetched)
}
fn bib(value:&str)->String{
    let mut out=String::new();
    for c in value.chars(){match c{
        '\\'=>out.push_str("\\textbackslash{}"),'{'=>out.push_str("\\{"),'}'=>out.push_str("\\}"),
        '$'|'&'|'#'|'%'|'_'=>{out.push('\\');out.push(c);},'~'=>out.push_str("\\textasciitilde{}"),'^'=>out.push_str("\\textasciicircum{}"),
        c if c.is_whitespace()=>out.push(' '),c=>out.push(c),
    }}out
}
pub fn citation(document:&webtool_protocol::Document,format:&str)->Result<Value>{
    let metadata=document.metadata.get("arxiv").context("citation_metadata_unavailable: saved document has no supported paper metadata")?;
    let p:Paper=serde_json::from_value(metadata.clone()).context("citation_metadata_unavailable: invalid saved arXiv metadata")?;
    let date=DateTime::parse_from_rfc3339(&p.updated).ok();
    let text=match format{
        "csl"=>{
            let mut csl=json!({"id":p.versioned_id,"type":"article","title":p.title,
                "author":p.authors.iter().map(|n|json!({"literal":n})).collect::<Vec<_>>(),
                "abstract":p.abstract_text,"URL":p.abstract_url,"archive":"arXiv","archive_location":p.versioned_id,"version":p.version,
                "note":format!("arXiv preprint {}. Not substituted with an associated journal publication.",p.versioned_id)});
            if let Some(d)=date{csl["issued"]=json!({"date-parts":[[d.year(),d.month(),d.day()]]});}
            serde_json::to_string_pretty(&csl)?
        },
        "bibtex"=>{
            let key=p.versioned_id.replace(['.','/'],"_");
            let mut fields=vec![format!("title = {{{}}}",bib(&p.title)),format!("author = {{{}}}",p.authors.iter().map(|n|format!("{{{}}}",bib(n))).collect::<Vec<_>>().join(" and ")),
                format!("eprint = {{{}}}",bib(&p.versioned_id)),"archivePrefix = {arXiv}".into(),format!("version = {{{}}}",bib(&p.version)),format!("url = {{{}}}",bib(&p.abstract_url)),
                format!("note = {{arXiv preprint {}}}",bib(&p.versioned_id))];
            if let Some(d)=date{fields.push(format!("year = {{{}}}",d.year()));}
            if let Some(category)=&p.primary_category{fields.push(format!("primaryClass = {{{}}}",bib(category)));}
            format!("@misc{{arxiv_{key},\n  {}\n}}",fields.join(",\n  "))
        },_=>bail!("citation_format_unsupported: saved arXiv papers support bibtex or csl; DOI RIS behavior is unchanged"),
    };
    Ok(json!({"document_id":document.id,"format":format,"source":p.abstract_url,"version":p.versioned_id,"metadata_source":"retained arXiv API metadata","text":text}))
}
