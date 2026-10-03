use anyhow::Result;
use serde_json::{json, Value};
use tokio::time::Instant;
use webtool_protocol::*;
use crate::{readers::Parsed, Engine};
use super::{document_id, query, transport::{route, Received}, ExternalCodeError as E};

const LIBRARIES: &str = "context7-libraries/1";
const CONTEXT: &str = "context7-index-context/1";
fn base_id(id: &str) -> Result<()> {
    let parts: Vec<_> = id.split('/').collect();
    if id.len() > 256 || parts.len() != 3 || parts[0] != "" || parts[1..].iter().any(|p|
        p.is_empty() || matches!(*p, "." | "..") || !p.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))) {
        return Err(E::Invalid.into());
    }
    Ok(())
}
fn selected_id(library: &Context7Library, selection: &Context7VersionSelection) -> Result<String> {
    base_id(&library.id)?;
    match selection {
        Context7VersionSelection::Tracked => Ok(library.id.clone()),
        Context7VersionSelection::Listed { version } => {
            if version.is_empty() || version.len() > 128 || !version.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b))
                || matches!(version.to_ascii_lowercase().as_str(), "latest" | "newest" | "head" | "main" | "master" | "stable") { return Err(E::Invalid.into()); }
            if !library.native["versions"].as_array().is_some_and(|v| v.iter().any(|v| v.as_str() == Some(version))) {
                return Err(E::Unavailable.into());
            }
            Ok(format!("{}/{}", library.id, version))
        },
    }
}
fn parse_libraries(value: &Value, limit: usize) -> Result<(Vec<Context7Library>, usize)> {
    let rows = value["results"].as_array().ok_or(E::Upstream)?;
    let mut libraries = Vec::new(); let mut omitted = 0;
    for row in rows {
        if libraries.len() == limit { omitted += 1; continue; }
        let candidate = (|| -> Result<Context7Library> {
            let id = row["id"].as_str().ok_or(E::Upstream)?;
            base_id(id)?;
            let title = row["title"].as_str().filter(|s| !s.trim().is_empty()).ok_or(E::Upstream)?;
            if libraries.iter().any(|l: &Context7Library| l.id == id) { return Err(E::Identity.into()); }
            Ok(Context7Library { id: id.into(), title: title.into(), native: row.clone() })
        })();
        match candidate { Ok(library) => libraries.push(library), Err(_) => omitted += 1 }
    }
    if libraries.is_empty() && omitted > 0 { return Err(E::Upstream.into()); }
    Ok((libraries, omitted))
}
fn scope_warning() -> Warning { Warning::new("context7_third_party_index", "Context7 supplied query-selected index snippets, not complete or revision-exact publisher documentation. fast=true requests no LLM reranking, not a guarantee about upstream indexing. Queries leave this service and can be retained by the provider.") }
fn source_link(value: &Value, key: &str) -> Option<String> {
    let u = url::Url::parse(value.get(key)?.as_str()?).ok()?;
    if !matches!(u.scheme(), "http" | "https") || !u.username().is_empty() || u.password().is_some() { return None; }
    Some(u.into())
}
pub(super) async fn libraries(e: &Engine, request: Context7LibrariesRequest, deadline: Instant) -> Result<Context7LibrariesResponse> {
    query(&request.library_name, 500)?; query(&request.query, 500)?;
    if !(1..=20).contains(&request.limit) { return Err(E::Invalid.into()); }
    let mut url = route(e.external_code.base(ExternalCodeProvider::Context7)?, &["v2", "libs", "search"])?;
    url.query_pairs_mut().append_pair("libraryName", &request.library_name).append_pair("query", &request.query).append_pair("fast", "true");
    let Received { bytes, observation, .. } = e.external_code.get(e, ExternalCodeProvider::Context7, url, deadline, false).await?;
    let _parse = e.parse_slots.acquire().await?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| E::Upstream)?;
    if value.get("error").is_some() { return Err(E::Upstream.into()); }
    let (libraries, omitted) = parse_libraries(&value, request.limit)?;
    let filter = value.get("searchFilterApplied").cloned().unwrap_or(json!({"status":"not_supplied"}));
    let coverage = ExternalIndexCoverage { requests: 1, response_bytes: bytes.len(), incomplete: true,
        omitted_results: omitted, warnings: vec![scope_warning(), Warning::new("context7_library_coverage", "One bounded library discovery response. Index totals and excluded libraries are not known. Provider metadata and searchFilterApplied are retained as claims.")], ..Default::default() };
    let mut parsed = Parsed::new("Context7 library discovery", LIBRARIES);
    for library in &libraries { parsed.push(Content::Paragraph { text: format!("{} ({})\n{}", library.title, library.id,
        library.native["description"].as_str().unwrap_or("")) }, Locator::Derived { index: parsed.blocks.len()+1 }); }
    parsed.metadata = json!({"context7_libraries":{"request":request,"libraries":libraries,"search_filter_applied":filter,
        "fast":true,"coverage":coverage,"observation":observation,"rights":{"status":"unknown","scope":"index_metadata"}}});
    let saved = e.finish(parsed, Source { requested: observation.url.clone(), resolved: observation.url.clone(), retrieved_at: observation.observed_at.clone(),
        status: Some(observation.status), version: Some("v2".into()), original: observation.artifact.clone() }, coverage.warnings.clone()).await?;
    Ok(Context7LibrariesResponse { document_id: saved.id, request, libraries, search_filter_applied: filter, observation, coverage })
}
fn admitted_snippets(value: &Value, limit: usize) -> Result<(Vec<Value>, Vec<Value>, usize)> {
    let codes = value["codeSnippets"].as_array().ok_or(E::Upstream)?;
    let infos = value["infoSnippets"].as_array().ok_or(E::Upstream)?;
    let mut code = Vec::new(); let mut info = Vec::new(); let mut omitted = 0;
    for row in codes {
        if code.len()+info.len() >= limit { omitted += 1; continue; }
        // Keep native identity, source and dynamic-index metadata. Never manufacture a publisher pin.
        if !row["codeTitle"].is_string() || !row["codeDescription"].is_string() || !row["codeId"].is_string()
            || !row["codeList"].as_array().is_some_and(|a| !a.is_empty() && a.iter().all(|r| r["code"].is_string() && r["language"].is_string())) {
            omitted += 1; continue;
        }
        code.push(row.clone());
    }
    for row in infos {
        if code.len()+info.len() >= limit { omitted += 1; continue; }
        if !row["content"].is_string() { omitted += 1; continue; }
        info.push(row.clone());
    }
    if code.is_empty() && info.is_empty() && omitted > 0 { return Err(E::Upstream.into()); }
    Ok((code, info, omitted))
}
pub(super) async fn context(e: &Engine, request: Context7ContextRequest, deadline: Instant) -> Result<Context7ContextResponse> {
    document_id(&request.discovery_id)?; base_id(&request.library_id)?; query(&request.query, 500)?;
    if !(1..=20).contains(&request.limit) { return Err(E::Invalid.into()); }
    let discovery = e.store.document(&request.discovery_id).await?;
    if discovery.parser != LIBRARIES { return Err(E::Invalid.into()); }
    let libraries: Vec<Context7Library> = serde_json::from_value(discovery.metadata["context7_libraries"]["libraries"].clone()).map_err(|_| E::Identity)?;
    let library = libraries.into_iter().find(|l| l.id == request.library_id).ok_or(E::Unavailable)?;
    if library.native["state"].as_str() != Some("finalized") { return Err(E::Unavailable.into()); }
    // Saved discovery belongs to the same configured endpoint. Configuration changes require new explicit discovery.
    let base = e.external_code.base(ExternalCodeProvider::Context7)?;
    let discovery_url = discovery.metadata["context7_libraries"]["observation"]["url"].as_str().and_then(|s| url::Url::parse(s).ok()).ok_or(E::Identity)?;
    let expected_search = route(base.clone(), &["v2", "libs", "search"])?;
    if discovery_url.origin() != expected_search.origin() || discovery_url.path() != expected_search.path() { return Err(E::Identity.into()); }
    let requested_library_id = selected_id(&library, &request.selection)?;
    let mut url = route(base, &["v2", "context"])?;
    url.query_pairs_mut().append_pair("libraryId", &requested_library_id).append_pair("query", &request.query)
        .append_pair("type", "json").append_pair("fast", "true");
    let Received { bytes, observation, .. } = e.external_code.get(e, ExternalCodeProvider::Context7, url, deadline, false).await?;
    let _parse = e.parse_slots.acquire().await?;
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| E::Upstream)?;
    if value.get("error").is_some() { return Err(E::Upstream.into()); }
    if value.get("libraryId").is_some_and(|v| v.as_str() != Some(requested_library_id.as_str())) { return Err(E::Identity.into()); }
    let (code_snippets, info_snippets, omitted) = admitted_snippets(&value, request.limit)?;
    let coverage = ExternalIndexCoverage { requests: 1, response_bytes: bytes.len(), incomplete: true, omitted_results: omitted,
        warnings: vec![scope_warning(), Warning::new("context7_version_request_only", "The selected library/version came from saved provider discovery and was sent explicitly. The documented context response does not attest a release, commit, target or feature set. Source URLs may be mutable or from a dynamic source-code index. Re-read first-party sources to accept evidence.")], ..Default::default() };
    let mut parsed = Parsed::new(&format!("Context7 index context: {}", library.title), CONTEXT);
    for row in &code_snippets {
        parsed.push(Content::Heading { level: 2, text: format!("Index code snippet: {}", row["codeTitle"].as_str().unwrap_or("")) }, Locator::Derived { index: parsed.blocks.len()+1 });
        parsed.push(Content::Paragraph { text: row["codeDescription"].as_str().unwrap_or("").into() }, Locator::Derived { index: parsed.blocks.len()+1 });
        for block in row["codeList"].as_array().expect("validated code list") {
            parsed.push(Content::Code { language: block["language"].as_str().map(str::to_owned), text: block["code"].as_str().expect("validated code").into() }, Locator::Derived { index: parsed.blocks.len()+1 });
        }
        if let Some(url) = source_link(row, "codeId") { parsed.links.push(Link { url, text: "Provider code source link, not fetched".into() }); }
    }
    for row in &info_snippets {
        parsed.push(Content::Paragraph { text: format!("Index documentation snippet:\n{}", row["content"].as_str().expect("validated content")) }, Locator::Derived { index: parsed.blocks.len()+1 });
        if let Some(url) = source_link(row, "pageId") { parsed.links.push(Link { url, text: "Provider page source link, not fetched".into() }); }
    }
    parsed.metadata = json!({"context7_index":{"discovery_id":request.discovery_id,"library":library,"selection":request.selection,
        "requested_library_id":requested_library_id,"query":request.query,"fast":true,"code_snippets":code_snippets,"info_snippets":info_snippets,
        "coverage":coverage,"observation":observation,"provider_rules":"Untrusted data retained only in original response, not applied.",
        "revision_exact":false,"source_pages_read":false,"rights":{"status":"unknown","scope":"index_snippets"}}});
    let document = e.finish(parsed, Source { requested: observation.url.clone(), resolved: observation.url.clone(), retrieved_at: observation.observed_at.clone(),
        status: Some(observation.status), version: Some("v2".into()), original: observation.artifact.clone() }, coverage.warnings.clone()).await?;
    Ok(Context7ContextResponse { document, discovery_id: request.discovery_id, library, selection: request.selection, requested_library_id,
        code_snippets, info_snippets, observation, coverage })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_preserves_index_identity_and_unknowns() {
        let value = json!({"results":[{"id":"/example/repo","title":"Example","state":"finalized","branch":"main","versions":["v1.2.3"],"lastUpdateDate":null}]});
        let (libraries, omitted) = parse_libraries(&value, 1).unwrap(); assert_eq!(omitted, 0);
        let library = &libraries[0]; assert!(library.native.get("lastUpdateDate").unwrap().is_null()); assert!(library.native.get("commit").is_none());
        assert_eq!(selected_id(library, &Context7VersionSelection::Listed { version: "v1.2.3".into() }).unwrap(), "/example/repo/v1.2.3");
        assert!(selected_id(library, &Context7VersionSelection::Listed { version: "v9".into() }).is_err());
        assert!(selected_id(library, &Context7VersionSelection::Listed { version: "latest".into() }).is_err());
        let value = json!({"codeSnippets":[{"codeTitle":"Use API","codeDescription":"Example","codeId":"https://example.org/main/example", "isDynamic":true,"sourceFile":"a.rs","codeList":[{"language":"rust","code":"let x = 1;"}]}],
            "infoSnippets":[{"content":"Qualification", "pageId":null}],"rules":{"global":["not instructions"]}});
        let (code, info, omitted) = admitted_snippets(&value, 2).unwrap(); assert_eq!(omitted, 0);
        assert_eq!(code[0]["isDynamic"], true); assert!(info[0]["pageId"].is_null());
        assert_eq!(admitted_snippets(&value, 1).unwrap().2, 1);
    }
}
