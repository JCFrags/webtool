pub mod html;
pub mod text;
pub mod captions;
pub mod document;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use webtool_protocol::*;

#[derive(Debug)]
pub struct Parsed {
    pub title: String,
    pub parser: String,
    pub blocks: Vec<Block>,
    pub links: Vec<Link>,
    pub metadata: Value,
    pub warnings: Vec<Warning>,
}
impl Parsed {
    pub fn new(title: &str, parser: &str) -> Self {
        Self { title:title.into(),parser:parser.into(),blocks:vec![],links:vec![],metadata:json!({}),warnings:vec![] }
    }
    pub fn push(&mut self, content: Content, locator: Locator) {
        self.blocks.push(Block{id:format!("b{}",self.blocks.len()+1),content,locator});
    }
}

pub fn extension(name: &str) -> String {
    let path = name.split('?').next().unwrap_or(name).split('#').next().unwrap_or(name);
    path.rsplit('.').next().unwrap_or("").to_ascii_lowercase()
}
pub fn detect(name:&str, content_type:Option<&str>, bytes:&[u8])->String {
    if bytes.starts_with(b"%PDF-") { return "application/pdf".into(); }
    if let Some(kind) = infer::get(bytes) {
        if kind.mime_type().starts_with("image/") || kind.mime_type().starts_with("audio/") || kind.mime_type().starts_with("video/") {
            return kind.mime_type().into();
        }
    }
    let ext=extension(name);
    let known=match ext.as_str() {
        "html"|"htm"=>Some("text/html"), "md"|"markdown"=>Some("text/markdown"),
        "csv"=>Some("text/csv"), "tsv"=>Some("text/tab-separated-values"),
        "json"=>Some("application/json"), "jsonl"|"ndjson"=>Some("application/x-ndjson"),
        "ipynb"=>Some("application/x-ipynb+json"), "srt"=>Some("application/x-subrip"),
        "vtt"=>Some("text/vtt"),"sbv"=>Some("text/x-sbv"),
        "rss"=>Some("application/rss+xml"),"atom"=>Some("application/atom+xml"),
        "xml"=>Some("application/xml"),
        "docx"=>Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
        "pptx"=>Some("application/vnd.openxmlformats-officedocument.presentationml.presentation"),
        "xlsx"=>Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        "epub"=>Some("application/epub+zip"),
        "odt"=>Some("application/vnd.oasis.opendocument.text"),
        "ods"=>Some("application/vnd.oasis.opendocument.spreadsheet"),
        "odp"=>Some("application/vnd.oasis.opendocument.presentation"),
        "eml"=>Some("message/rfc822"),_=>None,
    };
    let ct=content_type.unwrap_or("").split(';').next().unwrap_or("").trim().to_lowercase();
    // A generic HTTP media type must not override a meaningful file extension.
    if !ct.is_empty() && !["application/octet-stream","binary/octet-stream","text/plain"].contains(&ct.as_str()) { return ct; }
    if let Some(k)=known { return k.into(); }
    if let Ok(s)=std::str::from_utf8(bytes) {
        let prefix=s.trim_start().chars().take(300).collect::<String>().to_ascii_lowercase();
        if prefix.starts_with("<!doctype html")||prefix.starts_with("<html") { return "text/html".into(); }
        if prefix.starts_with("webvtt") { return "text/vtt".into(); }
        if prefix.contains("<rss") { return "application/rss+xml".into(); }
        if prefix.contains("<feed") { return "application/atom+xml".into(); }
        if serde_json::from_str::<Value>(s).is_ok() { return "application/json".into(); }
        if s.contains('\0') { return "application/octet-stream".into(); }
        return "text/plain".into();
    }
    infer::get(bytes).map(|t|t.mime_type().to_string()).unwrap_or_else(||"application/octet-stream".into())
}

/// Pure input parsing. CPU work is invoked on a bounded blocking pool by Engine.
pub fn parse(bytes:&[u8], name:&str, mime:&str, selector:Option<&str>)->Result<Parsed> {
    if mime=="text/html"||mime=="application/xhtml+xml" {
        return html::parse(bytes,name,selector);
    }
    if matches!(mime,"application/rss+xml"|"application/atom+xml") {
        let feed=feed_rs::parser::parse(bytes).context("parse RSS or Atom feed")?;
        let title=feed.title.map(|v|v.content).unwrap_or_else(||name.into());
        let mut p=Parsed::new(&title,"feed-rs/2");
        for (i,entry) in feed.entries.into_iter().enumerate() {
            let title=entry.title.map(|v|v.content).unwrap_or_else(||entry.id.clone());
            p.push(Content::Heading{level:2,text:title.clone()},Locator::Derived{index:i+1});
            if let Some(summary)=entry.summary { p.push(Content::Paragraph{text:summary.content},Locator::Derived{index:i+1}); }
            for link in entry.links { p.links.push(Link{url:link.href,text:title.clone()}); }
        }
        p.warnings.push(Warning::new("feed_entries_only","Feed entries are not full destination articles."));
        return Ok(p);
    }
    let s=std::str::from_utf8(bytes).context("input is not UTF-8 text; use the document reader for binary formats")?;
    if s.contains('\0') { bail!("text input contains NUL bytes; unsupported encoding or binary format"); }
    match mime {
        "text/vtt"|"application/x-subrip"|"text/x-sbv"=>captions::parse(s,name),
        "text/csv"|"text/tab-separated-values"=>text::csv(s,name,if mime=="text/csv"{b','}else{b'\t'}),
        "application/json"=>text::json(s,name),
        "application/x-ndjson"=>text::jsonl(s,name),
        "application/x-ipynb+json"=>text::notebook(s,name),
        "text/markdown"=>Ok(text::markdown(s,name)),
        "application/xml"|"text/xml"=>text::xml(s,name),
        m if m.starts_with("text/")=>Ok(text::plain(s,name)),
        _=>bail!("unsupported format {mime}; install a server built with the documents feature"),
    }
}
pub fn is_document_format(mime:&str)->bool {
    mime=="application/pdf"||mime.starts_with("image/")||mime.starts_with("application/vnd.")||
    matches!(mime,"application/epub+zip"|"message/rfc822")
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn pdf_magic_beats_misleading_type() { assert_eq!(detect("x.txt",Some("text/plain"),b"%PDF-1.7"),"application/pdf"); }
    #[test] fn generic_type_does_not_hide_csv() { assert_eq!(detect("x.csv",Some("application/octet-stream"),b"a,b"),"text/csv"); }
    #[test] fn rejects_binary_as_text() { assert!(parse(&[0,255],"x","text/plain",None).is_err()); }
    #[test] fn json_sniff() { assert_eq!(detect("upload",None,b"{\"a\":1}"),"application/json"); }
}
