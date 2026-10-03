//! Exact docs.rs releases. This is not a package-version resolver or build checker.
use anyhow::Result;
use scraper::{Html, Selector};
use serde_json::{json, Value};
use url::Url;
use webtool_protocol::*;
use crate::{code::{self, Budget, CodeError}, readers::Parsed, Engine};

fn selector(value: &str) -> Selector { Selector::parse(value).expect("constant selector") }
fn validate(request: &DocumentationRequest) -> Result<()> {
    if request.crate_name.is_empty() || request.crate_name.len() > 64 ||
        !request.crate_name.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')) {
        return Err(CodeError::Invalid.into());
    }
    // A complete semver release, not docs.rs's range/latest URL convenience.
    let version = regex::Regex::new(r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$").expect("constant regex");
    if request.version.len() > 128 || !version.is_match(&request.version) { return Err(CodeError::Invalid.into()); }
    code::path(&request.path, false)?;
    Ok(())
}
fn url(request: &DocumentationRequest) -> Url {
    let mut url = Url::parse("https://docs.rs/").expect("constant URL");
    let mut segments = url.path_segments_mut().expect("HTTP URL");
    if request.kind == DocumentationKind::Source { segments.push("crate"); }
    segments.push(&request.crate_name).push(&request.version);
    if request.kind == DocumentationKind::Source { segments.push("source"); }
    segments.extend(request.path.split('/'));
    drop(segments);
    url
}
fn same_release(url: &Url, request: &DocumentationRequest) -> bool {
    if url.scheme() != "https" || url.host_str() != Some("docs.rs") || !url.username().is_empty() || url.password().is_some() || url.port().is_some() || url.query().is_some() { return false; }
    let parts: Vec<_> = url.path_segments().into_iter().flatten().collect();
    match request.kind {
        DocumentationKind::Page => parts.len() >= 3 && parts[0] == request.crate_name && parts[1] == request.version,
        DocumentationKind::Source => parts.len() >= 5 && parts[0] == "crate" && parts[1] == request.crate_name && parts[2] == request.version && parts[3] == "source",
    }
}
fn identity(html: &str, request: &DocumentationRequest) -> Result<Value> {
    let document = Html::parse_document(html);
    let nodes: Vec<_> = document.select(&selector("script#crate-metadata")).collect();
    if nodes.len() != 1 { return Err(CodeError::Identity.into()); }
    let metadata: Value = serde_json::from_str(&nodes[0].text().collect::<String>()).map_err(|_| CodeError::Identity)?;
    if metadata["name"].as_str() != Some(&request.crate_name) || metadata["version"].as_str() != Some(&request.version) { return Err(CodeError::Identity.into()); }
    Ok(metadata)
}
fn source(html: &str, name: &str) -> Result<Parsed> {
    let document = Html::parse_document(html);
    let nodes: Vec<_> = document.select(&selector("#source-code pre code")).collect();
    if nodes.len() != 1 { return Err(CodeError::Unavailable.into()); }
    let text = nodes[0].text().collect::<String>();
    if text.trim().is_empty() { return Err(CodeError::Unavailable.into()); }
    let mut parsed = Parsed::new(name, "docs-rs-source-html/1");
    parsed.push(Content::Code { language: name.ends_with(".rs").then(|| "rust".into()), text },
        Locator::Html { selector: "#source-code pre code".into() });
    parsed.warnings.push(Warning::new("docs_source_html", "Source text was decoded from the retained docs.rs HTML code element. It is not a raw crate file, and its lines are not original HTML line numbers."));
    Ok(parsed)
}
impl Engine {
    pub async fn documentation(&self, request: DocumentationRequest) -> Result<DocumentationResponse> {
        self.code_run(async {
            validate(&request)?;
            let requested = url(&request);
            let mut current = requested.clone();
            let mut budget = Budget::new(3, self.config.max_bytes.min(4*1024*1024));
            let mut observations = Vec::new();
            let response = loop {
                let response = self.code_get(current.clone(), &mut budget).await?;
                observations.push(response.observation.clone());
                if (300..400).contains(&response.observation.status) {
                    let next = current.join(response.location.as_deref().ok_or(CodeError::Identity)?).map_err(|_| CodeError::Identity)?;
                    // Validate before making another request. Never turn a missing
                    // release into latest, another crate, or an external website.
                    if !same_release(&next, &request) { return Err(CodeError::Identity.into()); }
                    // A directory landing page is not the requested file. Permit
                    // only a same-path trailing-slash normalization.
                    if next.path().trim_end_matches('/') != current.path().trim_end_matches('/') { return Err(CodeError::Identity.into()); }
                    current = next;
                } else { break response; }
            };
            let html = std::str::from_utf8(&response.bytes).map_err(|_| CodeError::Unsupported)?;
            let metadata = identity(html, &request)?;
            let mut parsed = match request.kind {
                DocumentationKind::Source => source(html, &request.path)?,
                DocumentationKind::Page => {
                    let mut parsed = self.parse(response.bytes.clone(), current.to_string(), "text/html".into(), Some("main".into())).await.map_err(|_| CodeError::Unavailable)?;
                    if parsed.blocks.is_empty() { return Err(CodeError::Unavailable.into()); }
                    parsed.parser = format!("docs-rs-page/1+{}", parsed.parser);
                    parsed
                },
            };
            parsed.metadata["documentation"] = json!({"provider":"docs.rs","crate_name":request.crate_name,"requested_version":request.version,
                "resolved_version":metadata["version"],"kind":request.kind,"path":request.path,"identity_evidence":metadata,
                "observations":observations,"source_commit":null,"target":null,"features":null,
                "rights":{"status":"unknown","scope":"page_or_source","reason":"Public access is not a reuse license. No license assessment was made."}});
            parsed.warnings.push(Warning::new("docs_build_scope", "The release is explicit. The build target/features and source commit are not established. Reading a page does not prove that its examples build in your environment."));
            let document = self.finish(parsed, Source { requested: requested.into(), resolved: current.into(), retrieved_at: chrono::Utc::now().to_rfc3339(),
                status: Some(response.observation.status), version: Some(request.version), original: response.observation.artifact }, vec![]).await?;
            Ok(DocumentationResponse { document, coverage: budget.coverage })
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_release_and_source_dom_are_required() {
        let mut request = DocumentationRequest { crate_name: "hex".into(), version: "0.4.3".into(), path: "src/lib.rs".into(), kind: DocumentationKind::Source };
        assert!(validate(&request).is_ok());
        assert!(same_release(&url(&request), &request));
        assert!(!same_release(&Url::parse("https://docs.rs/crate/hex/latest/source/src/lib.rs").unwrap(), &request));
        let html = r#"<script id="crate-metadata">{"name":"hex","version":"0.4.3"}</script><pre id="line-numbers">1</pre><div id="source-code"><pre><code>fn x() { a &lt; b; }\n</code></pre></div>"#;
        assert!(identity(html, &request).is_ok());
        assert_eq!(source(html, "lib.rs").unwrap().blocks[0].content.text(), "fn x() { a < b; }\\n");
        request.version = "latest".into();
        assert!(validate(&request).is_err());
        assert!(identity(html, &request).is_err());
    }
}
