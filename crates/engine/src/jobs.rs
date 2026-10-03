//! Application-owned bounded crawl workers with durable SQLite frontiers.
//! Startup never retries a request. Explicit resume retains all attempt charges.
mod frontier;
mod robots;
mod sitemap;
use std::{sync::Arc,time::Duration};
use anyhow::{bail,Context,Result};
use chrono::Utc;
use futures_util::{stream::FuturesUnordered,StreamExt,FutureExt};
use serde_json::json;
use tokio_util::sync::CancellationToken;
use url::Url;
use webtool_protocol::*;
use crate::{Engine,fetch};
use frontier::{Frontier,Kind,State,PAGE_CANDIDATES,SITEMAP_LIMIT,SITEMAP_DEPTH,warn,warn_once};
use robots::{Robots,target};

impl Engine {
    pub async fn recover_jobs(&self)->Result<()>{
        for mut job in self.store.jobs().await? {
            if !job.state.terminal() {
                job.state=JobState::Interrupted;
                job.error=Some("Server stopped before this job finished. Explicit resume is required; saved documents remain available.".into());
                if matches!(job.request,JobRequest::Media(_)) {
                    self.recover_media(&mut job).await?;
                } else if let Some(mut frontier)=Frontier::load(&self.store,&job.id).await? {
                    frontier.interrupt_active();
                    frontier.checkpoint(&self.store,&mut job,None).await?;
                } else {
                    job.updated_at=Utc::now().to_rfc3339();
                    self.store.put_job(job).await?;
                }
            }
        }
        Ok(())
    }
    pub async fn submit(&self,request:CrawlRequest)->Result<Job>{
        let _submission=self.submission_lock.lock().await;
        let base=fetch::validated_url(&request.url)?;
        if !(1..=500).contains(&request.max_pages)||request.max_depth>8 { bail!("crawl limits are 1 to 500 pages and 0 to 8 levels"); }
        if request.url.len()>8192 || request.sitemaps.len()>8 { bail!("crawl_invalid_request: use a URL up to 8192 bytes and at most eight sitemap roots"); }
        for sitemap in &request.sitemaps {
            if sitemap.len()>=2048 || fetch::validated_url(sitemap)?.origin()!=base.origin() { bail!("crawl_invalid_request: sitemap roots must be same-origin URLs under 2048 bytes"); }
        }
        if let Some(name)=&request.library { self.store.require_library(name).await?; }
        self.require_job_capacity().await?;
        let now=Utc::now().to_rfc3339();
        let mut job=Job { id:uuid::Uuid::new_v4().to_string(),state:JobState::Queued,request:JobRequest::Crawl(request.clone()),created_at:now.clone(),updated_at:now,
            document_ids:vec![],visited:0,failed:0,progress:None,media:None,warnings:vec![Warning::new("crawl_scope","Bounded same-origin HTTP crawl, not a complete-site archive. Started attempts consume budget, including interruptions. Visited counts completed attempts only."),Warning::new("crawl_policy","Robots metadata uses a stricter bounded policy: same-origin redirects, identity encoding, UTF-8, and no assumption of permission on errors. This is not full RFC 9309 conformance.")],error:None };
        let mut frontier=Frontier::default();
        frontier.admit(Kind::Page,base.to_string(),0,None);
        for sitemap in &request.sitemaps { frontier.admit(Kind::Sitemap,fetch::validated_url(sitemap)?.to_string(),0,None); }
        frontier.checkpoint(&self.store,&mut job,None).await?;
        self.schedule(job.clone()).await;
        Ok(job)
    }
    pub(crate) async fn require_job_capacity(&self)->Result<()> {
        if self.shutting_down.load(std::sync::atomic::Ordering::SeqCst) { bail!("job queue is full"); }
        if self.store.jobs().await?.iter().filter(|j|!j.state.terminal()).count()>=100 { bail!("job queue is full"); }
        Ok(())
    }
    pub async fn resume(&self,id:&str,request:CrawlResumeRequest)->Result<Job> {
        let _submission=self.submission_lock.lock().await;
        let mut job=self.store.job(id).await?;
        let crawl=job.request.crawl().context("media_resume_unsupported: submit a new preview-checked media job")?.clone();
        if !matches!(job.state,JobState::Interrupted|JobState::Cancelled|JobState::Partial) || self.job_tokens.lock().await.contains_key(id) {
            bail!("crawl_not_resumable: job must be interrupted, cancelled, or partial with no active worker");
        }
        let mut frontier=Frontier::load(&self.store,id).await?.context("crawl_not_resumable: historical job has no saved frontier")?;
        let max=request.max_pages.unwrap_or(crawl.max_pages);
        if max<crawl.max_pages || max>500 { bail!("crawl_invalid_request: resume may only keep or increase the total page budget up to 500"); }
        frontier.requeue_interrupted();
        let p=frontier.progress();
        let pages=p.pending>0 && p.attempted<max;
        let maps=p.sitemap_pending>0 && p.sitemap_attempted<SITEMAP_LIMIT;
        let discovery=!frontier.sitemaps_seeded && crawl.discover_sitemaps;
        if !pages && !maps && !discovery { bail!("crawl_not_resumable: no pending work within the budget; increase max_pages if page attempts remain"); }
        self.require_job_capacity().await?;
        if let Some(name)=&crawl.library { self.store.require_library(name).await?; }
        job.request.crawl_mut().expect("crawl request checked").max_pages=max; job.state=JobState::Queued; job.error=None;
        warn_once(&mut job,"crawl_resumed","Explicit resume retains saved documents and prior attempt charges. Interrupted requests may be fetched again; completed failures and exclusions are not retried.");
        frontier.checkpoint(&self.store,&mut job,None).await?;
        self.schedule(job.clone()).await;
        Ok(job)
    }
    pub(crate) async fn schedule(&self,job:Job) {
        let token=CancellationToken::new(); self.job_tokens.lock().await.insert(job.id.clone(),token.clone());
        let engine=self.clone();
        tokio::spawn(async move {
            let id=job.id.clone();
            let is_media=matches!(job.request,JobRequest::Media(_));
            let result=std::panic::AssertUnwindSafe(engine.execute_job(job,token)).catch_unwind().await;
            let error=match result { Ok(Ok(()))=>None,Ok(Err(e))=>Some(if is_media { "media_helper_failed: worker failed; staging may require operator inspection".into() } else {format!("{e:#}").chars().take(2000).collect::<String>()}),Err(_)=>Some("job worker panicked".into()) };
            if let Some(error)=error {
                let save=async {
                    let mut job=engine.store.job(&id).await?;
                    job.state=JobState::Failed; job.error=Some(error);
                    if let Some(mut frontier)=Frontier::load(&engine.store,&id).await? {
                        frontier.interrupt_active(); frontier.checkpoint(&engine.store,&mut job,None).await
                    } else { job.updated_at=Utc::now().to_rfc3339(); engine.store.put_job(job).await }
                }.await;
                if let Err(e)=save { tracing::error!(error=%e,"cannot persist failed job"); }
            }
            engine.job_tokens.lock().await.remove(&id);
            engine.job_finished.notify_waiters();
        });
    }
    pub async fn cancel(&self,id:&str)->Result<serde_json::Value> {
        let _submission=self.submission_lock.lock().await;
        let job=self.store.job(id).await?;
        if job.state.terminal() { return Ok(json!({"id":id,"cancel_requested":false,"state":job.state})); }
        if let Some(token)=self.job_tokens.lock().await.get(id) { token.cancel(); }
        Ok(json!({"id":id,"cancel_requested":true,"state":job.state}))
    }
    pub async fn stop_jobs(&self) {
        self.shutting_down.store(true,std::sync::atomic::Ordering::SeqCst);
        { let _submission=self.submission_lock.lock().await;
          for token in self.job_tokens.lock().await.values() { token.cancel(); } }
        // Do not return from graceful shutdown while owned helpers or cleanup remain.
        loop {
            let done=self.job_finished.notified(); tokio::pin!(done); done.as_mut().enable();
            if self.job_tokens.lock().await.is_empty() { break; }
            done.await;
        }
    }
    async fn pace(&self,origin:&str,delay:Duration) {
        let when={let mut map=self.crawl_pacing.lock().await;
            let now=tokio::time::Instant::now();map.retain(|_,until|*until>now);
            let when=map.get(origin).copied().unwrap_or(now).max(now);map.insert(origin.into(),when+delay);when};
        tokio::time::sleep_until(when).await;
    }
    async fn cancelled(&self,frontier:&mut Frontier,job:&mut Job)->Result<()> {
        frontier.interrupt_active(); job.state=JobState::Cancelled;
        frontier.checkpoint(&self.store,job,None).await
    }
    async fn execute_job(&self,mut job:Job,token:CancellationToken)->Result<()> {
        if matches!(job.request,JobRequest::Media(_)) { return self.execute_media_job(job,token).await; }
        let request=job.request.crawl().context("crawl_invalid_request: wrong job type")?.clone();
        let mut frontier=Frontier::load(&self.store,&job.id).await?.context("crawl has no persistent frontier")?;
        let permit=tokio::select! { result=self.job_slots.acquire()=>Some(result?),_=token.cancelled()=>None };
        let Some(_permit)=permit else { return self.cancelled(&mut frontier,&mut job).await; };
        job.state=JobState::Running; frontier.checkpoint(&self.store,&mut job,None).await?;
        let base=fetch::validated_url(&request.url)?; let origin=base.origin().ascii_serialization();
        let metadata_client=crawl_client(&self.config,&base,None,true)?;
        let robots=tokio::select! {
            r=async {
                let _slot=self.network.acquire().await?;
                self.pace(&origin,Duration::from_millis(500)).await;
                robots::load(&metadata_client,&origin,self.config.max_bytes).await
            }=>Arc::new(r?),
            _=token.cancelled()=>return self.cancelled(&mut frontier,&mut job).await,
        };
        let mut engine=self.clone();
        engine.client=crawl_client(&self.config,&base,Some(robots.clone()),false)?;
        let metadata_client=crawl_client(&self.config,&base,Some(robots.clone()),true)?;
        if !frontier.sitemaps_seeded {
            if request.discover_sitemaps {
                for url in &robots.sitemaps { admit(&mut frontier,&mut job,&base,&robots,Kind::Sitemap,url,0,None); }
                if robots.sitemap_truncated { warn_once(&mut job,"sitemap_limit","Robots sitemap discovery stopped at 32 declarations."); }
            }
            frontier.sitemaps_seeded=true;
            frontier.checkpoint(&self.store,&mut job,None).await?;
        }
        // Discovery is bounded and checkpointed before page scheduling. Sitemap
        // URLs become depth-zero seeds; link depth starts at each seed.
        while !token.is_cancelled() && frontier.progress().sitemap_attempted<SITEMAP_LIMIT {
            let Some(i)=frontier.next(Kind::Sitemap,None) else { break; };
            let entry=frontier.entries[i].clone();
            if !robots.allowed(&target(&fetch::validated_url(&entry.url)?)) {
                frontier.change(i,State::Excluded,Some("robots.txt disallows this sitemap".into()));
                warn(&mut job,"robots_excluded",entry.url);
                frontier.checkpoint(&self.store,&mut job,None).await?; continue;
            }
            frontier.change(i,State::Active,None);
            frontier.checkpoint(&self.store,&mut job,None).await?;
            let result:Result<_>=tokio::select! {
                r=async {
                    let _slot=self.network.acquire().await?;
                    self.pace(&origin,robots.delay).await;
                    let response=metadata_client.get(&entry.url).header(reqwest::header::ACCEPT_ENCODING,"identity").send().await?;
                    if !response.status().is_success() { bail!("sitemap returned HTTP {}",response.status().as_u16()); }
                    let resolved=response.url().clone();
                    let bytes=robots::bounded_identity_body(response,self.config.max_bytes.min(sitemap::MAX_BYTES)).await?;
                    let map=sitemap::parse(&bytes)?;
                    Ok((resolved,map))
                }=>r,
                _=token.cancelled()=>return self.cancelled(&mut frontier,&mut job).await,
            };
            match result {
                Ok((resolved,map))=>{
                    frontier.change(i,State::Saved,None);
                    if map.invalid>0 { warn(&mut job,"sitemap_invalid_entries",format!("{}: {} malformed locations were skipped",entry.url,map.invalid)); }
                    if map.truncated { warn_once(&mut job,"sitemap_limit","Sitemap entry scanning reached its bounded limit."); }
                    let directory=resolved.path().rsplit_once('/').map(|(prefix,_)|format!("{prefix}/")).unwrap_or_else(||"/".into());
                    for url in map.urls {
                        let depth=if map.kind==Kind::Sitemap { entry.depth+1 } else { 0 };
                        let scope=if map.kind==Kind::Page { Some(directory.as_str()) } else { None };
                        admit(&mut frontier,&mut job,&base,&robots,map.kind,&url,depth,scope);
                    }
                },
                Err(e)=>{
                    let error=format!("{e:#}").chars().take(2000).collect::<String>();
                    frontier.change(i,State::Failed,Some(error.clone())); warn(&mut job,"sitemap_failed",format!("{}: {error}",entry.url));
                },
            }
            frontier.checkpoint(&self.store,&mut job,None).await?;
        }
        if token.is_cancelled() { return self.cancelled(&mut frontier,&mut job).await; }
        if frontier.next(Kind::Sitemap,None).is_some() { warn_once(&mut job,"sitemap_limit","Sitemap attempt budget reached. Unfinished discovery remains visible and saved."); }
        let mut results=FuturesUnordered::new(); let mut level=None;
        loop {
            if token.is_cancelled() { break; }
            if results.is_empty() { level=frontier.next(Kind::Page,None).map(|i|frontier.entries[i].depth); }
            while level.is_some() && results.len()<self.config.crawl_concurrency && frontier.progress().attempted<request.max_pages && !token.is_cancelled() {
                let Some(i)=frontier.next(Kind::Page,level) else { break; };
                let entry=frontier.entries[i].clone();
                if !robots.allowed(&target(&fetch::validated_url(&entry.url)?)) {
                    frontier.change(i,State::Excluded,Some("robots.txt disallows this page".into())); warn(&mut job,"robots_excluded",entry.url);
                    frontier.checkpoint(&self.store,&mut job,None).await?; continue;
                }
                // Persist the charge before any pacing, read queue, or network wait.
                frontier.change(i,State::Active,None);
                frontier.checkpoint(&self.store,&mut job,None).await?;
                let engine=engine.clone(); let origin=origin.clone(); let delay=robots.delay;
                results.push(async move {
                    engine.pace(&origin,delay).await;
                    let result=engine.read(ReadRequest { url:entry.url,refresh:true,renderer:Renderer::Http,language:default_language(),library:None,selector:None,actor:None }).await;
                    (i,result)
                });
            }
            if results.is_empty() {
                if frontier.progress().attempted>=request.max_pages || frontier.next(Kind::Page,None).is_none() { break; }
                continue;
            }
            let next=tokio::select! { biased; _=token.cancelled()=>break, r=results.next()=>r };
            let Some((i,result))=next else { continue; };
            let entry=frontier.entries[i].clone(); let mut attachment=None;
            let document=result.and_then(|r| {
                if r.document.blocks.is_empty() || !fetch::validated_url(&r.document.source.resolved).is_ok_and(|u|u.origin()==base.origin() && robots.allowed(&target(&u))) { bail!("empty, excluded, or out-of-scope result was not attached"); }
                Ok(r.document)
            });
            match document {
                Ok(d)=>{
                    frontier.change(i,State::Saved,None); frontier.entries[i].document_id=Some(d.id.clone()); attachment=Some(d.id.clone());
                    if !d.warnings.is_empty() { warn(&mut job,"document_warnings",format!("{} has {} extraction warnings",d.id,d.warnings.len())); }
                    if d.links.len()>PAGE_CANDIDATES { warn_once(&mut job,"frontier_limit","Link scanning stopped at 10000 entries per document."); }
                    for link in d.links.into_iter().take(PAGE_CANDIDATES) { admit(&mut frontier,&mut job,&base,&robots,Kind::Page,&link.url,entry.depth+1,None); }
                },
                Err(e)=>{
                    let error=format!("{e:#}").chars().take(2000).collect::<String>();
                    frontier.change(i,State::Failed,Some(error.clone())); warn(&mut job,"crawl_read_failed",format!("{}: {error}",entry.url));
                },
            }
            // Each usable document and newly discovered candidates become visible
            // together, before refilling this depth's vacant worker slot.
            frontier.checkpoint(&self.store,&mut job,attachment).await?;
        }
        drop(results);
        if token.is_cancelled() { return self.cancelled(&mut frontier,&mut job).await; }
        if frontier.next(Kind::Page,None).is_some() { warn_once(&mut job,"page_limit_reached","The total attempt budget was reached. Resume can increase it up to 500 without resetting past charges."); }
        if job.document_ids.is_empty() && job.visited>0 {
            job.state=JobState::Failed; job.error=Some("No usable documents were saved from the completed crawl attempts.".into());
        } else {
            job.state=if job.warnings.iter().all(|w|matches!(w.code.as_str(),"crawl_scope"|"crawl_policy")) { JobState::Complete } else { JobState::Partial };
        }
        frontier.checkpoint(&self.store,&mut job,None).await
    }
    pub async fn map(&self,url:&str,limit:usize)->Result<serde_json::Value>{
        if !(1..=5000).contains(&limit){bail!("map limit must be between 1 and 5000");}
        let _permit=self.network.acquire().await?;
        let result=fetch::http(&self.client,url,self.config.max_bytes).await?;
        let mime=crate::readers::detect(&result.resolved,result.content_type.as_deref(),&result.bytes);
        if matches!(mime.as_str(),"text/html"|"application/xhtml+xml") {
            let original=self.store.put_bytes(&result.bytes,&mime,&result.role).await?;
            let decoded=self.decode_html(result.bytes,mime,result.content_type,false).await?;
            let all=crate::readers::html::links(&decoded.text,&result.resolved);
            return Ok(json!({"source":result.resolved,"kind":"page_links","links":all.iter().take(limit).collect::<Vec<_>>(),
                "truncated":all.len()>limit,"original":original,"html_encoding":decoded.metadata,"warnings":decoded.warnings}));
        }
        let source=std::str::from_utf8(&result.bytes)?;
        if let Ok(tree)=roxmltree::Document::parse(source){
            if matches!(tree.root_element().tag_name().name(),"urlset"|"sitemapindex"){
                let all:Vec<_>=tree.descendants().filter(|n|n.has_tag_name("loc")).filter_map(|n|n.text())
                    .filter_map(|s|fetch::validated_url(s).ok()).map(|u|u.to_string()).collect();
                return Ok(json!({"source":result.resolved,"kind":tree.root_element().tag_name().name(),"urls":all.iter().take(limit).collect::<Vec<_>>(),"truncated":all.len()>limit,"nested_sitemaps_expanded":false}));
            }
        }
        let all=crate::readers::html::links(source,&result.resolved);
        Ok(json!({"source":result.resolved,"kind":"page_links","links":all.iter().take(limit).collect::<Vec<_>>(),"truncated":all.len()>limit}))
    }
}

