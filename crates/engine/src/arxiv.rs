//! Official arXiv metadata, immutable PDF identity, and offline preprint citations.
use std::{sync::LazyLock,time::Duration};
use crate::error::ErrorKind;
use anyhow::{bail,Context,Result};
use chrono::{DateTime,Datelike,NaiveDateTime};
use regex::Regex;
use scraper::{Html,Selector,ElementRef};
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use tokio::{sync::{Mutex,MutexGuard},time::Instant};
use url::Url;
use crate::fetch::{self,Fetched};

pub const VERSION:&str="arxiv-abstract-html/3";
static IDS:LazyLock<Regex>=LazyLock::new(||Regex::new(r"^(?P<id>(?:[0-9]{2}(?:0[1-9]|1[0-2])\.[0-9]{4,5}|[a-z][a-z-]*(?:\.[A-Z]{2})?/[0-9]{2}(?:0[1-9]|1[0-2])[0-9]{3}))(?:v(?P<version>[1-9][0-9]{0,8}))?$").expect("constant regex"));
static GATE:LazyLock<Mutex<Instant>>=LazyLock::new(||Mutex::new(Instant::now()));
/// One shared connection, with three seconds after completion/cancellation before
/// the next request. This is deliberately more conservative than start spacing.
pub struct Slot(MutexGuard<'static,Instant>);
impl Drop for Slot{fn drop(&mut self){*self.0=(*self.0).max(Instant::now()+Duration::from_secs(3));}}
impl Slot{pub fn cooldown(&mut self,delay:Duration){*self.0=(*self.0).max(Instant::now()+delay);}}
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
    let c=IDS.captures(path).context(ErrorKind::ArxivInvalidIdentifier.context("arxiv_invalid_identifier: expected a modern or legacy arXiv ID with optional vN"))?;
    Ok(Some(Identity{id:c["id"].into(),version:c.name("version").map(|v|v.as_str().into())}))
}
#[derive(Debug,Serialize,Deserialize)]pub struct Paper{
    pub resolver:String,pub id:String,pub version:String,pub versioned_id:String,pub requested_id:String,
    pub title:String,pub authors:Vec<String>,pub abstract_text:String,pub categories:Vec<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub primary_category:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub published:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub updated:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub doi:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub arxiv_doi:Option<String>,
    #[serde(default,skip_serializing_if="Option::is_none")] pub journal_reference:Option<String>,
    #[serde(default)] pub source_dates:std::collections::BTreeMap<String,String>,
    #[serde(default)] pub citation_authors:Vec<String>,
    #[serde(default)] pub metadata_origin:String,
    /// License links for the selected version, not an associated journal paper.
    #[serde(default)] pub license_evidence:Vec<String>,
    pub abstract_url:String,pub pdf_url:String,
}
fn selector(css:&str)->Selector{Selector::parse(css).expect("constant selector")}
fn text(e:ElementRef<'_>)->String{e.text().collect::<String>().trim().to_owned()}
fn one(doc:&Html,css:&str)->Result<Option<String>>{
    let values:Vec<_>=doc.select(&selector(css)).map(text).filter(|s|!s.is_empty()).collect();
    if values.len()>1{bail!(ErrorKind::ArxivInvalidMetadata.context(format!("arxiv_invalid_metadata: ambiguous {css}")));}Ok(values.into_iter().next())
}
fn meta(doc:&Html,name:&str)->Result<Option<String>>{
    let values:Vec<_>=doc.select(&selector("meta[name][content]")).filter(|e|e.value().attr("name")==Some(name))
        .filter_map(|e|e.value().attr("content")).map(str::trim).filter(|s|!s.is_empty()).collect();
    if values.windows(2).any(|w|w[0]!=w[1]){bail!(ErrorKind::ArxivInvalidMetadata.context(format!("arxiv_invalid_metadata: conflicting {name}")));}
    Ok(values.first().map(|s|s.to_string()))
}
fn id_from(url:&Url,value:&str)->Result<Identity>{
    identify(&url.join(value)? )?.context(ErrorKind::ArxivIdentityMismatch.context("arxiv_identity_mismatch: not an arXiv paper URL"))
}
fn same(selected:&Identity,other:&Identity)->Result<()>{
    if selected.id!=other.id || other.version.as_ref().is_some_and(|v|Some(v)!=selected.version.as_ref()){
        bail!(ErrorKind::ArxivIdentityMismatch.context(format!("arxiv_identity_mismatch: selected {}, conflicting {}",selected.full(),other.full())));
    }Ok(())
}
async fn get(client:&reqwest::Client,url:&str,max:usize,phase:&str)->Result<Fetched>{
    fetch::http(client,url,max).await.map_err(|e|{
        let kind=match crate::error::kind(&e) {
            Some(ErrorKind::SourceHttpStatus(429))=>ErrorKind::ArxivRateLimited,
            Some(ErrorKind::SourceHttpStatus(404|410))=>ErrorKind::ArxivVersionUnavailable,
            Some(ErrorKind::SourceHttpStatus(_)|ErrorKind::SourceRequestFailed|ErrorKind::SizeLimit)=>
                if phase=="PDF"{ErrorKind::ArxivPdfFailed}else{ErrorKind::ArxivInvalidMetadata},
            // Unknown faults are not promoted to a provider or validation error.
            _=>return e,
        };
        kind.with_source(e).into()
    })
}
pub async fn resolve(client:&reqwest::Client,wanted:&Identity,max:usize)->Result<(Paper,Fetched)>{
    let url=format!("https://arxiv.org/abs/{}",wanted.full());
    let response=get(client,&url,max,"abstract HTML").await?;
    let paper=parse_html(&response.bytes,&response.resolved,wanted)?;
    Ok((paper,response))
}
pub(crate) fn parse_html(bytes:&[u8],resolved:&str,wanted:&Identity)->Result<Paper>{
    let source=std::str::from_utf8(bytes).context(ErrorKind::ArxivInvalidMetadata.context("arxiv_invalid_metadata: HTML is not UTF-8"))?;
    let doc=Html::parse_document(source);let base=fetch::validated_url(resolved)?;
    // This article-specific row says "for this version". Neither a canonical
    // link nor an arbitrary occurrence in submission history selects a version.
    let rows:Vec<_>=doc.select(&selector("#abs .arxividv a[href]")).collect();
    if rows.len()!=1{bail!(ErrorKind::ArxivInvalidMetadata.context(format!("arxiv_invalid_metadata: missing or ambiguous selected-version row")));}
    let selected=id_from(&base,rows[0].value().attr("href").unwrap())?;
    let version=selected.version.clone().context(ErrorKind::ArxivInvalidMetadata.context("arxiv_invalid_metadata: selected version is not explicit"))?;
    same(&selected,wanted)?;same(&selected,&id_from(&base,resolved)?)?;
    for e in doc.select(&selector("link[rel=canonical][href], #abs .arxivid a[href], .full-text a.download-pdf[href], .header-breadcrumbs-mobile strong")){
        let evidence=if let Some(href)=e.value().attr("href"){id_from(&base,href)?}
            else{id_from(&base,&format!("/abs/{}",text(e).trim_start_matches("arXiv:")))?};
        same(&selected,&evidence)?;
    }
    for name in ["citation_arxiv_id","citation_pdf_url"]{
        if let Some(v)=meta(&doc,name)?{
            let v=if name=="citation_arxiv_id"{format!("/abs/{}",v.trim_start_matches("arXiv:"))}else{v};
            same(&selected,&id_from(&base,&v)?)?;
        }
    }
    // The selected history marker is the sole unlinked [vN]. Read only its
    // adjacent timestamp, never the most recent timestamp or the entire history.
    let markers:Vec<_>=doc.select(&selector(".submission-history strong")).filter(|e|e.select(&selector("a")).next().is_none() && text(*e).starts_with("[v")).collect();
    if markers.len()!=1 || text(markers[0])!=format!("[v{version}]"){
        bail!(ErrorKind::ArxivIdentityMismatch.context(format!("arxiv_identity_mismatch: history selection contradicts selected-version row")));
    }
    let mut date_text=String::new();
    for n in markers[0].next_siblings(){
        if let Some(e)=ElementRef::wrap(n){if e.value().name()=="br"{break;}bail!(ErrorKind::ArxivInvalidMetadata.context(format!("arxiv_invalid_metadata: unexpected selected date markup")));}
        if let Some(t)=n.value().as_text(){date_text.push_str(t);}
    }
    let date_text=date_text.trim().split(" UTC").next().filter(|_|date_text.contains(" UTC"))
        .context(ErrorKind::ArxivInvalidMetadata.context("arxiv_invalid_metadata: selected submission date has no UTC marker"))?;
    let date=NaiveDateTime::parse_from_str(date_text,"%a, %e %b %Y %H:%M:%S")
        .context(ErrorKind::ArxivInvalidMetadata.context("arxiv_invalid_metadata: selected submission date is invalid"))?.and_utc().to_rfc3339();
    let mut source_dates=std::collections::BTreeMap::from([("selected_submission".into(),date.clone())]);
    for name in ["citation_date","citation_online_date"]{if let Some(v)=meta(&doc,name)?{source_dates.insert(name.into(),v);}}
    if let Some(v)=one(&doc,"#abs .dateline")?{source_dates.insert("dateline".into(),v);}
    let title=meta(&doc,"citation_title")?.context(ErrorKind::ArxivInvalidMetadata.context("arxiv_invalid_metadata: missing citation title"))?;
    let citation_authors=doc.select(&selector("meta[name=citation_author][content]")).filter_map(|e|e.value().attr("content")).map(str::to_owned).collect::<Vec<_>>();
    let mut authors=doc.select(&selector("#abs .authors a")).map(text).filter(|s|!s.is_empty()).collect::<Vec<_>>();
    if authors.is_empty(){authors=citation_authors.clone();}
    if authors.is_empty(){bail!(ErrorKind::ArxivInvalidMetadata.context(format!("arxiv_invalid_metadata: missing authors")));}
    let abstract_text=meta(&doc,"citation_abstract")?.context(ErrorKind::ArxivInvalidMetadata.context("arxiv_invalid_metadata: missing citation abstract"))?;
    let subjects=one(&doc,"#abs .subjects")?.unwrap_or_default();
    let codes=Regex::new(r"\(([a-z][a-z.-]*(?:\.[A-Z]{2})?)\)").expect("constant regex");
    let categories=codes.captures_iter(&subjects).map(|c|c[1].to_owned()).collect();
    let primary_category=one(&doc,"#abs .primary-subject")?.and_then(|v|codes.captures(&v).map(|c|c[1].to_owned()));
    let mut doi=None;let mut arxiv_doi=None;
    let mut dois=vec![];
    if let Some(v)=meta(&doc,"citation_doi")?{dois.push(v);}
    for e in doc.select(&selector("#abs .doi a[href], #abs .arxivdoi a[href]")){dois.push(e.value().attr("href").unwrap().to_owned());}
    for value in dois{
        let value=value.trim().trim_start_matches("https://doi.org/").trim_start_matches("http://doi.org/").to_owned();
        let target=if value.to_ascii_lowercase().starts_with("10.48550/arxiv."){&mut arxiv_doi}else{&mut doi};
        if target.as_ref().is_some_and(|s|s!=&value){bail!(ErrorKind::ArxivInvalidMetadata.context(format!("arxiv_invalid_metadata: conflicting DOI identifiers")));}*target=Some(value);
    }
    let license_evidence=doc.select(&selector(".full-text .abs-license a[href]"))
        .filter_map(|e|e.value().attr("href")).map(str::to_owned).collect();
    let full=selected.full();
    Ok(Paper{resolver:VERSION.into(),id:selected.id,version,versioned_id:full.clone(),requested_id:wanted.full(),title,authors,abstract_text,categories,primary_category,
        published:meta(&doc,"citation_date")?,updated:Some(date),doi,arxiv_doi,journal_reference:one(&doc,"#abs .jref")?,source_dates,citation_authors,
        metadata_origin:"official arXiv abstract-page HTML".into(),license_evidence,abstract_url:format!("https://arxiv.org/abs/{full}"),pdf_url:format!("https://arxiv.org/pdf/{full}")})
}

impl Paper {
    pub fn rights(&self)->webtool_protocol::ScholarlyRights {
        use webtool_protocol::{ReuseDecision,ScholarlyRights};
        let license=if self.license_evidence.len()==1 {self.license_evidence.first().cloned()}else{None};
        let decision=license.as_deref().map(reuse_decision).unwrap_or(ReuseDecision::Unknown);
        ScholarlyRights {decision,license,evidence_url:Some(self.abstract_url.clone()),basis:match decision {
            ReuseDecision::Permitted=>"The selected arXiv version reports a supported CC0, CC BY, or CC BY-SA license. Preserve attribution, license, and applicable share-alike terms.",
            ReuseDecision::NotPermitted=>"The arXiv distribution license does not establish permission for this service to redistribute full text.",
            ReuseDecision::Unknown=>"The selected version has missing, ambiguous, or unsupported reuse terms. Metadata and links only.",
        }.into()}
    }
}
fn reuse_decision(value:&str)->webtool_protocol::ReuseDecision {
    use webtool_protocol::ReuseDecision::*;
    let Ok(url)=Url::parse(value) else{return Unknown;};
    if !matches!(url.scheme(),"http"|"https")||url.query().is_some()||url.fragment().is_some()||!url.username().is_empty()||url.password().is_some(){return Unknown;}
    match (url.host_str(),url.path().trim_end_matches('/')) {
        (Some("creativecommons.org"|"www.creativecommons.org"),"/publicdomain/zero/1.0"|"/licenses/by/3.0"|"/licenses/by/4.0"|"/licenses/by-sa/3.0"|"/licenses/by-sa/4.0")=>Permitted,
        (Some("arxiv.org"|"info.arxiv.org"),path) if path.starts_with("/licenses/")=>NotPermitted,
        _=>Unknown,
    }
}
pub async fn pdf(client:&reqwest::Client,paper:&Paper,max:usize)->Result<Fetched>{
    if paper.rights().decision!=webtool_protocol::ReuseDecision::Permitted {
        bail!(ErrorKind::ArxivReuseNotEstablished.context(format!("arxiv_reuse_not_established: the selected version has no supported full-text reuse basis; use scholarly arXiv inspection for saved metadata and links")));
    }
    let mut fetched=get(client,&paper.pdf_url,max,"PDF").await?;
    let returned=identify(&fetch::validated_url(&fetched.resolved)?)?.context(ErrorKind::ArxivIdentityMismatch.context("arxiv_identity_mismatch: PDF redirected outside supported paper URLs"))?;
    if returned.full()!=paper.versioned_id || !Url::parse(&fetched.resolved)?.path().starts_with("/pdf/"){
        bail!(ErrorKind::ArxivIdentityMismatch.context(format!("arxiv_identity_mismatch: PDF destination is not the requested pinned version")));
    }
    if !fetched.bytes.starts_with(b"%PDF-"){bail!(ErrorKind::ArxivPdfFailed.context(format!("arxiv_pdf_failed: versioned endpoint did not return PDF bytes")));}
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
    let metadata=document.metadata.get("arxiv").context(ErrorKind::CitationMetadataUnavailable.context("citation_metadata_unavailable: saved document has no supported paper metadata"))?;
    let p:Paper=serde_json::from_value(metadata.clone()).context(ErrorKind::CitationMetadataUnavailable.context("citation_metadata_unavailable: invalid saved arXiv metadata"))?;
    let date=p.updated.as_deref().and_then(|d|DateTime::parse_from_rfc3339(d).ok());
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
        },_=>bail!(ErrorKind::CitationMetadataUnavailable.context(format!("citation_format_unsupported: saved arXiv papers support bibtex or csl; DOI RIS behavior is unchanged"))),
    };
    Ok(json!({"document_id":document.id,"format":format,"source":p.abstract_url,"version":p.versioned_id,"metadata_source":if p.metadata_origin.is_empty(){"retained legacy arXiv metadata"}else{&p.metadata_origin},"text":text}))
}

