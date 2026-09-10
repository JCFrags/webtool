use std::{collections::HashMap,time::{Duration,Instant}};
use anyhow::{bail,Result};
use futures_util::{stream,StreamExt};
use webtool_protocol::*;
use crate::config::Config;

pub fn merge(rows:Vec<(String,Vec<SearchResult>)>,limit:usize)->Vec<SearchResult>{
    let mut merged:HashMap<String,SearchResult>=HashMap::new();
    for (provider,items) in rows{
        let mut seen=std::collections::HashSet::new();
        for (index,mut item) in items.into_iter().enumerate(){
            let Ok(url)=crate::fetch::validated_url(&item.url)else{continue;};
            let key=url.to_string();if !seen.insert(key.clone()){continue;}
            let entry=merged.entry(key.clone()).or_insert_with(||{
                item.url=key;item.score=0.0;item.providers.clear();item
            });
            entry.score+=1.0/(60.0+(index+1) as f64);
            if !entry.providers.contains(&provider){entry.providers.push(provider.clone());}
        }
    }
    let mut results:Vec<_>=merged.into_values().collect();
    results.sort_by(|a,b|b.score.total_cmp(&a.score).then_with(||a.url.cmp(&b.url)));
    results.truncate(limit);results
}
pub async fn search(request:SearchRequest,config:&Config)->Result<SearchResponse>{
    if request.query.trim().is_empty()||request.query.len()>4096{bail!("query must contain 1 to 4096 bytes");}
    if !(1..=50).contains(&request.limit){bail!("search limit must be between 1 and 50");}
    #[cfg(feature="web-search")]{
        use std::sync::Arc;
        use metadata_search_engine_rs::engines::{build_http_client,BraveEngine,DuckDuckGoEngine,StartpageEngine,YahooEngine,SearchEngine};
        let client=Arc::new(build_http_client()?);
        let mut engines:Vec<Arc<dyn SearchEngine>>=Vec::new();
        for name in &config.search_engines{
            let engine:Arc<dyn SearchEngine>=match name.as_str(){
                "duckduckgo"=>Arc::new(DuckDuckGoEngine::new(client.clone())),
                "brave"=>Arc::new(BraveEngine::new(client.clone())),
                "startpage"=>Arc::new(StartpageEngine::new(client.clone())),
                "yahoo"=>Arc::new(YahooEngine::new(client.clone())),_=>continue,
            };engines.push(engine);
        }
        let now=Instant::now();let q=request.query.clone();let limit=request.limit;
        let timeout=config.request_timeout_seconds;
        let pending=engines.into_iter().map(|engine|{
            let q=q.clone();async move{
                let name=engine.name().to_string();
                let result=tokio::time::timeout(Duration::from_secs(timeout),engine.search(&q,limit)).await;
                let result=match result{
                    Ok(Ok(results))=>Ok(results.into_iter().map(|r|SearchResult{title:r.title,url:r.url,snippet:r.snippet.unwrap_or_default(),score:0.0,providers:vec![],document_id:None}).collect::<Vec<_>>()),
                    Ok(Err(e))=>Err(e.to_string()),Err(_)=>Err("provider timeout".into()),
                };(name,result)
            }
        }).collect::<Vec<_>>();
        let answers=stream::iter(pending).buffer_unordered(4).collect::<Vec<_>>().await;
        let mut rows=Vec::new();let mut warnings=Vec::new();
        for (name,result) in answers{
            match result{
                Ok(items)=>{
                    if items.is_empty(){warnings.push(Warning::new("provider_empty",format!("{name} returned no parsed results. This can mean no matches or an upstream page change.")));}
                    rows.push((name,items));
                },Err(e)=>warnings.push(Warning::new("provider_error",format!("{name}: {e}"))),
            }
        }
        Ok(SearchResponse{query:request.query,results:merge(rows,limit),warnings,elapsed_ms:now.elapsed().as_millis() as u64})
    }
    #[cfg(not(feature="web-search"))]{let _=config;bail!("this server was built without web-search");}
}
#[cfg(test)]mod tests{
    use super::*;
    fn r(url:&str)->SearchResult{SearchResult{title:"T".into(),url:url.into(),snippet:"snippet".into(),score:0.0,providers:vec![],document_id:None}}
    #[test]fn duplicates_merge_without_deleting_query_parameters(){let rows=vec![("a".into(),vec![r("https://x.test/a?x=1#part"),r("https://x.test/a?x=2")]),("b".into(),vec![r("https://x.test/a?x=1")])];let out=merge(rows,10);assert_eq!(out.len(),2);assert_eq!(out[0].providers.len(),2);}
    #[test]fn duplicates_from_one_provider_do_not_boost_rank(){let out=merge(vec![("a".into(),vec![r("https://a.test"),r("https://a.test")])],10);assert_eq!(out[0].score,1.0/61.0);}
}
