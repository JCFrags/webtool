//! Bounded robots handling, not a claim of complete RFC 9309 conformance.
use std::time::Duration;
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use regex::Regex;
use url::Url;
use super::frontier::SITEMAP_LIMIT;

pub(super) const MAX_BYTES: usize = 512 * 1024;
#[derive(Default)]
struct Group { agents: Vec<String>, rules: Vec<(bool,String)>, delay: Option<f64>, rules_started: bool }
struct Rule { allow: bool, pattern: Regex, octets: usize }
pub(super) struct Robots {
    rules: Vec<Rule>,
    pub delay: Duration,
    pub sitemaps: Vec<String>,
    pub sitemap_truncated: bool,
}
pub(super) fn target(url: &Url) -> String {
    match url.query() { Some(q)=>format!("{}?{q}",url.path()), None=>url.path().into() }
}
/// Decode only percent-encoded unreserved ASCII. Keep reserved escapes distinct,
/// canonicalize escape case, and encode raw UTF-8 octets before matching.
fn normalize(value: &str) -> String {
    let bytes=value.as_bytes(); let mut out=String::new(); let mut i=0;
    while i<bytes.len() {
        if bytes[i]==b'%' && i+2<bytes.len() {
            let hex=|b:u8| (b as char).to_digit(16).map(|v|v as u8);
            if let (Some(a),Some(b))=(hex(bytes[i+1]),hex(bytes[i+2])) {
                let b=a*16+b;
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { out.push(b as char); }
                else { out.push_str(&format!("%{b:02X}")); }
                i+=3; continue;
            }
        }
        let b=bytes[i];
        if b>=128 || b.is_ascii_control() { out.push_str(&format!("%{b:02X}")); }
        else { out.push(b as char); }
        i+=1;
    }
    out
}
impl Robots {
    pub fn parse(source: &str) -> Result<Self> {
        let mut groups=Vec::new(); let mut group=Group::default();
        let mut sitemaps=Vec::new(); let mut sitemap_truncated=false;
        for raw in source.trim_start_matches('\u{feff}').split(['\r','\n']) {
            let line=raw.split('#').next().unwrap_or("").trim();
            let Some((key,value))=line.split_once(':') else { continue; };
            let key=key.trim(); let value=value.trim();
            match key.to_ascii_lowercase().as_str() {
                "user-agent"=>{
                    // Empty allow/disallow still ends the preceding agents list.
                    if group.rules_started { groups.push(group); group=Group::default(); }
                    group.agents.push(value.to_ascii_lowercase());
                },
                "allow"|"disallow"=>{
                    if !group.agents.is_empty() {
                        group.rules_started=true;
                        if value.starts_with('/') { group.rules.push((key.eq_ignore_ascii_case("allow"),value.into())); }
                    }
                },
                "crawl-delay"=>{
                    if !group.agents.is_empty() {
                        if let Ok(d)=value.parse::<f64>() {
                            if d.is_finite() && d>=0.0 { group.delay=Some(group.delay.unwrap_or(0.0).max(d)); }
                        }
                    }
                },
                "sitemap"=>{
                    if sitemaps.len()<SITEMAP_LIMIT { sitemaps.push(value.into()); }
                    else { sitemap_truncated=true; }
                },
                _=>{},
            }
        }
        groups.push(group);
        let specific=groups.iter().any(|g|g.agents.iter().any(|a|a=="webtool"));
        let agent=if specific { "webtool" } else { "*" };
        let mut rules=Vec::new(); let mut delay=0.5_f64;
        for g in groups.into_iter().filter(|g|g.agents.iter().any(|a|a==agent)) {
            if let Some(d)=g.delay {
                if d>60.0 { bail!("robots.txt requests a crawl delay over 60 seconds; crawl stopped rather than shortening it"); }
                delay=delay.max(d);
            }
            for (allow,pattern) in g.rules {
                let anchored=pattern.ends_with('$');
                let raw=normalize(if anchored { &pattern[..pattern.len()-1] } else { &pattern });
                let expression=format!("^{}{}",raw.split('*').map(regex::escape).collect::<Vec<_>>().join(".*"),if anchored { "$" } else { "" });
                rules.push(Rule { allow,pattern:Regex::new(&expression).context("robots rule exceeds matcher bounds; crawl stopped")?,octets:raw.bytes().filter(|b|*b!=b'*').count() });
            }
        }
        Ok(Self { rules,delay:Duration::from_secs_f64(delay),sitemaps,sitemap_truncated })
    }
    pub fn allowed(&self, target: &str) -> bool {
        if target.split('?').next()==Some("/robots.txt") { return true; }
        let target=normalize(target); let mut matched:Option<(usize,bool)>=None;
        for rule in &self.rules {
            if rule.pattern.is_match(&target) && matched.is_none_or(|(n,a)|rule.octets>n || (rule.octets==n && rule.allow && !a)) { matched=Some((rule.octets,rule.allow)); }
        }
        matched.is_none_or(|(_,allow)|allow)
    }
}
/// The caller uses a same-origin redirect client. Cross-authority robots
/// redirects are refused, not treated as a missing robots file.
pub(super) async fn load(client: &reqwest::Client, origin: &str, max: usize) -> Result<Robots> {
    let response=client.get(format!("{origin}/robots.txt")).header(reqwest::header::ACCEPT_ENCODING,"identity").send().await.context("retrieve robots.txt (cross-origin redirects are refused)")?;
    match response.status().as_u16() {
        404|410=>return Robots::parse(""),
        401|403=>return Robots::parse("User-agent: *\nDisallow: /"),
        200=>{},
        code=>bail!("robots.txt returned HTTP {code}; crawl stopped rather than assuming permission"),
    }
    let bytes=bounded_identity_body(response,max.min(MAX_BYTES)).await?;
    Robots::parse(std::str::from_utf8(&bytes).context("robots.txt must be UTF-8; crawl stopped")?)
}
/// Used by metadata requests with automatic decompression disabled. This bounds
/// transferred bodies and rejects gzip files instead of expanding unbounded data.
pub(super) async fn bounded_identity_body(response: reqwest::Response, max: usize) -> Result<Vec<u8>> {
    if response.headers().get(reqwest::header::CONTENT_ENCODING).is_some_and(|v|!v.to_str().is_ok_and(|v|v.eq_ignore_ascii_case("identity"))) { bail!("compressed crawl metadata is unsupported; request was for identity encoding"); }
    if response.content_length().is_some_and(|n|n>max as u64) { bail!("crawl metadata exceeds {max}-byte limit"); }
    let mut bytes=Vec::new(); let mut stream=response.bytes_stream();
    while let Some(part)=stream.next().await {
        let part=part?;
        if bytes.len().saturating_add(part.len())>max { bail!("crawl metadata exceeds {max}-byte limit"); }
        bytes.extend_from_slice(&part);
    }
    if bytes.starts_with(&[0x1f,0x8b]) { bail!("gzip sitemap files are unsupported"); }
    Ok(bytes)
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn rules_groups_and_empty_boundaries() {
        let r=Robots::parse("User-agent: *\nDisallow: /\nUser-agent: webtool\nDisallow:\nUser-agent: other\nDisallow: /\nUser-agent: WEBTOOL\nDisallow: /private\nAllow: /private/public\n").unwrap();
        assert!(r.allowed("/x")); assert!(!r.allowed("/private/a")); assert!(r.allowed("/private/public/a")); assert!(r.allowed("/robots.txt"));
        assert!(!Robots::parse("User-agent: web\nAllow: /\nUser-agent: *\nDisallow: /\n").unwrap().allowed("/x"));
    }
    #[test] fn percent_octets_wildcards_and_ties() {
        let r=Robots::parse("User-agent: *\nDisallow: /café\nAllow: /caf%C3%A9/public\nDisallow: /a%2fb\nDisallow: /%7Eprivate\nDisallow: /*.pdf$\nDisallow: /same\nAllow: /same\n").unwrap();
        assert!(!r.allowed("/caf%c3%a9")); assert!(r.allowed("/café/public"));
        assert!(!r.allowed("/a%2Fb")); assert!(r.allowed("/a/b")); assert!(!r.allowed("/~private"));
        assert!(!r.allowed("/a.pdf")); assert!(r.allowed("/a.pdf?x=1")); assert!(r.allowed("/same"));
    }
}
