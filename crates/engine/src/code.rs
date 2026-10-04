//! Bounded public GitHub navigation. No credentials, clones, or remote code execution.
use std::{collections::{HashSet, VecDeque}, future::Future, sync::Arc, time::Duration};
use anyhow::Result;
use base64::Engine as _;
use chrono::Utc;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::{sync::Mutex, time::Instant};
use url::Url;
use webtool_protocol::*;
use crate::{config::Config, readers::{self, Parsed}, sources::{commit_fields, endpoint, pinned}, Engine};

mod github;
mod comparison;
mod navigation;

pub const MAP_PARSER: &str = "github-bounded-map/1";
const FILE_PARSER: &str = "github-exact-file/1";
const API_VERSION: &str = "2022-11-28";

/// New domain errors have fixed public messages. Unknown storage/programming
/// failures remain internal errors at the server boundary.
#[derive(Debug, thiserror::Error, Clone, Copy)]
pub enum CodeError {
    #[error("Use an explicit public repository/ref or exact docs.rs release and valid bounded options.")] Invalid,
    #[error("Authenticated GitHub code search is unavailable. Explicitly select paths or literal search instead.")] SearchUnavailable,
    #[error("The requested public source, version, path, or documentation build is unavailable.")] Unavailable,
    #[error("The provider denied public access. No retry or alternate source was used.")] Denied,
    #[error("The public provider rate limit was reached. No retry was made.")] RateLimited,
    #[error("The provider response did not match the selected source identity.")] Identity,
    #[error("The operation reached its explicit byte, file, entry, or request budget.")] Limit,
    #[error("The source is not a supported regular UTF-8 file or documentation page.")] Unsupported,
    #[error("The public provider request or response failed.")] Upstream,
    #[error("The code/documentation operation deadline was reached.")] Timeout,
}
impl CodeError {
    pub fn message(self) -> &'static str { match self {
        Self::Invalid => "Use an explicit public repository/ref or exact docs.rs release and valid bounded options.",
        Self::SearchUnavailable => "Authenticated GitHub code search is unavailable. Explicitly select paths or literal search instead.",
        Self::Unavailable => "The requested public source, version, path, or documentation build is unavailable.",
        Self::Denied => "The provider denied public access. No retry or alternate source was used.",
        Self::RateLimited => "The public provider rate limit was reached. No retry was made.",
        Self::Identity => "The provider response did not match the selected source identity.",
        Self::Limit => "The operation reached its explicit byte, file, entry, or request budget.",
        Self::Unsupported => "The source is not a supported regular UTF-8 file or documentation page.",
        Self::Upstream => "The public provider request or response failed.",
        Self::Timeout => "The code/documentation operation deadline was reached.",
    } }
    pub fn status_code(self) -> u16 {
        match self { Self::Invalid => 400, Self::SearchUnavailable | Self::Unsupported => 422,
            Self::Unavailable => 404, Self::RateLimited => 429, Self::Limit => 413,
            Self::Timeout => 504, _ => 502 }
    }
    pub fn code(self) -> &'static str {
        match self { Self::Invalid => "code_invalid_request", Self::SearchUnavailable => "code_search_unavailable",
            Self::Unavailable => "code_source_unavailable", Self::Denied => "code_access_denied",
            Self::RateLimited => "code_rate_limited", Self::Identity => "code_identity_mismatch",
            Self::Limit => "code_limit", Self::Unsupported => "code_unsupported", Self::Upstream => "code_upstream_error",
            Self::Timeout => "code_timeout" }
    }
    pub fn problem(self) -> Problem { Problem { code: self.code().into(), message: self.to_string() } }
}