fn crawl_client(config:&crate::config::Config,base:&Url,robots:Option<Arc<Robots>>,identity:bool)->Result<reqwest::Client> {
    let origin=base.origin();
    let agent=if config.user_agent.to_ascii_lowercase().contains("webtool") { config.user_agent.clone() } else { format!("{} webtool",config.user_agent) };
    let mut builder=reqwest::Client::builder().user_agent(agent).connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(config.request_timeout_seconds))
        .redirect(reqwest::redirect::Policy::custom(move |a| {
            if a.previous().len()>=10 { a.error("too many redirects") }
            else if a.url().origin()!=origin { a.error("redirect leaves crawl origin") }
            else if !a.url().username().is_empty() || a.url().password().is_some() { a.error("redirect contains credentials") }
            else if robots.as_ref().is_some_and(|r|!r.allowed(&target(a.url()))) { a.error("redirect excluded by robots.txt") }
            else { a.follow() }
        }));
    if identity { builder=builder.no_gzip().no_brotli().no_deflate().no_zstd(); }
    Ok(builder.build()?)
}
fn admit(frontier:&mut Frontier,job:&mut Job,base:&Url,robots:&Robots,kind:Kind,raw:&str,depth:usize,directory:Option<&str>) {
    if raw.len()>8192 || (kind==Kind::Sitemap && raw.len()>=2048) { warn_once(job,"crawl_url_limit","Oversized discovered URLs were not admitted."); return; }
    let Ok(url)=fetch::validated_url(raw) else { warn_once(job,"crawl_invalid_link","Invalid discovered URLs were not admitted."); return; };
    if frontier.contains(kind,url.as_str()) {
        if kind==Kind::Sitemap { warn_once(job,"sitemap_revisited","A duplicate or cyclic sitemap reference was not followed again."); }
        return;
    }
    let reason=if url.origin()!=base.origin() { Some(("crawl_scope_excluded","URL leaves the crawl origin")) }
        else if directory.is_some_and(|prefix|!url.path().starts_with(prefix)) { Some(("sitemap_scope_excluded","URL leaves the sitemap directory scope")) }
        else if !robots.allowed(&target(&url)) { Some(("robots_excluded","robots.txt disallows this URL")) }
        else if depth>if kind==Kind::Page { job.request.crawl().expect("crawl dispatch checked").max_depth } else { SITEMAP_DEPTH } { Some((if kind==Kind::Page { "depth_limit_reached" } else { "sitemap_depth_limit" },"URL exceeds the configured page depth or supported sitemap depth")) }
        else { None };
    if !frontier.admit(kind,url.to_string(),depth,reason.map(|(_,text)|text.into())) {
        warn_once(job,if kind==Kind::Page { "frontier_limit" } else { "sitemap_limit" },"Candidate admission limit reached, including excluded URLs.");
    } else if let Some((code,text))=reason { warn(job,code,format!("{url}: {text}")); }
}
