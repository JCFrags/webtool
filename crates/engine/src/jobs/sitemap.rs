use anyhow::{bail, Context, Result};
use super::frontier::{Kind, PAGE_CANDIDATES, SITEMAP_LIMIT};

pub(super) const MAX_BYTES: usize = 1024 * 1024;
pub(super) struct Sitemap {
    pub kind: Kind,
    pub urls: Vec<String>,
    pub invalid: usize,
    pub truncated: bool,
}
pub(super) fn parse(bytes: &[u8]) -> Result<Sitemap> {
    let text=std::str::from_utf8(bytes).context("sitemap must be UTF-8")?;
    // roxmltree rejects DTDs by default. No external entities are fetched.
    let tree=roxmltree::Document::parse(text).context("invalid sitemap XML")?;
    let root=tree.root_element();
    let namespace=root.tag_name().namespace();
    if namespace.is_some_and(|ns|ns!="http://www.sitemaps.org/schemas/sitemap/0.9") { bail!("unsupported sitemap namespace"); }
    let (kind,item,limit)=match root.tag_name().name() {
        "urlset"=>(Kind::Page,"url",PAGE_CANDIDATES),
        "sitemapindex"=>(Kind::Sitemap,"sitemap",SITEMAP_LIMIT),
        _=>bail!("expected a urlset or sitemapindex, not an HTML page, feed, or text list"),
    };
    let mut urls=Vec::new(); let mut invalid=0; let mut truncated=false;
    for (i,node) in root.children().filter(|n|n.is_element() && n.tag_name().name()==item && n.tag_name().namespace()==namespace).enumerate() {
        if i>=limit { truncated=true; break; }
        let mut locations=node.children().filter(|n|n.is_element() && n.tag_name().name()=="loc" && n.tag_name().namespace()==namespace);
        let loc=locations.next();
        if locations.next().is_some() { invalid+=1; continue; }
        let Some(value)=loc.filter(|n|!n.children().any(|c|c.is_element())).and_then(|n|n.text()).map(str::trim).filter(|s|!s.is_empty() && s.len()<2048) else { invalid+=1; continue; };
        urls.push(value.into());
    }
    Ok(Sitemap { kind,urls,invalid,truncated })
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn only_direct_protocol_locations() {
        let map=parse(br#"<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:x="urn:foreign"><url><loc>https://example.com/?a=1&amp;b=2</loc><x:loc>https://other.invalid/</x:loc></url><url><loc>a</loc><loc>b</loc></url></urlset>"#).unwrap();
        assert_eq!(map.kind,Kind::Page); assert_eq!(map.urls,vec!["https://example.com/?a=1&b=2"]); assert_eq!(map.invalid,1);
        assert!(parse(b"<!DOCTYPE urlset [<!ENTITY x 'value'>]><urlset/>").is_err());
    }
}