#[derive(Clone)]
pub(crate) struct CodeService {
    client: reqwest::Client,
    github_gate: Arc<Mutex<Gate>>,
    docs_gate: Arc<Mutex<Gate>>,
}
struct Gate { next: Instant, blocked_until: Instant }
impl CodeService {
    pub fn new(config: &Config) -> Result<Self> {
        // A separate unauthenticated transport makes redirect and credential
        // policy explicit. It shares this engine's network/operation capacity.
        let client = reqwest::Client::builder().user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.request_timeout_seconds))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none()).retry(reqwest::retry::never()).build()?;
        let gate = || Arc::new(Mutex::new(Gate { next: Instant::now(), blocked_until: Instant::now() }));
        Ok(Self { client, github_gate: gate(), docs_gate: gate() })
    }
}

pub(crate) struct Budget { pub coverage: CodeCoverage, max_requests: usize, max_bytes: usize }
impl Budget {
    pub fn new(max_requests: usize, max_bytes: usize) -> Self { Self { coverage: CodeCoverage::default(), max_requests, max_bytes } }
    fn remaining(&self) -> usize { self.max_bytes.saturating_sub(self.coverage.response_bytes) }
    fn stop(&mut self, error: CodeError) { self.coverage.incomplete = true; self.coverage.stopped = Some(error.problem()); }
}
pub(crate) struct Response { pub bytes: Vec<u8>, pub observation: CodeObservation, pub location: Option<String>, pub pagination: Option<String> }
impl Response {
    fn json(&self) -> Result<Value> { serde_json::from_slice(&self.bytes).map_err(|_| CodeError::Upstream.into()) }
}
fn number(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.parse().ok()
}
fn status_error(status: u16, remaining: Option<u64>) -> Option<CodeError> {
    match status {
        200..=299 | 301 | 302 | 303 | 307 | 308 => None,
        429 => Some(CodeError::RateLimited),
        403 if remaining == Some(0) => Some(CodeError::RateLimited),
        401 | 403 => Some(CodeError::Denied),
        404 | 410 | 409 => Some(CodeError::Unavailable),
        422 => Some(CodeError::Invalid),
        _ => Some(CodeError::Upstream),
    }
}
impl Engine {
    pub(crate) async fn code_run<T>(&self, future: impl Future<Output = Result<T>>) -> Result<T> {
        tokio::time::timeout(Duration::from_secs(self.config.request_timeout_seconds), async {
            let _operation = self.operation_slots.acquire().await?;
            future.await
        }).await.map_err(|_| CodeError::Timeout)?
    }
    pub(crate) async fn code_get(&self, url: Url, budget: &mut Budget) -> Result<Response> {
        let github = url.host_str() == Some("api.github.com");
        if !github && url.host_str() != Some("docs.rs") { return Err(CodeError::Invalid.into()); }
        if budget.coverage.requests >= budget.max_requests || budget.remaining() == 0 { return Err(CodeError::Limit.into()); }
        let mut gate = if github { self.code.github_gate.lock().await } else { self.code.docs_gate.lock().await };
        if gate.blocked_until > Instant::now() { return Err(CodeError::RateLimited.into()); }
        tokio::time::sleep_until(gate.next).await;
        let _network = self.network.acquire().await?;
        budget.coverage.requests += 1;
        let mut request = self.code.client.get(url.clone());
        if github { request = request.header("Accept", "application/vnd.github+json").header("X-GitHub-Api-Version", API_VERSION); }
        gate.next = Instant::now() + Duration::from_secs(if url.path() == "/search/repositories" { 6 } else { 1 });
        let response = request.send().await.map_err(|_| CodeError::Upstream)?;
        let status = response.status().as_u16();
        let remaining = number(response.headers(), "x-ratelimit-remaining");
        let reset = number(response.headers(), "x-ratelimit-reset");
        if matches!(status, 401 | 403 | 429) || remaining == Some(0) {
            let reset_wait = reset.unwrap_or(0).saturating_sub(Utc::now().timestamp().max(0) as u64);
            let wait = number(response.headers(), "retry-after").unwrap_or(60).max(reset_wait).clamp(60, 86400);
            gate.blocked_until = Instant::now() + Duration::from_secs(wait);
        }
        if let Some(error) = status_error(status, remaining) { return Err(error.into()); }
        if github && (300..400).contains(&status) { return Err(CodeError::Identity.into()); }
        let location = response.headers().get("location").and_then(|v| v.to_str().ok()).map(str::to_owned);
        let pagination = response.headers().get("link").and_then(|v| v.to_str().ok()).map(str::to_owned);
        let limit = budget.remaining().min(self.config.max_bytes);
        if response.content_length().is_some_and(|n| n > limit as u64) { return Err(CodeError::Limit.into()); }
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| CodeError::Upstream)?;
            if bytes.len().saturating_add(chunk.len()) > limit {
                budget.coverage.response_bytes += bytes.len();
                return Err(CodeError::Limit.into());
            }
            bytes.extend_from_slice(&chunk);
        }
        budget.coverage.response_bytes += bytes.len();
        let artifact = self.store.put_bytes(&bytes, if github { "application/json" } else { "text/html" },
            if github { "github_api_response" } else { "docs_rs_response" }).await?;
        Ok(Response { bytes, location, pagination, observation: CodeObservation { url: url.into(), status, artifact,
            rate_remaining: remaining, rate_reset_unix: reset } })
    }
    pub async fn code_discover(&self, request: RepositoryDiscoverRequest) -> Result<RepositoryDiscoverResponse> {
        self.code_run(discover(self, request)).await
    }
    pub async fn code_map(&self, request: RepositoryMapRequest) -> Result<RepositoryMapResponse> {
        self.code_run(map(self, request)).await
    }
    pub async fn code_search(&self, request: CodeSearchRequest) -> Result<CodeSearchResponse> {
        self.code_run(search(self, request)).await
    }
    pub async fn code_file(&self, request: CodeFileRequest) -> Result<CodeFileResponse> {
        self.code_run(async {
            validate_files(&request.limits)?;
            let map = saved_map(self, &request.map_id).await?;
            let entry = admitted(&map, &request.path)?;
            let mut budget = file_budget(&request.limits);
            let document = file(self, &map, entry, &request.limits, &mut budget).await?;
            Ok(CodeFileResponse { document, coverage: budget.coverage })
        }).await
    }
}
fn repository(name: &str) -> Result<(&str, &str)> {
    let (owner, repo) = name.split_once('/').ok_or(CodeError::Invalid)?;
    for part in [owner, repo] {
        if part.is_empty() || part.len() > 100 || matches!(part, "." | "..") ||
            !part.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)) { return Err(CodeError::Invalid.into()); }
    }
    Ok((owner, repo))
}
pub(crate) fn path(value: &str, empty: bool) -> Result<()> {
    if value.is_empty() && empty { return Ok(()); }
    if value.is_empty() || value.len() > 1024 || value.split('/').count() > 16 ||
        value.split('/').any(|p| p.is_empty() || matches!(p, "." | "..")) ||
        value.chars().any(|c| c.is_control() || matches!(c, '\\' | '?' | '#' | '%')) { return Err(CodeError::Invalid.into()); }
    Ok(())
}
fn sha(value: &str) -> Result<()> {
    if value.len() != 40 || !value.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(CodeError::Identity.into()); }
    Ok(())
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> { value[key].as_str().ok_or_else(|| CodeError::Upstream.into()) }
fn validate_map(request: &RepositoryMapRequest) -> Result<()> {
    repository(&request.repository)?;
    path(&request.path, true)?;
    if request.reference.is_empty() || request.reference.len() > 256 || request.reference.chars().any(char::is_control) ||
        !(1..=4).contains(&request.limits.depth) || !(1..=1000).contains(&request.limits.max_entries) ||
        !(2..=20).contains(&request.limits.max_requests) || !(1024..=8*1024*1024).contains(&request.limits.max_bytes) {
        return Err(CodeError::Invalid.into());
    }
    Ok(())
}
fn validate_files(limits: &FileLimits) -> Result<()> {
    if !(1..=20).contains(&limits.max_files) || !(1..=20).contains(&limits.max_requests) ||
        !(1..=1024*1024).contains(&limits.max_file_bytes) || !(1..=4*1024*1024).contains(&limits.max_bytes) {
        return Err(CodeError::Invalid.into());
    }
    Ok(())
}
fn file_budget(limits: &FileLimits) -> Budget { Budget::new(limits.max_requests, limits.max_bytes.saturating_mul(2) + limits.max_files * 8192) }
async fn discover(e: &Engine, request: RepositoryDiscoverRequest) -> Result<RepositoryDiscoverResponse> {
    if request.query.trim().is_empty() || request.query.len() > 256 || !(1..=20).contains(&request.limit) { return Err(CodeError::Invalid.into()); }
    let mut url = endpoint(&["search", "repositories"]);
    url.query_pairs_mut().append_pair("q", &request.query).append_pair("per_page", &request.limit.to_string()).append_pair("page", "1");
    let response = e.code_get(url, &mut Budget::new(1, 1024*1024)).await?;
    let value = response.json()?;
    let mut results = Vec::new();
    let items = value["items"].as_array().ok_or(CodeError::Upstream)?;
    for item in items.iter().take(request.limit) {
        if item["private"].as_bool() != Some(false) { return Err(CodeError::Identity.into()); }
        let name = string(item, "full_name")?;
        let (owner, repo) = repository(name)?;
        results.push(RepositoryHit { repository: name.into(), url: format!("https://github.com/{owner}/{repo}"),
            description: item["description"].as_str().map(str::to_owned), default_branch: string(item, "default_branch")?.into(),
            license: item.get("license").cloned().unwrap_or(json!({"status":"not_supplied"})) });
    }
    let incomplete = value["incomplete_results"].as_bool() != Some(false) || value["total_count"].as_u64().is_none_or(|n| n > results.len() as u64);
    let warnings = vec![Warning::new("repository_discovery_only", "One public search page. Descriptions and license metadata are provider claims, not fetched code or file-specific reuse permission.")];
    let mut parsed = Parsed::new(&format!("Repository discovery: {}", request.query), "github-repository-discovery/1");
    for hit in &results { parsed.push(Content::Paragraph { text: format!("{}\n{}\n{}", hit.repository, hit.url, hit.description.as_deref().unwrap_or("")) }, Locator::Derived { index: parsed.blocks.len()+1 }); }
    parsed.metadata = json!({"query":request.query,"incomplete":incomplete,"scope":"discovery_only","api_version":API_VERSION});
    let source = Source { requested: response.observation.url.clone(), resolved: response.observation.url.clone(),
        retrieved_at: Utc::now().to_rfc3339(), status: Some(response.observation.status), version: Some(API_VERSION.into()), original: response.observation.artifact.clone() };
    let document = e.finish(parsed, source, warnings.clone()).await?;
    Ok(RepositoryDiscoverResponse { document_id: document.id, query: request.query, observed_at: document.source.retrieved_at,
        total_count: value["total_count"].as_u64(), incomplete, results, observation: response.observation, warnings })
}
fn entries(value: &Value) -> Result<&Vec<Value>> { value["tree"].as_array().ok_or_else(|| CodeError::Upstream.into()) }
fn regular(entry: &RepositoryEntry) -> bool { entry.kind == "blob" && matches!(entry.mode.as_str(), "100644" | "100755") }
fn tree_entry(value: &Value, prefix: &str, owner: &str, repo: &str, commit: &str) -> Result<RepositoryEntry> {
    let name = string(value, "path")?;
    path(name, false)?;
    if name.contains('/') { return Err(CodeError::Identity.into()); }
    let full = if prefix.is_empty() { name.into() } else { format!("{prefix}/{name}") };
    let kind = string(value, "type")?;
    let object = string(value, "sha")?;
    sha(object)?;
    Ok(RepositoryEntry { path: full.clone(), kind: kind.into(), mode: string(value, "mode")?.into(), object_sha: object.into(),
        size: value["size"].as_u64(), url: pinned(owner, repo, if kind == "tree" { "tree" } else { "blob" }, commit, &full) })
}
async fn tree(e: &Engine, owner: &str, repo: &str, object: &str, budget: &mut Budget, observations: &mut Vec<CodeObservation>) -> Result<Value> {
    let response = e.code_get(endpoint(&["repos", owner, repo, "git", "trees", object]), budget).await?;
    let value = response.json()?;
    if string(&value, "sha")? != object { return Err(CodeError::Identity.into()); }
    observations.push(response.observation);
    if value["truncated"].as_bool() != Some(false) {
        budget.coverage.incomplete = true;
        budget.coverage.warnings.push(Warning::new("github_tree_incomplete", "A provider tree is truncated or lacks a completeness flag."));
    }
    Ok(value)
}
async fn map(e: &Engine, request: RepositoryMapRequest) -> Result<RepositoryMapResponse> {
    validate_map(&request)?;
    let (owner, repo) = repository(&request.repository)?;
    let mut budget = Budget::new(request.limits.max_requests, request.limits.max_bytes);
    let response = e.code_get(endpoint(&["repos", owner, repo, "commits", &request.reference]), &mut budget).await?;
    let (commit, root_tree) = commit_fields(&response.json()?).map_err(|_| CodeError::Identity)?;
    sha(&root_tree)?;
    if request.reference.len() == 40 && request.reference.bytes().all(|b| b.is_ascii_hexdigit()) && !commit.eq_ignore_ascii_case(&request.reference) { return Err(CodeError::Identity.into()); }
    let mut observations = vec![response.observation];
    let mut current = tree(e, owner, repo, &root_tree, &mut budget, &mut observations).await?;
    for component in request.path.split('/').filter(|p| !p.is_empty()) {
        let item = entries(&current)?.iter().find(|v| v["path"].as_str() == Some(component)).ok_or(
            if current["truncated"].as_bool() == Some(false) { CodeError::Unavailable } else { CodeError::Limit })?;
        if item["type"] != "tree" || item["mode"] != "040000" { return Err(CodeError::Unsupported.into()); }
        let object = string(item, "sha")?.to_owned(); sha(&object)?;
        current = tree(e, owner, repo, &object, &mut budget, &mut observations).await?;
    }
    let mut queue = VecDeque::from([(request.path.clone(), 1usize, current)]);
    let mut pending = VecDeque::new();
    let mut admitted = Vec::new();
    loop {
        if let Some((prefix, depth, listing)) = queue.pop_front() {
            for item in entries(&listing)? {
                if admitted.len() == request.limits.max_entries { budget.stop(CodeError::Limit); break; }
                let entry = tree_entry(item, &prefix, owner, repo, &commit)?;
                if entry.kind == "tree" && entry.mode == "040000" {
                    if depth < request.limits.depth { pending.push_back((entry.path.clone(), depth+1, entry.object_sha.clone())); }
                    else { budget.coverage.incomplete = true; }
                }
                admitted.push(entry);
            }
        }
        if budget.coverage.stopped.is_some() { break; }
        let Some((prefix, depth, object)) = pending.pop_front() else { break; };
        match tree(e, owner, repo, &object, &mut budget, &mut observations).await {
            Ok(listing) => queue.push_back((prefix, depth, listing)),
            Err(error) => match error.downcast_ref::<CodeError>() { Some(error) => { budget.stop(*error); break; }, None => return Err(error) },
        }
    }
    if budget.coverage.incomplete { budget.coverage.warnings.push(Warning::new("bounded_map", "Coverage is limited to listed entries. Depth, admission, provider, or request limits left some repository paths unvisited.")); }
    let map = RepositoryMap { repository: request.repository.clone(), requested_ref: request.reference.clone(), resolved_commit: commit.clone(),
        root_tree, path: request.path.clone(), limits: request.limits, entries: admitted, observations, coverage: budget.coverage };
    // This manifest is explicitly derived. Each original API response is retained
    // separately by hash. Do not call the manifest an original repository file.
    let manifest = serde_json::to_vec(&map)?;
    let original = e.store.put_bytes(&manifest, "application/json", "derived_repository_map").await?;
    let url = pinned(owner, repo, "tree", &commit, &request.path);
    let source = Source { requested: pinned(owner, repo, "tree", &request.reference, &request.path), resolved: url,
        retrieved_at: Utc::now().to_rfc3339(), status: None, version: Some(commit), original };
    let mut parsed = Parsed::new(&format!("{} repository map", request.repository), MAP_PARSER);
    parsed.push(Content::Paragraph { text: format!("Bounded map at {}. File contents were not read.", map.resolved_commit) }, Locator::Derived { index: 1 });
    for item in &map.entries { parsed.push(Content::Paragraph { text: format!("{} {} {}\n{}", item.mode, item.kind, item.path, item.url) }, Locator::Derived { index: parsed.blocks.len()+1 }); }
    parsed.metadata = json!({"code_map":map,"rights":{"status":"unknown","scope":"files","reason":"No revision-specific license assessment was made."}});
    let document = e.finish(parsed, source, map.coverage.warnings.clone()).await?;
    let map = serde_json::from_value(document.metadata["code_map"].clone())?;
    Ok(RepositoryMapResponse { document_id: document.id, map })
}
async fn saved_map(e: &Engine, id: &str) -> Result<RepositoryMap> {
    let document = e.store.document(id).await?;
    if document.parser != MAP_PARSER { return Err(CodeError::Invalid.into()); }
    Ok(serde_json::from_value(document.metadata["code_map"].clone())?)
}
fn admitted<'a>(map: &'a RepositoryMap, name: &str) -> Result<&'a RepositoryEntry> {
    path(name, false)?;
    let entry = map.entries.iter().find(|entry| entry.path == name).ok_or(CodeError::Unavailable)?;
    if !regular(entry) { return Err(CodeError::Unsupported.into()); }
    Ok(entry)
}
fn blob(value: &Value, expected: &str, max: usize) -> Result<Vec<u8>> {
    if value["sha"].as_str() != Some(expected) || value["encoding"] != "base64" { return Err(CodeError::Identity.into()); }
    if value["size"].as_u64().is_none_or(|n| n > max as u64) { return Err(CodeError::Limit.into()); }
    let encoded: Vec<u8> = string(value, "content")?.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_| CodeError::Upstream)?;
    if bytes.len() > max || value["size"].as_u64() != Some(bytes.len() as u64) { return Err(CodeError::Identity.into()); }
    Ok(bytes)
}
async fn file(e: &Engine, map: &RepositoryMap, entry: &RepositoryEntry, limits: &FileLimits, budget: &mut Budget) -> Result<Document> {
    if budget.coverage.files_read >= limits.max_files { return Err(CodeError::Limit.into()); }
    let max = limits.max_file_bytes.min(limits.max_bytes.saturating_sub(budget.coverage.file_bytes)).min(e.config.max_bytes);
    if entry.size.is_none_or(|n| n > max as u64) { return Err(CodeError::Limit.into()); }
    let (owner, repo) = repository(&map.repository)?;
    let response = e.code_get(endpoint(&["repos", owner, repo, "git", "blobs", &entry.object_sha]), budget).await?;
    let bytes = blob(&response.json()?, &entry.object_sha, max)?;
    if entry.size != Some(bytes.len() as u64) { return Err(CodeError::Identity.into()); }
    let original = e.store.put_bytes(&bytes, "text/plain", "repository_file").await?;
    budget.coverage.files_read += 1;
    budget.coverage.file_bytes += bytes.len();
    let text = std::str::from_utf8(&bytes).map_err(|_| CodeError::Unsupported)?;
    if text.contains('\0') { return Err(CodeError::Unsupported.into()); }
    // Plain parsing is deliberate: notebook/HTML/Markdown source is code here,
    // not an instruction to execute cells or select rendered document content.
    let mut parsed = readers::text::plain(text, &entry.path);
    parsed.parser = FILE_PARSER.into();
    parsed.metadata = json!({"code":{"repository":map.repository,"requested_ref":map.requested_ref,
        "resolved_commit":map.resolved_commit,"path":entry.path,"blob_sha":entry.object_sha,"tree_sha":map.root_tree,
        "api":response.observation,"line_separator":"LF","rights":{"status":"unknown","scope":"file","notices":"Retained verbatim in original bytes; not interpreted as a license grant."}}});
    e.finish(parsed, Source { requested: entry.url.clone(), resolved: entry.url.clone(), retrieved_at: Utc::now().to_rfc3339(),
        status: Some(response.observation.status), version: Some(map.resolved_commit.clone()), original }, vec![]).await
}
fn literal_matches(text: &str, query: &str, entry: &RepositoryEntry, id: &str, limit: usize) -> (Vec<CodeMatch>, bool) {
    let mut matches = Vec::new();
    for (offset, matched) in text.match_indices(query) {
        if matches.len() == limit { return (matches, true); }
        let mut start = offset.saturating_sub(100);
        let mut end = (offset + matched.len() + 200).min(text.len());
        while !text.is_char_boundary(start) { start += 1; }
        while !text.is_char_boundary(end) { end -= 1; }
        let line = text.as_bytes()[..offset].iter().filter(|b| **b == b'\n').count()+1;
        matches.push(CodeMatch { path: entry.path.clone(), blob_sha: entry.object_sha.clone(), url: format!("{}#L{line}", entry.url),
            document_id: id.into(), line, byte_range: [offset, offset+matched.len()], excerpt: text[start..end].into() });
    }
    (matches, false)
}
async fn search(e: &Engine, request: CodeSearchRequest) -> Result<CodeSearchResponse> {
    if request.mode == CodeSearchMode::GithubCode { return Err(CodeError::SearchUnavailable.into()); }
    if request.query.is_empty() || request.query.len() > 4096 || !(1..=50).contains(&request.limit) { return Err(CodeError::Invalid.into()); }
    validate_files(&request.limits)?;
    let path_mode = matches!(request.mode, CodeSearchMode::Paths | CodeSearchMode::PathGlob);
    if !path_mode && (request.paths.is_empty() || request.paths.len() > 20 || request.paths.len() > request.limits.max_files) { return Err(CodeError::Invalid.into()); }
    if path_mode && !request.paths.is_empty() { return Err(CodeError::Invalid.into()); }
    let path_glob = if request.mode == CodeSearchMode::PathGlob { Some(navigation::glob(&request.query)?) } else { None };
    if request.mode == CodeSearchMode::Regex { navigation::pattern(&request.query)?; }
    let map = saved_map(e, &request.map_id).await?;
    let mut coverage = CodeCoverage { incomplete: map.coverage.incomplete, warnings: map.coverage.warnings.clone(), ..Default::default() };
    let mut paths = Vec::new(); let mut matches = Vec::new(); let mut files = Vec::new();
    if path_mode {
        for entry in map.entries.iter().filter(|v| path_glob.as_ref().map_or_else(|| v.path.contains(&request.query), |glob| glob.is_match(&v.path))) {
            if paths.len() == request.limit { coverage.incomplete = true; coverage.warnings.push(Warning::new("match_limit", "Additional admitted path matches were omitted.")); break; }
            paths.push(entry.clone());
        }
    } else {
        let mut seen = HashSet::new();
        let selected = request.paths.iter().map(|name| {
            if !seen.insert(name) { return Err(CodeError::Invalid.into()); }
            let entry = admitted(&map, name)?;
            if request.mode == CodeSearchMode::Symbols { navigation::language(&entry.path)?; }
            Ok(entry)
        }).collect::<Result<Vec<_>>>()?;
        let mut budget = file_budget(&request.limits);
        for entry in selected {
            if budget.coverage.stopped.is_some() {
                files.push(CodeFileOutcome { path: entry.path.clone(), document_id: None, error: budget.coverage.stopped.clone() });
                continue;
            }
            match file(e, &map, entry, &request.limits, &mut budget).await {
                Ok(document) => {
                    // Verify against the retained artifact, not a provider snippet or
                    // potentially normalized parser view. Every match has exact bytes.
                    let bytes = e.store.bytes(&document.source.original).await?;
                    let text = std::str::from_utf8(&bytes).map_err(|_| CodeError::Unsupported)?;
                    let remaining = request.limit.saturating_sub(matches.len());
                    let (found, truncated) = if request.mode == CodeSearchMode::Literal {
                        literal_matches(text, &request.query, entry, &document.id, remaining)
                    } else {
                        let permit = e.parse_slots.clone().acquire_owned().await?;
                        let (text, query, entry, id, mode) = (text.to_owned(), request.query.clone(), entry.clone(), document.id.clone(), request.mode);
                        tokio::task::spawn_blocking(move || { let _permit = permit; navigation::matches(&text, &query, mode, &entry, &id, remaining) }).await??
                    };
                    matches.extend(found);
                    if truncated { budget.coverage.incomplete = true; budget.coverage.warnings.push(Warning::new("match_limit", "Additional exact literal matches were omitted.")); }
                    files.push(CodeFileOutcome { path: entry.path.clone(), document_id: Some(document.id), error: None });
                },
                Err(error) => {
                    let Some(domain) = error.downcast_ref::<CodeError>() else { return Err(error); };
                    files.push(CodeFileOutcome { path: entry.path.clone(), document_id: None, error: Some(domain.problem()) });
                    budget.coverage.incomplete = true;
                    if !matches!(domain, CodeError::Unsupported | CodeError::Unavailable) { budget.stop(*domain); }
                },
            }
        }
        budget.coverage.incomplete |= coverage.incomplete;
        budget.coverage.warnings.extend(coverage.warnings);
        budget.coverage.warnings.push(Warning::new("selected_files_only", "Only explicitly selected admitted files were searched, not the whole repository."));
        if request.mode == CodeSearchMode::Symbols { budget.coverage.warnings.push(Warning::new("lexical_declarations", "Deterministic declaration-name matches for Rust, Python, JavaScript/TypeScript, or Go. Lexical comment/string filtering is heuristic, not compiler symbol resolution, references, or complete declaration coverage.")); }
        if request.mode == CodeSearchMode::Regex { budget.coverage.warnings.push(Warning::new("regex_scope", "Bounded case-sensitive Rust regex matching. Zero-width matches are omitted. Matches address exact retained UTF-8 file bytes.")); }
        coverage = budget.coverage;
    }
    Ok(CodeSearchResponse { map_id: request.map_id, repository: map.repository, requested_ref: map.requested_ref,
        resolved_commit: map.resolved_commit, mode: request.mode, query: request.query, paths, matches, files, coverage })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_literal_offsets_and_boundaries() {
        let entry = RepositoryEntry { path: "a.rs".into(), kind: "blob".into(), mode: "100644".into(), object_sha: "b".repeat(40), size: None, url: "https://github.com/example/repo/blob/commit/a.rs".into() };
        let text = "// café\r\nfn target() {}\n// target";
        let (matches, truncated) = literal_matches(text, "target", &entry, "saved", 1);
        assert!(truncated); assert_eq!(matches[0].line, 2);
        assert_eq!(&text.as_bytes()[matches[0].byte_range[0]..matches[0].byte_range[1]], b"target");
        assert!(path("../private", false).is_err()); assert!(path("a/%2f", false).is_err());
        assert!(repository("owner/repo/other").is_err());
    }
    #[test]
    fn provider_errors_and_blob_identity_stay_explicit() {
        assert!(matches!(status_error(401, None), Some(CodeError::Denied)));
        assert!(matches!(status_error(403, Some(0)), Some(CodeError::RateLimited)));
        assert!(matches!(status_error(429, None), Some(CodeError::RateLimited)));
        let value = json!({"sha":"b".repeat(40),"encoding":"base64","size":3,"content":"YWJj\n"});
        assert_eq!(blob(&value, &"b".repeat(40), 3).unwrap(), b"abc");
        assert!(blob(&value, &"a".repeat(40), 3).is_err());
        assert!(blob(&value, &"b".repeat(40), 2).is_err());
    }
}