#[cfg(test)]
mod scholarly_tests {
    use super::*;
    #[test]
    fn selected_license_and_version_are_independent_of_latest_history() {
        let html=r#"<html><head><meta name="citation_title" content="Selected paper"><meta name="citation_author" content="Literal Author"><meta name="citation_abstract" content="Abstract"></head><body><div id="abs"><div class="arxividv"><a href="/abs/2401.12345v1">for this version</a></div></div><div class="submission-history"><strong><a href="/abs/2401.12345v2">[v2]</a></strong> Tue, 2 Jan 2024 03:04:05 UTC<br><strong>[v1]</strong> Mon, 1 Jan 2024 03:04:05 UTC<br></div><div class="full-text"><div class="abs-license"><a href="https://creativecommons.org/licenses/by/4.0/">license</a></div></div></body></html>"#;
        let wanted=Identity{id:"2401.12345".into(),version:Some("1".into())};
        let p=parse_html(html.as_bytes(),"https://arxiv.org/abs/2401.12345v1",&wanted).unwrap();
        assert_eq!(p.version,"1");assert_eq!(p.source_dates["selected_submission"],"2024-01-01T03:04:05+00:00");
        assert_eq!(p.rights().decision,webtool_protocol::ReuseDecision::Permitted);
        let ambiguous=html.replace("</div></div></body>","<a href=\"https://arxiv.org/licenses/nonexclusive-distrib/1.0/\">other</a></div></div></body>");
        assert_eq!(parse_html(ambiguous.as_bytes(),"https://arxiv.org/abs/2401.12345v1",&wanted).unwrap().rights().decision,webtool_protocol::ReuseDecision::Unknown);
        let contradiction=html.replace("arxividv\"><a href=\"/abs/2401.12345v1", "arxividv\"><a href=\"/abs/2401.12345v2");
        assert!(parse_html(contradiction.as_bytes(),"https://arxiv.org/abs/2401.12345v1",&wanted).is_err());
        assert_eq!(reuse_decision("https://creativecommons.org/licenses/by-nc/4.0/"),webtool_protocol::ReuseDecision::Unknown);
        assert_eq!(reuse_decision("https://example.org/licenses/by/4.0/"),webtool_protocol::ReuseDecision::Unknown);
    }
}
