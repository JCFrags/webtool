pub mod config;
pub mod fetch;
pub mod jobs;
pub mod media;
pub mod readers;
pub mod search;
pub mod sources;
pub mod store;
use std::{collections::HashMap,sync::{Arc,Weak},time::Instant};
use anyhow::{anyhow,bail,Context,Result};
use chrono::Utc;
use regex::RegexBuilder;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use tokio::sync::{Mutex,Semaphore};
use tokio_util::sync::CancellationToken;
use webtool_protocol::*;
use config::Config;
use store::Store;

#[derive(Clone)]
pub struct Engine {
    pub config:Arc<Config>,pub store:Store,pub client:reqwest::Client,
    operation_slots:Arc<Semaphore>,
    submission_lock:Arc<Mutex<()>>,
    network:Arc<Semaphore>,parse_slots:Arc<Semaphore>,browser_slots:Arc<Semaphore>,
    job_slots:Arc<Semaphore>,locks:Arc<Mutex<HashMap<String,Weak<Mutex<()>>>>>,
    job_tokens:Arc<Mutex<HashMap<String,CancellationToken>>>,
    crawl_pacing:Arc<Mutex<HashMap<String,tokio::time::Instant>>>,
}
impl Engine {
    pub async fn new(config:Config)->Result<Self>{
        config.validate()?;
        let store=Store::open(&config.data_dir).await?;
        let client=fetch::client(&config)?;
        Ok(Self {store,client,operation_slots:Arc::new(Semaphore::new(config.network_concurrency + config.parse_concurrency)),submission_lock:Arc::new(Mutex::new(())),network:Arc::new(Semaphore::new(config.network_concurrency)),
            parse_slots:Arc::new(Semaphore::new(config.parse_concurrency)),browser_slots:Arc::new(Semaphore::new(config.browser_concurrency)),
            job_slots:Arc::new(Semaphore::new(config.job_concurrency)),config:Arc::new(config),
            locks:Arc::new(Mutex::new(HashMap::new())),job_tokens:Arc::new(Mutex::new(HashMap::new())),
            crawl_pacing:Arc::new(Mutex::new(HashMap::new()))})
    }
    pub async fn health(&self)->Health{
        let helper=|name:&str,path:&Option<std::path::PathBuf>|Capability{
            name:name.into(),available:path.as_ref().is_some_and(|p|p.is_file()),
            detail:if path.is_some(){"Configured executable path. Run an actual request to verify compatibility.".into()}else{"Not configured on this server.".into()},
        };
        Health {version:env!("CARGO_PKG_VERSION").into(),api_version:API_VERSION.into(),capabilities:vec![
            Capability{name:"html".into(),available:cfg!(feature="web-extraction"),detail:"Native extraction and explicit CSS selection. No quality benchmark has been run.".into()},
            Capability{name:"search".into(),available:cfg!(feature="web-search"),detail:format!("Configured providers: {}. Availability is not verified by this endpoint.",self.config.search_engines.join(", "))},
            Capability{name:"documents".into(),available:cfg!(feature="documents"),detail:"Compiled Xberg document support for native PDF text and structured tables. This is not OCR or a guarantee of format accuracy.".into()},
            Capability{name:"ocr".into(),available:cfg!(feature="ocr"),detail:if cfg!(feature="ocr"){"OCR feature compiled; configured backend/model readiness is not verified.".into()}else{"OCR is not compiled. Image-only scans require OCR; native PDF text does not.".into()}},
            Capability{name:"crw_browser".into(),available:cfg!(feature="crw-browser")&&self.config.crw_renderer.is_some(),detail:"Experimental fastCRW adapter.".into()},
            helper("lightpanda",&self.config.lightpanda_path),helper("chromium",&self.config.chromium_path),
            Capability{name:"yt_dlp".into(),available:self.config.ytdlp_path.as_ref().is_some_and(|p|media::executable(p)),
                detail:format!("Configured executable checked locally; no YouTube request made. JS runtime: {}. Track availability and runtime compatibility require an actual read.",self.config.ytdlp_js_runtime.as_deref().unwrap_or("yt-dlp default (Deno)"))},
        ]}
    }
    pub async fn read(&self,request:ReadRequest)->Result<ReadResponse>{
        let _operation=self.operation_slots.acquire().await?;
        let url=fetch::validated_url(&request.url)?;
        if let Some(name)=&request.library{self.store.require_library(name).await?;}
        let caption_url = if matches!(request.renderer,Renderer::Captions) || (matches!(request.renderer,Renderer::Auto) && request.selector.is_none()) {
            media::youtube_url(url.as_str())?
        } else { None };
        if matches!(request.renderer,Renderer::Captions) && (caption_url.is_none() || request.selector.is_some()) {
            bail!("unsupported captions request: use a YouTube watch/youtu.be URL without a CSS selector");
        }
        if caption_url.is_some() { media::validate_language(&request.language)?; }
        let key=hex::encode(Sha256::digest(serde_json::to_vec(&json!({"url":caption_url.as_deref().unwrap_or(url.as_str()),"renderer":request.renderer,
            "language":request.language,"media_parser":media::PARSER,"source_resolver":sources::VERSION,"selector":request.selector,"version":EXTRACTION_VERSION,"document_config":self.config.document_config,
            "browser_wait_ms":self.config.browser_wait_ms,"crw":self.config.crw_renderer}))?));
        let lock={
            let mut locks=self.locks.lock().await;
            locks.retain(|_,v|v.strong_count()>0);
            if let Some(lock)=locks.get(&key).and_then(Weak::upgrade){lock}else{
                let lock=Arc::new(Mutex::new(()));locks.insert(key.clone(),Arc::downgrade(&lock));lock
            }
        };
        let _same_source=lock.lock().await;
        if !request.refresh{
            if let Some(document)=self.store.cached(&key,self.config.cache_seconds).await?{
                if let Some(name)=&request.library{self.store.add(name,&document.id,request.actor.clone()).await?;}
                return Ok(ReadResponse{document,cached:true});
            }
        }
        if let Some(canonical)=caption_url {
            let _slot=self.parse_slots.acquire().await?;
            let (parsed,bytes,mime)=media::read(&canonical,&request.language,&self.config).await?;
            let original=self.store.put_bytes(&bytes,&mime,"caption_track").await?;
            let source=Source{requested:request.url,resolved:canonical,retrieved_at:Utc::now().to_rfc3339(),status:None,version:None,original};
            let document=self.finish(parsed,source,vec![]).await?;
            self.store.cache(key,document.id.clone()).await?;
            if let Some(name)=request.library{self.store.add(&name,&document.id,request.actor).await?;}
            return Ok(ReadResponse{document,cached:false});
        }
        let (fetched,github)={
            let _slot=self.network.acquire().await?;
            let native=if matches!(request.renderer,Renderer::Auto) && request.selector.is_none() {
                sources::github(&self.client,&url,self.config.max_bytes).await?
            } else { None };
            if let Some(resolved)=native { (resolved.fetched,Some(resolved.details)) }
            else {
                let fetched=match request.renderer {
                    Renderer::Auto|Renderer::Http=>fetch::http(&self.client,url.as_str(),self.config.max_bytes).await?,
                    _=>{let _browser=self.browser_slots.acquire().await?;fetch::browser(url.as_str(),&request.renderer,&self.config).await?},
                };
                (fetched,None)
            }
        };
        let filename=github.as_ref().map(|g|g.filename.clone()).unwrap_or_else(||fetched.resolved.clone());
        let mime=readers::detect(&filename,fetched.content_type.as_deref(),&fetched.bytes);
        let original=self.store.put_bytes(&fetched.bytes,&mime,&fetched.role).await?;
        let source=Source {requested:request.url,resolved:fetched.resolved.clone(),retrieved_at:Utc::now().to_rfc3339(),
            status:fetched.status,version:fetched.version,original};
        let readme_links=github.as_ref().filter(|g|g.readme).map(|g|sources::readme_links(&fetched.bytes,g));
        let mut parsed=if let Some(details)=github.as_ref().filter(|g|g.directory) {
            sources::directory(&fetched.bytes,details)?
        } else { self.parse(fetched.bytes,filename,mime,request.selector).await? };
        if let Some(details)=github { parsed.metadata["github"]=details.metadata; }
        if let Some(links)=readme_links {
            parsed.links.extend(links);
            parsed.warnings.push(Warning::new("readme_links_partial","Pinned links supplement inline Markdown links and reference definitions. Original text is unchanged; complex Markdown/HTML link syntax is not fully interpreted."));
        }
        let document=self.finish(parsed,source,fetched.warnings).await?;
        self.store.cache(key,document.id.clone()).await?;
        if let Some(name)=request.library{self.store.add(&name,&document.id,request.actor).await?;}
        Ok(ReadResponse{document,cached:false})
    }
    async fn parse(&self,bytes:Vec<u8>,name:String,mime:String,selector:Option<String>)->Result<readers::Parsed>{
        let permit=self.parse_slots.clone().acquire_owned().await?;
        if readers::is_document_format(&mime){
            // The permit stays alive for the whole document operation.
            let _permit=permit;
            return readers::document::parse(&bytes,&name,&self.config).await;
        }
        tokio::task::spawn_blocking(move||{
            let _permit=permit;readers::parse(&bytes,&name,&mime,selector.as_deref())
        }).await.context("reader task failed")?
    }
    async fn finish(&self,parsed:readers::Parsed,source:Source,mut warnings:Vec<Warning>)->Result<Document>{
        warnings.extend(parsed.warnings);
        if parsed.blocks.is_empty(){warnings.push(Warning::new("empty_document","The source was accepted but contains no readable blocks."));}
        let id=hex::encode(Sha256::digest(serde_json::to_vec(&json!({"source":source.requested,"hash":source.original.sha256,
            "version":source.version,"parser":parsed.parser,"extraction":EXTRACTION_VERSION,"blocks":parsed.blocks,"configuration":self.config.document_config}))?));
        let document=Document{schema_version:1,id,title:parsed.title,source,parser:parsed.parser,extraction_version:EXTRACTION_VERSION.into(),
            blocks:parsed.blocks,links:parsed.links,metadata:parsed.metadata,warnings};
        self.store.save(document).await
    }
    pub async fn ingest(&self,bytes:Vec<u8>,name:String,library:Option<String>,actor:Option<String>,selector:Option<String>)->Result<Document>{
        let _operation=self.operation_slots.acquire().await?;
        if bytes.len()>self.config.max_bytes{bail!("upload exceeds configured size limit");}
        if name.is_empty()||name.len()>256||name.contains('/')||name.contains('\\'){bail!("upload name must be a filename, not a path");}
        if let Some(name)=&library{self.store.require_library(name).await?;}
        let mime=readers::detect(&name,None,&bytes);
        let original=self.store.put_bytes(&bytes,&mime,"upload").await?;
        let source=Source{requested:format!("upload:{name}"),resolved:format!("upload:{name}"),retrieved_at:Utc::now().to_rfc3339(),
            status:None,version:None,original};
        let parsed=self.parse(bytes,name,mime,selector).await?;
        let document=self.finish(parsed,source,vec![]).await?;
        if let Some(name)=library{self.store.add(&name,&document.id,actor).await?;}
        Ok(document)
    }
    pub async fn media(&self,url:String,language:String,library:Option<String>)->Result<Document>{
        Ok(self.read(ReadRequest{url,language,library,renderer:Renderer::Captions,refresh:false,selector:None,actor:None}).await?.document)
    }
    pub async fn search(&self,request:SearchRequest)->Result<SearchResponse>{
        let start=Instant::now();
        if let Some(library)=&request.library{
            if library!="*"{self.store.require_library(library).await?;}
            let results=self.store.search(request.query.clone(),if library=="*"{None}else{Some(library.clone())},request.limit).await?;
            return Ok(SearchResponse{query:request.query,results,warnings:vec![],elapsed_ms:start.elapsed().as_millis() as u64});
        }
        let _slot=self.network.acquire().await?;
        search::search(request,&self.config).await
    }
    pub async fn find(&self,id:&str,request:FindRequest)->Result<FindResponse>{
        let document=self.store.document(id).await?;
        tokio::task::spawn_blocking(move||find(&document,&request)).await?
    }
    pub async fn extract(&self,id:&str,request:ExtractRequest)->Result<ExtractResponse>{
        let d=self.store.document(id).await?;
        let mut warnings=d.warnings.clone();
        let data=match request.kind{
            ExtractKind::Links=>json!(d.links),ExtractKind::Metadata=>json!({"title":d.title,"source":d.source,"metadata":d.metadata}),
            ExtractKind::Tables|ExtractKind::Code|ExtractKind::Images|ExtractKind::Outline=>{
                let blocks:Vec<_>=d.blocks.iter().filter(|b|matches!((&request.kind,&b.content),
                    (ExtractKind::Tables,Content::Table{..})|(ExtractKind::Code,Content::Code{..})|
                    (ExtractKind::Images,Content::Image{..})|(ExtractKind::Outline,Content::Heading{..}))).collect();json!(blocks)
            },
            ExtractKind::JsonPointer=>{
                let bytes=self.store.bytes(&d.source.original).await?;let v:Value=serde_json::from_slice(&bytes).context("original is not a JSON document")?;
                let pointer=request.expression.context("a JSON pointer expression is required")?;
                match v.pointer(&pointer){Some(value)=>json!({"pointer":pointer,"found":true,"value":value}),None=>{
                    warnings.push(Warning::new("missing_value","The requested JSON pointer does not exist."));json!({"pointer":pointer,"found":false,"value":null})
                }}
            },
            ExtractKind::Css=>{
                if !matches!(d.source.original.media_type.as_str(),"text/html"|"application/xhtml+xml"){bail!("CSS extraction requires an HTML original");}
                let bytes=self.store.bytes(&d.source.original).await?;
                let css=request.expression.context("a CSS selector expression is required")?;
                let source=String::from_utf8(bytes)?;
                json!(tokio::task::spawn_blocking(move||readers::html::select_original(&source,&css)).await??)
            },
        };
        Ok(ExtractResponse{document_id:d.id,data,warnings})
    }
    pub async fn citation(&self,doi:&str,format:&str)->Result<Value>{
        let doi=doi.trim().trim_start_matches("https://doi.org/").trim_start_matches("doi:");
        if !doi.starts_with("10.")||!doi.contains('/')||doi.chars().any(char::is_whitespace){bail!("expected a DOI such as 10.1234/example");}
        let accept=match format{"bibtex"=>"application/x-bibtex","ris"=>"application/x-research-info-systems","csl"=>"application/vnd.citationstyles.csl+json",_=>bail!("format must be bibtex, ris, or csl")};
        let mut url=url::Url::parse("https://doi.org/")?;url.set_path(doi);
        let response=self.client.get(url.clone()).header(reqwest::header::ACCEPT,accept).send().await?.error_for_status()?;
        use futures_util::StreamExt;
        let mut data=Vec::new();
        let mut stream=response.bytes_stream();
        while let Some(part)=stream.next().await {
            let part=part?;
            if data.len().saturating_add(part.len())>1024*1024 { bail!("citation response exceeds one megabyte"); }
            data.extend_from_slice(&part);
        }
        Ok(json!({"doi":doi,"format":format,"source":url.as_str(),"text":std::str::from_utf8(&data)?}))
    }
}

pub fn find(document:&Document,request:&FindRequest)->Result<FindResponse>{
    if request.query.is_empty()||request.query.len()>4096{bail!("find query must contain 1 to 4096 bytes");}
    if !(1..=1000).contains(&request.limit){bail!("find limit must be between 1 and 1000");}
    let pattern=if request.regex{request.query.clone()}else{regex::escape(&request.query)};
    let regex=RegexBuilder::new(&pattern).case_insensitive(request.ignore_case).size_limit(2*1024*1024).build().context("invalid find pattern")?;
    let mut matches=Vec::new();let mut remaining=request.limit;let mut truncated=false;
    for b in &document.blocks{
        let text=b.content.text();let mut ranges=Vec::new();
        for m in regex.find_iter(&text){
            if remaining==0{truncated=true;break;}
            ranges.push([m.start(),m.end()]);remaining-=1;
        }
        if !ranges.is_empty(){matches.push(Match{block_id:b.id.clone(),locator:b.locator.clone(),text,ranges});}
        if truncated{break;}
    }
    Ok(FindResponse{document_id:document.id.clone(),matches,truncated})
}
