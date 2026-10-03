use anyhow::Result;
use serde_json::{json, Value};
use tokio::time::Instant;
use webtool_protocol::*;
use crate::{readers::Parsed, Engine};
use super::{document_id, query, transport::{route, Received}, ExternalCodeError as E};

pub(super) const PARSER: &str = "sourcegraph-index/1";
fn repository(value: &str) -> Result<()> {
    if value.len() > 256 || value.split('/').count() < 3 || value.split('/').any(|s| s.is_empty() || matches!(s, "." | ".."))
        || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b)) { return Err(E::Invalid.into()); }
    Ok(())
}
fn commit(value: &str) -> bool { value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) }
fn quoted(value: &str) -> String { serde_json::to_string(value).expect("string serialization") }
fn provider_query(request: &SourcegraphSearchRequest) -> Result<String> {
    query(&request.query, 500)?;
    if !(1..=20).contains(&request.limit) { return Err(E::Invalid.into()); }
    if matches!(request.mode, SourcegraphMode::Symbol | SourcegraphMode::Regexp) { return Err(E::Unsupported.into()); }
    if let Some(repo) = &request.repository { repository(repo)?; }
    if let Some(reference) = &request.reference {
        if request.repository.is_none() || reference.is_empty() || reference.len() > 128
            || !reference.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b)) { return Err(E::Unsupported.into()); }
    }
    let mut filters = vec![format!("count:{}", request.limit), "fork:no".into(), "archived:no".into()];
    if let Some(repo) = &request.repository {
        let scope = format!("^{}$", regex::escape(repo));
        filters.push(format!("repo:{}{}", scope, request.reference.as_ref().map(|r| format!("@{r}")).unwrap_or_default()));
    }
    if let Some(path) = &request.path { query(path, 256)?; filters.push(format!("file:{}", quoted(&regex::escape(path)))); }
    if let Some(language) = &request.language {
        query(language, 64)?;
        filters.push(format!("lang:{}", quoted(language)));
    }
    match request.mode {
        SourcegraphMode::Path => {
            filters.push("type:path".into());
            filters.push(format!("file:{}", quoted(&regex::escape(&request.query))));
        },
        SourcegraphMode::Literal | SourcegraphMode::Regexp => {
            filters.push("type:file".into());
            filters.push(format!("content:{}", quoted(&request.query)));
        },
        SourcegraphMode::Symbol => unreachable!(),
    }
    Ok(filters.join(" "))
}
fn indexed_url(e: &Engine, repository: &str, path: &str, sha: Option<&str>) -> Result<String> {
    let base = e.external_code.base(ExternalCodeProvider::Sourcegraph)?;
    let mut parts: Vec<String> = repository.split('/').map(str::to_owned).collect();
    if let Some(sha) = sha { let last = parts.last_mut().expect("repository validated"); *last = format!("{last}@{sha}"); }
    parts.extend(["-".into(), "blob".into()]);
    parts.extend(path.split('/').map(str::to_owned));
    Ok(route(base, &parts.iter().map(String::as_str).collect::<Vec<_>>())?.into())
}
struct Stream {
    hits: Vec<Value>, progress: Vec<Value>, alerts: Vec<Value>, filters: Vec<Value>, done: bool, incomplete: bool,
}
fn stream(bytes: &[u8]) -> Stream {
    let mut result = Stream { hits: vec![], progress: vec![], alerts: vec![], filters: vec![], done: false, incomplete: false };
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(error) => { result.incomplete = true; std::str::from_utf8(&bytes[..error.valid_up_to()]).expect("valid prefix") },
    }.replace("\r\n", "\n");
    for frame in text.split_inclusive("\n\n") {
        if !frame.ends_with("\n\n") { if !frame.trim().is_empty() { result.incomplete = true; } break; }
        let mut kind = None;
        let mut data = Vec::new();
        for line in frame.lines() {
            if let Some(value) = line.strip_prefix("event:") { kind = Some(value.trim()); }
            if let Some(value) = line.strip_prefix("data:") { data.push(value.trim_start()); }
        }
        if kind.is_none() && data.is_empty() { continue; }
        if result.done { result.incomplete = true; break; }
        let Ok(value) = serde_json::from_str::<Value>(&data.join("\n")) else { result.incomplete = true; break; };
        match kind {
            Some("matches") => if let Some(rows) = value.as_array() { result.hits.extend(rows.iter().cloned()); } else { result.incomplete = true; },
            Some("progress") if value.is_object() => result.progress.push(value),
            Some("alert") => { result.incomplete = true; result.alerts.push(value); },
            Some("filters") => result.filters.push(value),
            Some("done") => result.done = true,
            _ => result.incomplete = true,
        }
    }
    if !result.done || result.progress.last().and_then(|p| p["done"].as_bool()) != Some(true) { result.incomplete = true; }
    if result.progress.iter().any(|p| p.get("skipped").is_some_and(|s| !s.is_array() || s.as_array().is_some_and(|s| !s.is_empty()))) { result.incomplete = true; }
    result
}
fn hit(value: &Value, request: &SourcegraphSearchRequest) -> Result<SourcegraphHit> {
    let kind = value["type"].as_str().ok_or(E::Upstream)?;
    if !matches!(kind, "content" | "path") { return Err(E::Unsupported.into()); }
    let repo = value["repository"].as_str().ok_or(E::Upstream)?;
    repository(repo)?;
    if request.repository.as_deref().is_some_and(|r| r != repo) { return Err(E::Identity.into()); }
    let path = value["path"].as_str().ok_or(E::Upstream)?;
    crate::code::path(path, false).map_err(|_| E::Unsupported)?;
    let sha = match value.get("commit") {
        Some(Value::String(s)) if commit(s) => Some(s.clone()),
        Some(Value::String(_)) => return Err(E::Identity.into()),
        Some(Value::Null) | None => None,
        _ => return Err(E::Upstream.into()),
    };
    if request.reference.as_deref().is_some_and(commit) && sha.as_deref() != request.reference.as_deref() { return Err(E::Identity.into()); }
    let mut lines = Vec::new();
    if kind == "content" {
        for line in value["lineMatches"].as_array().ok_or(E::Unsupported)? {
            let text = line["line"].as_str().ok_or(E::Upstream)?;
            let number = line["lineNumber"].as_u64().and_then(|n| usize::try_from(n).ok()).ok_or(E::Upstream)?;
            if text.contains('\n') || text.len() > 256*1024 { return Err(E::Unsupported.into()); }
            lines.push(SourcegraphLine { text: text.into(), line_number_zero_based: number,
                offset_and_lengths: line.get("offsetAndLengths").cloned().unwrap_or(json!({"status":"not_supplied"})) });
        }
    }
    Ok(SourcegraphHit { kind: kind.into(), repository: repo.into(), path: path.into(), commit: sha, lines,
        indexed_url: String::new(), native: value.clone() })
}
pub(super) async fn search(e: &Engine, request: SourcegraphSearchRequest, deadline: Instant) -> Result<SourcegraphSearchResponse> {
    let provider_query = provider_query(&request)?;
    let mut url = route(e.external_code.base(ExternalCodeProvider::Sourcegraph)?, &[".api", "search", "stream"])?;
    url.query_pairs_mut().append_pair("q", &provider_query).append_pair("v", "V3")
        .append_pair("t", if request.mode == SourcegraphMode::Regexp { "regexp" } else { "standard" })
        .append_pair("cm", "false").append_pair("display", &request.limit.to_string());
    let Received { bytes, observation, stopped } = e.external_code.get(e, ExternalCodeProvider::Sourcegraph, url, deadline, true).await?;
    let _parse = e.parse_slots.acquire().await?;
    let parsed = stream(&bytes);
    let mut coverage = ExternalIndexCoverage { requests: 1, response_bytes: bytes.len(),
        incomplete: parsed.incomplete || stopped.is_some(), stopped, ..Default::default() };
    let mut hits = Vec::new();
    for value in &parsed.hits {
        if hits.len() >= request.limit { coverage.omitted_results += 1; coverage.incomplete = true; continue; }
        match hit(value, &request) {
            Ok(mut h) => {
                h.indexed_url = indexed_url(e, &h.repository, &h.path, h.commit.as_deref())?;
                if h.commit.is_none() { coverage.warnings.push(Warning::new("index_commit_unknown", "An index hit lacks an immutable commit and cannot be source-verified.")); }
                hits.push(h);
            },
            Err(error) => {
                let error = *error.downcast_ref::<E>().ok_or(E::Upstream)?;
                coverage.omitted_results += 1; coverage.incomplete = true;
                if matches!(error, E::Identity) { return Err(error.into()); }
                coverage.warnings.push(Warning::new("index_result_unsupported", "An unsupported or malformed index result was omitted. Raw admitted stream remains available."));
            },
        }
    }
    if hits.is_empty() && (!parsed.done || coverage.stopped.is_some() || coverage.omitted_results > 0) { return Err(E::Upstream.into()); }
    coverage.warnings.push(Warning::new("index_snippets_not_source", "Only this configured instance's index was searched. Repository fetch dates, skips and alerts are provider claims. Snippets are not fetched or verified files."));
    let mut document = Parsed::new("Sourcegraph indexed matches", PARSER);
    for h in &hits {
        document.push(Content::Paragraph { text: format!("Index snippet: {} {} at {}\n{}", h.repository, h.path,
            h.commit.as_deref().unwrap_or("unknown commit"), h.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n")) }, Locator::Derived { index: document.blocks.len()+1 });
        document.links.push(Link { url: h.indexed_url.clone(), text: h.path.clone() });
    }
    document.metadata = json!({"sourcegraph_index":{"request":request,"provider_query":provider_query,"hits":hits,
        "progress":parsed.progress,"alerts":parsed.alerts,"filters":parsed.filters,"stream_done":parsed.done,"coverage":coverage,
        "verification":"not_read","rights":{"status":"unknown","scope":"indexed_content"}},"observation":observation});
    let saved = e.finish(document, Source { requested: observation.url.clone(), resolved: observation.url.clone(),
        retrieved_at: observation.observed_at.clone(), status: Some(observation.status), version: Some("V3".into()), original: observation.artifact.clone() }, coverage.warnings.clone()).await?;
    Ok(SourcegraphSearchResponse { document_id: saved.id, request, provider_query, hits, progress: parsed.progress,
        alerts: parsed.alerts, filters: parsed.filters, stream_done: parsed.done, observation, coverage })
}
fn verified_lines(hit: &SourcegraphHit, bytes: &[u8]) -> Result<Vec<VerifiedIndexLine>> {
    if hit.kind != "content" || hit.lines.is_empty() { return Err(E::Unsupported.into()); }
    let text = std::str::from_utf8(bytes).map_err(|_| E::Unsupported)?;
    let mut starts = vec![0];
    for (i, b) in bytes.iter().enumerate() { if *b == b'\n' { starts.push(i+1); } }
    let mut result = Vec::new();
    for line in &hit.lines {
        let start = *starts.get(line.line_number_zero_based).ok_or(E::Identity)?;
        let end = starts.get(line.line_number_zero_based+1).map(|n| n-1).unwrap_or(bytes.len());
        // LF defines lines. Exclude the CR in a CRLF terminator, not any other whitespace.
        let end = if end > start && bytes[end-1] == b'\r' { end-1 } else { end };
        if text[start..end] != line.text { return Err(E::Identity.into()); }
        result.push(VerifiedIndexLine { line: line.line_number_zero_based+1, byte_range: [start, end], text: line.text.clone() });
    }
    Ok(result)
}
pub(super) async fn verify(e: &Engine, request: SourcegraphVerifyRequest) -> Result<SourcegraphVerifyResponse> {
    for id in [&request.search_id, &request.map_id, &request.file_id] { document_id(id)?; }
    let search = e.store.document(&request.search_id).await?;
    let map = e.store.document(&request.map_id).await?;
    let file = e.store.document(&request.file_id).await?;
    if search.parser != PARSER || map.parser != crate::code::MAP_PARSER || file.parser != "github-exact-file/1"
        || file.source.original.role != "repository_file" { return Err(E::Unsupported.into()); }
    let hit: SourcegraphHit = serde_json::from_value(search.metadata["sourcegraph_index"]["hits"].as_array()
        .and_then(|h| h.get(request.hit)).cloned().ok_or(E::Invalid)?).map_err(|_| E::Identity)?;
    let sha = hit.commit.as_deref().filter(|s| commit(s)).ok_or(E::Unsupported)?;
    let repo = hit.repository.strip_prefix("github.com/").ok_or(E::Unsupported)?;
    let map: RepositoryMap = serde_json::from_value(map.metadata["code_map"].clone()).map_err(|_| E::Identity)?;
    let entry = map.entries.iter().find(|v| v.path == hit.path).ok_or(E::Identity)?;
    let identity = &file.metadata["code"];
    if map.repository != repo || map.resolved_commit != sha || entry.kind != "blob"
        || !matches!(entry.mode.as_str(), "100644" | "100755")
        || identity["repository"].as_str() != Some(repo) || identity["path"].as_str() != Some(hit.path.as_str())
        || identity["blob_sha"].as_str() != Some(entry.object_sha.as_str())
        || entry.size != Some(file.source.original.size) { return Err(E::Identity.into()); }
    let bytes = e.store.bytes(&file.source.original).await?;
    let lines = verified_lines(&hit, &bytes)?;
    Ok(SourcegraphVerifyResponse { search_id: request.search_id, hit: request.hit, map_id: request.map_id, file_id: request.file_id,
        repository: hit.repository, commit: sha.into(), path: hit.path, blob_sha: entry.object_sha.clone(), original: file.source.original,
        lines, warnings: vec![Warning::new("selected_saved_file_only", "Index lines were compared with retained first-party file bytes using the selected map's commit/blob identity. No network request or whole-index validation was made. Rights remain unknown.")] })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_query_stream_and_exact_line_verification() {
        let request = SourcegraphSearchRequest { query: "repo:other OR count:all".into(), mode: SourcegraphMode::Literal,
            repository: Some("github.com/example/repo".into()), reference: Some("a".repeat(40)), path: None, language: None, limit: 5 };
        let q = provider_query(&request).unwrap();
        assert!(q.contains("content:\"repo:other OR count:all\"")); assert!(q.contains("count:5"));
        let mut bad = request.clone(); bad.reference = Some("main:other".into()); assert!(provider_query(&bad).is_err());
        let value = json!({"type":"content","repository":"github.com/example/repo","path":"src/lib.rs","commit":"a".repeat(40),
            "repoLastFetched":null,"lineMatches":[{"line":"fn café() {}","lineNumber":1,"offsetAndLengths":[[3,4]]}]});
        let hit = hit(&value, &request).unwrap();
        let bytes = "// top\r\nfn café() {}\r\n".as_bytes();
        let verified = verified_lines(&hit, bytes).unwrap(); assert_eq!(verified[0].line, 2);
        assert_eq!(&bytes[verified[0].byte_range[0]..verified[0].byte_range[1]], "fn café() {}".as_bytes());
        assert!(verified_lines(&hit, b"// top\nfn stale() {}\n").is_err());
        let s = format!("event: matches\r\ndata: [{}]\r\n\r\nevent: progress\r\ndata: {{\"done\":true,\"skipped\":[{{\"reason\":\"excluded-archive\"}}]}}\r\n\r\nevent: done\r\ndata: {{}}\r\n\r\n", value);
        let parsed = stream(s.as_bytes()); assert!(parsed.done && parsed.incomplete); assert_eq!(parsed.hits.len(), 1);
        assert!(!stream(b"event: matches\ndata: []\n\n").done);
    }
}
