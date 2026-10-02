use std::collections::HashMap;
use webtool_protocol::*;
mod organic;
#[cfg(feature="web-search")]
mod service;
#[cfg(feature="web-search")]
pub(crate) use service::SearchService;
#[cfg(not(feature="web-search"))]
#[derive(Clone)]
pub(crate) struct SearchService;
#[cfg(not(feature="web-search"))]
impl SearchService {
    pub(crate) fn new(_: &crate::config::Config) -> anyhow::Result<Self> { Ok(Self) }
    pub(crate) async fn search(&self, _: SearchRequest) -> anyhow::Result<SearchResponse> {
        anyhow::bail!("this server was built without web-search")
    }
}

pub fn merge(rows:Vec<(String,Vec<SearchResult>)>,limit:usize)->Vec<SearchResult>{
    let mut merged:HashMap<String,SearchResult>=HashMap::new();
    let mut provider_seen:HashMap<String,std::collections::HashSet<String>>=HashMap::new();
    for (provider,items) in rows{
        let seen=provider_seen.entry(provider.clone()).or_default();
        for (index,mut item) in items.into_iter().enumerate(){
            let Ok(url)=crate::fetch::validated_url(&item.url)else{continue;};
            if organic::paid_url(&url){continue;}
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
#[cfg(test)]mod tests{
    use super::*;
    fn r(url:&str)->SearchResult{SearchResult{title:"T".into(),url:url.into(),snippet:"snippet".into(),score:0.0,providers:vec![],document_id:None}}
    #[test]fn duplicates_merge_without_deleting_query_parameters(){let rows=vec![("a".into(),vec![r("https://x.test/a?x=1#part"),r("https://x.test/a?x=2")]),("b".into(),vec![r("https://x.test/a?x=1")])];let out=merge(rows,10);assert_eq!(out.len(),2);assert_eq!(out[0].providers.len(),2);}
    #[test]fn duplicates_from_one_provider_do_not_boost_rank(){let out=merge(vec![("a".into(),vec![r("https://a.test"),r("https://a.test")]),("a".into(),vec![r("https://a.test")])],10);assert_eq!(out[0].score,1.0/61.0);}
    #[test]fn paid_urls_cannot_contribute_merge_votes(){let out=merge(vec![("a".into(),vec![r("https://example.org/?gclid=paid"),r("https://example.org/?topic=ads")])],10);assert_eq!(out.len(),1);assert_eq!(out[0].url,"https://example.org/?topic=ads");}
}
