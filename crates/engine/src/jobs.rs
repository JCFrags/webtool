//! Persistent crawl records with a bounded in-process worker pool.
//! Interrupted running jobs are reported, not silently replayed as new work.
use std::{collections::{HashSet,VecDeque},sync::Arc,time::Duration};
use anyhow::{bail,Context,Result};
use chrono::Utc;
use futures_util::{stream,StreamExt,FutureExt};
use regex::Regex;
use serde_json::json;
use tokio_util::sync::CancellationToken;
use url::Url;
use webtool_protocol::*;
use crate::{Engine,fetch};

impl Engine {
    pub async fn recover_jobs(&self)->Result<()>{
        for mut job in self.store.jobs().await?{
            match job.state{
                JobState::Running=>{
                    job.state=JobState::Interrupted;job.updated_at=Utc::now().to_rfc3339();
                    job.error=Some("Server stopped while this job was running. Previously saved documents remain available.".into());
                    self.store.put_job(job).await?;
                },
                JobState::Queued=>self.schedule(job).await,
                _=>{},
            }
        }Ok(())
    }
    pub async fn submit(&self,request:CrawlRequest)->Result<Job>{
        let _submission=self.submission_lock.lock().await;
        fetch::validated_url(&request.url)?;
        if !(1..=500).contains(&request.max_pages)||request.max_depth>8{bail!("crawl limits are 1 to 500 pages and 0 to 8 levels");}
        if let Some(name)=&request.library{self.store.require_library(name).await?;}
        let queued=self.store.jobs().await?.iter().filter(|j|!j.state.terminal()).count();
        if queued>=100{bail!("job queue is full");}
        let now=Utc::now().to_rfc3339();
        let job=Job{id:uuid::Uuid::new_v4().to_string(),state:JobState::Queued,request,created_at:now.clone(),updated_at:now,
            document_ids:vec![],visited:0,failed:0,warnings:vec![Warning::new("crawl_scope","Bounded same-origin HTTP crawl only. Visited counts completed attempts (including failures), not pending/cancelled requests. Saved IDs identify usable documents; this is not a complete-site archive.")],error:None};
        self.store.put_job(job.clone()).await?;self.schedule(job.clone()).await;Ok(job)
    }
    async fn schedule(&self,job:Job){
        let token=CancellationToken::new();self.job_tokens.lock().await.insert(job.id.clone(),token.clone());
        let engine=self.clone();
        tokio::spawn(async move{
            let id=job.id.clone();let mut error_record=job.clone();
            let result=std::panic::AssertUnwindSafe(engine.execute_job(job,token)).catch_unwind().await;
            let error=match result{Ok(Ok(()))=>None,Ok(Err(e))=>Some(format!("{e:#}")),Err(_)=>Some("crawl worker panicked".into())};
            if let Some(error)=error{
                if let Ok(saved)=engine.store.job(&id).await{error_record=saved;}
                error_record.state=JobState::Failed;error_record.error=Some(error);
                error_record.updated_at=Utc::now().to_rfc3339();
                if let Err(e)=engine.store.put_job(error_record).await{tracing::error!(error=%e,"cannot persist failed job");}
            }
            engine.job_tokens.lock().await.remove(&id);
        });
    }
    pub async fn cancel(&self,id:&str)->Result<serde_json::Value>{
        let job=self.store.job(id).await?;
        if job.state.terminal(){return Ok(json!({"id":id,"cancel_requested":false,"state":job.state}));}
        if let Some(token)=self.job_tokens.lock().await.get(id){token.cancel();}
        Ok(json!({"id":id,"cancel_requested":true,"state":job.state}))
    }
    pub async fn stop_jobs(&self){for token in self.job_tokens.lock().await.values(){token.cancel();}}
    async fn pace(&self,origin:&str,delay:Duration){
        let when={let mut map=self.crawl_pacing.lock().await;
            let now=tokio::time::Instant::now();map.retain(|_,until|*until>now);
            let when=map.get(origin).copied().unwrap_or(now).max(now);map.insert(origin.into(),when+delay);when};
        tokio::time::sleep_until(when).await;
    }
    async fn execute_job(&self,mut job:Job,token:CancellationToken)->Result<()>{
        let permit=tokio::select!{
            result=self.job_slots.acquire()=>Some(result?),
            _=token.cancelled()=>None,
        };
        let Some(_permit)=permit else{job.state=JobState::Cancelled;job.updated_at=Utc::now().to_rfc3339();return self.store.put_job(job).await;};
        job.state=JobState::Running;job.updated_at=Utc::now().to_rfc3339();self.store.put_job(job.clone()).await?;
        let base=fetch::validated_url(&job.request.url)?;let origin=base.origin().ascii_serialization();
        // Crawl redirects stay in scope. Ordinary `read` requests may follow cross-origin redirects.
        let allowed_origin=base.origin();
        let mut engine=self.clone();
        engine.client=reqwest::Client::builder().user_agent(&self.config.user_agent)
            .timeout(Duration::from_secs(self.config.request_timeout_seconds))
            .redirect(reqwest::redirect::Policy::custom(move|a|{
                if a.previous().len()>=10{a.error("too many redirects")}
                else if a.url().origin()!=allowed_origin{a.error("redirect leaves crawl origin")}
                else{a.follow()}
            })).build()?;
        let robots=tokio::select!{r=load_robots(&engine.client,&origin,self.config.max_bytes)=>r?,_=token.cancelled()=>{
            job.state=JobState::Cancelled;return self.store.put_job(job).await;
        }};
        let robots=Arc::new(robots);
        // Recheck every redirect hop against robots as well as origin. Fresh
        // crawl reads cannot reuse an ordinary-read cache that bypassed this policy.
        let redirect_robots=robots.clone();let allowed_origin=base.origin();
        engine.client=reqwest::Client::builder().user_agent(&self.config.user_agent)
            .timeout(Duration::from_secs(self.config.request_timeout_seconds))
            .redirect(reqwest::redirect::Policy::custom(move|a|{
                if a.previous().len()>=10{a.error("too many redirects")}
                else if a.url().origin()!=allowed_origin{a.error("redirect leaves crawl origin")}
                else if !redirect_robots.allowed(&robot_target(a.url())){a.error("redirect excluded by robots.txt")}
                else{a.follow()}
            })).build()?;
        let mut frontier=VecDeque::from([(base.to_string(),0usize)]);
        let mut seen=HashSet::from([base.to_string()]);let mut attempts=0usize;
        while !frontier.is_empty()&&attempts<job.request.max_pages&&!token.is_cancelled(){
            let mut batch=Vec::new();
            let level=frontier.front().map(|(_,depth)|*depth);
            while batch.len()<self.config.crawl_concurrency&&attempts+batch.len()<job.request.max_pages{
                // Keep breadth-first depth semantics even at a short batch boundary.
                if frontier.front().map(|(_,depth)|*depth)!=level{break;}
                let Some((url,depth))=frontier.pop_front()else{break;};
                let u=Url::parse(&url)?;
                let target=robot_target(&u);
                if !robots.allowed(&target){job.warnings.push(Warning::new("robots_excluded",url));continue;}
                batch.push((url,depth));
            }
            if batch.is_empty(){continue;}
            attempts+=batch.len();
            let mut results=stream::iter(batch).map(|(url,depth)|{
                let engine=engine.clone();let token=token.clone();let origin=origin.clone();
                let delay=robots.delay;let request=ReadRequest{url:url.clone(),refresh:true,renderer:Renderer::Http,language:default_language(),library:None,selector:None,actor:None};
                async move{
                    let result=tokio::select!{
                        r=async{engine.pace(&origin,delay).await;engine.read(request).await}=>Some(r),
                        _=token.cancelled()=>None,
                    };(url,depth,result)
                }
            }).buffer_unordered(self.config.crawl_concurrency);
            while let Some((url,depth,result))=results.next().await{
                let Some(result)=result else{continue;};job.visited+=1;
                match result{
                    Ok(result)=>{
                        let d=result.document;
                        if !fetch::validated_url(&d.source.resolved).is_ok_and(|u|u.origin()==base.origin() && robots.allowed(&robot_target(&u))) || d.blocks.is_empty() {
                            job.failed+=1;
                            job.warnings.push(Warning::new("crawl_unusable_document",format!("{url}: empty or out-of-scope result was not attached")));
                            job.updated_at=Utc::now().to_rfc3339();self.store.put_job(job.clone()).await?;
                            continue;
                        }
                        if !d.warnings.is_empty(){job.warnings.push(Warning::new("document_warnings",format!("{} has {} extraction warnings",d.id,d.warnings.len())));}
                        if let Some(name)=&job.request.library{self.store.add(name,&d.id,job.request.actor.clone()).await?;}
                        if !job.document_ids.contains(&d.id){job.document_ids.push(d.id.clone());}
                        for link in d.links{
                                let Ok(next)=fetch::validated_url(&link.url)else{continue;};
                                if next.origin()!=base.origin(){continue;}
                                if seen.contains(next.as_str()){continue;}
                                // Bound all discovered URLs, including exclusions, not
                                // only candidates that eventually enter the frontier.
                                if seen.len()>=10000 {
                                    if !job.warnings.iter().any(|w|w.code=="frontier_limit") {
                                        job.warnings.push(Warning::new("frontier_limit","URL discovery stopped at 10000 candidates."));
                                    }
                                    break;
                                }
                                seen.insert(next.to_string());
                                if !robots.allowed(&robot_target(&next)) {
                                    job.warnings.push(Warning::new("robots_excluded",next.to_string()));
                                    continue;
                                }
                                if depth>=job.request.max_depth {
                                    if !job.warnings.iter().any(|w|w.code=="depth_limit_reached") {
                                        job.warnings.push(Warning::new("depth_limit_reached","Links beyond the configured traversal depth were not followed."));
                                    }
                                    continue;
                                }
                                frontier.push_back((next.to_string(),depth+1));
                        }
                    },Err(e)=>{job.failed+=1;job.warnings.push(Warning::new("crawl_read_failed",format!("{url}: {e:#}")));},
                }
                // Publish each completion, not the slowest sibling in the batch.
                job.updated_at=Utc::now().to_rfc3339();self.store.put_job(job.clone()).await?;
            }
        }
        if token.is_cancelled(){job.state=JobState::Cancelled;}
        else{
            if !frontier.is_empty(){job.warnings.push(Warning::new("page_limit_reached","The configured page budget was reached. This is not a complete-site archive."));}
            if job.document_ids.is_empty() && job.visited>0 {
                job.state=JobState::Failed;
                job.error=Some("No usable documents were saved from the completed crawl attempts.".into());
            } else {
                job.state=if job.warnings.iter().all(|w|w.code=="crawl_scope"){JobState::Complete}else{JobState::Partial};
            }
        }
        job.updated_at=Utc::now().to_rfc3339();self.store.put_job(job).await
    }
    pub async fn map(&self,url:&str,limit:usize)->Result<serde_json::Value>{
        if !(1..=5000).contains(&limit){bail!("map limit must be between 1 and 5000");}
        let _permit=self.network.acquire().await?;
        let result=fetch::http(&self.client,url,self.config.max_bytes).await?;
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

fn robot_target(url:&Url)->String{
    match url.query(){Some(q)=>format!("{}?{q}",url.path()),None=>url.path().into()}
}

#[derive(Default)]struct Group{agents:Vec<String>,rules:Vec<(bool,String)>,delay:Option<f64>}
struct Robots{rules:Vec<(bool,String)>,delay:Duration,excessive_delay:bool}
impl Robots{
    fn parse(source:&str)->Self{
        let mut groups=Vec::new();let mut group=Group::default();
        for raw in source.lines(){
            let line=raw.split('#').next().unwrap_or("").trim();let Some((key,value))=line.split_once(':')else{continue;};
            let value=value.trim();
            match key.trim().to_ascii_lowercase().as_str(){
                "user-agent"=>{
                    if !group.rules.is_empty()||group.delay.is_some(){groups.push(group);group=Group::default();}
                    group.agents.push(value.to_ascii_lowercase());
                },
                "allow"|"disallow"=>{if !group.agents.is_empty()&&!value.is_empty(){group.rules.push((key.trim().eq_ignore_ascii_case("allow"),value.into()));}},
                "crawl-delay"=>group.delay=value.parse::<f64>().ok().filter(|n|n.is_finite()&&*n>=0.0),_=>{},
            }
        }
        groups.push(group);
        let score=|g:&Group|g.agents.iter().filter_map(|a|if a=="*"{Some(0)}else if "webtool".contains(a){Some(a.len())}else{None}).max();
        let best=groups.iter().filter_map(score).max();let mut rules=vec![];let mut delay:f64=0.5;let mut excessive_delay=false;
        for g in groups{if score(&g).is_some()&&score(&g)==best{rules.extend(g.rules);if let Some(d)=g.delay{if d>60.0 { excessive_delay=true; } else { delay=delay.max(d); }}}}
        Self{rules,delay:Duration::from_secs_f64(delay),excessive_delay}
    }
    fn allowed(&self,target:&str)->bool{
        let mut matched:Option<(usize,bool)>=None;
        for (allow,pattern) in &self.rules{
            let anchored=pattern.ends_with('$');let raw=if anchored{&pattern[..pattern.len()-1]}else{pattern};
            let expression=format!("^{}{}",raw.split('*').map(regex::escape).collect::<Vec<_>>().join(".*"),if anchored{"$"}else{""});
            if Regex::new(&expression).is_ok_and(|r|r.is_match(target)){
                let size=raw.chars().filter(|c|*c!='*').count();
                if matched.is_none_or(|(n,a)|size>n||(size==n&&*allow&&!a)){matched=Some((size,*allow));}
            }
        }
        matched.map(|(_,a)|a).unwrap_or(true)
    }
}
async fn load_robots(client:&reqwest::Client,origin:&str,max:usize)->Result<Robots>{
    let response=client.get(format!("{origin}/robots.txt")).send().await.context("retrieve robots.txt")?;
    match response.status().as_u16(){
        404|410=>return Ok(Robots::parse("")),401|403=>return Ok(Robots::parse("User-agent: *\nDisallow: /")),
        200=>{},code=>bail!("robots.txt returned HTTP {code}; crawl stopped rather than assuming permission"),
    }
    if response.content_length().is_some_and(|n|n>max as u64){bail!("robots.txt exceeds size limit");}
    let mut bytes=Vec::new();let mut body=response.bytes_stream();
    while let Some(part)=body.next().await{let part=part?;if bytes.len()+part.len()>max{bail!("robots.txt exceeds size limit");}bytes.extend_from_slice(&part);}
    let robots=Robots::parse(std::str::from_utf8(&bytes)?);
    if robots.excessive_delay { bail!("robots.txt requests a crawl delay over 60 seconds; crawl stopped rather than shortening it"); }
    Ok(robots)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn longest_allow_wins(){let r=Robots::parse("User-agent: *\nDisallow: /private\nAllow: /private/public\n");assert!(!r.allowed("/private/a"));assert!(r.allowed("/private/public/a"));}
    #[test]fn specific_agent_overrides_wildcard(){let r=Robots::parse("User-agent: *\nDisallow: /\nUser-agent: webtool\nAllow: /\n");assert!(r.allowed("/x"));}
    #[test]fn wildcard_and_end_anchor(){let r=Robots::parse("User-agent: *\nDisallow: /*.pdf$\n");assert!(!r.allowed("/a.pdf"));assert!(r.allowed("/a.pdf?x=1"));}
    #[test]fn empty_disallow_allows(){assert!(Robots::parse("User-agent: *\nDisallow:\n").allowed("/"));}
}
