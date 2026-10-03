//! Optional JSON providers. Only selected web results become discovery snippets.
use anyhow::{anyhow, bail, Result};
use reqwest::{header::HeaderValue, Client, RequestBuilder};
use serde::{de::IgnoredAny, Deserialize};
use serde_json::Value;
use webtool_protocol::{SearchRequest, SearchResult, Warning};

use crate::config::{BraveApiConfig, SearxngConfig};
use super::{organic, ProviderResults};

const BRAVE_ENDPOINT: &str = "https://api.search.brave.com/res/v1/web/search";

pub(super) enum JsonProvider {
    Brave(BraveApiConfig),
    Searxng(SearxngConfig),
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(super) struct Failure {
    pub code: &'static str,
    message: &'static str,
}
fn unconfigured(message: &'static str) -> anyhow::Error {
    Failure { code: "provider_unconfigured", message }.into()
}

impl JsonProvider {
    pub(super) fn request(&self, client: &Client, request: &SearchRequest) -> Result<RequestBuilder> {
        match self {
            Self::Brave(config) => {
                if request.query.chars().count() > 600 || request.query.split_whitespace().count() > 75 {
                    return Err(Failure { code: "provider_unsupported", message:
                        "query exceeds Brave API's 600-character or 75-word limit; no request was sent" }.into());
                }
                let name = config.api_key_env.as_deref().ok_or_else(||
                    unconfigured("set search.brave_api.api_key_env to a server environment variable name"))?;
                // Read only the configured reference, only when this provider is selected.
                let key = std::env::var(name).map_err(|_| unconfigured(
                    "the configured server key environment variable is missing or is not Unicode"))?;
                if key.is_empty() || key.len() > 8192 || key.bytes().any(|b| b.is_ascii_whitespace()) {
                    return Err(unconfigured("the configured server key is empty or invalid"));
                }
                let mut token = HeaderValue::from_str(&key)
                    .map_err(|_| unconfigured("the configured server key is not a valid header value"))?;
                token.set_sensitive(true);
                Ok(client.get(BRAVE_ENDPOINT).header("X-Subscription-Token", token)
                    .header("Accept", "application/json").query(&[
                        ("q", request.query.as_str()), ("count", &request.limit.min(20).to_string()),
                        ("result_filter", "web"), ("spellcheck", "false"), ("text_decorations", "false"),
                    ]))
            }
            Self::Searxng(config) => {
                let endpoint = config.endpoint.as_deref().ok_or_else(||
                    unconfigured("set search.searxng.endpoint to an operator-selected JSON-enabled search endpoint"))?;
                Ok(client.get(endpoint).header("Accept", "application/json")
                    .query(&[("q", request.query.as_str()), ("format", "json"), ("categories", "general")]))
            }
        }
    }

    pub(super) async fn search(&self, request: RequestBuilder, limit: usize, max_bytes: usize) -> Result<ProviderResults> {
        // The JSON client follows no redirects. Never return URLs or response bodies
        // in errors, including HTTP 200 error envelopes and serde field errors.
        let mut response = request.send().await.map_err(reqwest::Error::without_url)?
            .error_for_status().map_err(reqwest::Error::without_url)?;
        if response.status().is_redirection() {
            bail!("provider redirect refused (HTTP {})", response.status().as_u16());
        }
        if response.content_length().is_some_and(|n| n > max_bytes as u64) {
            bail!("search response exceeds the {max_bytes}-byte limit");
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(reqwest::Error::without_url)? {
            if body.len().saturating_add(chunk.len()) > max_bytes {
                bail!("search response exceeds the {max_bytes}-byte limit after decompression");
            }
            body.extend_from_slice(&chunk);
        }
        match self {
            Self::Brave(_) => parse_brave(&body, limit),
            Self::Searxng(_) => parse_searxng(&body, limit),
        }
    }
}

#[derive(Deserialize)]
struct BraveResponse {
    #[serde(rename = "type")]
    kind: Option<String>,
    query: Option<BraveQuery>,
    web: Option<BraveWeb>,
    error: Option<IgnoredAny>,
}
#[derive(Deserialize)]
struct BraveQuery { original: String }
#[derive(Deserialize)]
struct BraveWeb {
    #[serde(rename = "type")]
    kind: Option<String>,
    results: Vec<Value>,
}
#[derive(Deserialize)]
struct BraveResult {
    #[serde(rename = "type")]
    kind: Option<String>,
    title: String,
    url: String,
    description: Option<String>,
    #[serde(flatten)]
    paid: PaidSignals,
}
#[derive(Deserialize)]
struct SearxngResponse {
    results: Vec<Value>,
    #[serde(default)]
    unresponsive_engines: Vec<(String, String)>,
    error: Option<IgnoredAny>,
}
#[derive(Deserialize)]
struct SearxngResult {
    template: String,
    #[serde(default)]
    category: String,
    title: String,
    url: String,
    content: Option<String>,
    #[serde(flatten)]
    paid: PaidSignals,
}

// Defensive boolean flags, not an assertion that every upstream exposes these.
// No title/snippet keyword matching or conversion of other result collections.
#[derive(Default, Deserialize)]
#[serde(default)]
struct PaidSignals {
    is_ad: bool,
    is_sponsored: bool,
    sponsored: bool,
    promoted: bool,
}
impl PaidSignals {
    fn excluded(&self) -> bool { self.is_ad || self.is_sponsored || self.sponsored || self.promoted }
}

fn item(title: String, href: String, snippet: Option<String>) -> Result<Option<SearchResult>> {
    if title.trim().is_empty() { bail!("missing result title"); }
    let url = crate::fetch::validated_url(&href).map_err(|_| anyhow!("invalid result URL"))?;
    if organic::paid_url(&url) { return Ok(None); }
    Ok(Some(SearchResult { title, url: url.into(), snippet: snippet.unwrap_or_default(),
        score: 0.0, providers: vec![], document_id: None }))
}

fn finish(provider: &str, items: Vec<SearchResult>, malformed: usize, upstream_failures: usize) -> Result<ProviderResults> {
    if items.is_empty() && (malformed > 0 || upstream_failures > 0) {
        bail!("no usable organic results; {malformed} malformed rows and {upstream_failures} reported upstream failures");
    }
    let mut warnings = Vec::new();
    if malformed > 0 || upstream_failures > 0 {
        warnings.push(Warning::new("provider_partial", format!(
            "{provider}: retained usable results; {malformed} malformed rows and {upstream_failures} reported upstream failures. Upstream error text is withheld.")));
    }
    Ok(ProviderResults { items, warnings })
}

fn parse_brave(body: &[u8], limit: usize) -> Result<ProviderResults> {
    let response: BraveResponse = serde_json::from_slice(body)
        .map_err(|_| anyhow!("invalid Brave API JSON response"))?;
    if response.error.is_some() { bail!("Brave API returned an error envelope; details withheld"); }
    if response.kind.as_deref().is_some_and(|kind| kind != "search") {
        bail!("unexpected Brave API response type");
    }
    let rows = match response.web {
        Some(web) => {
            if web.kind.as_deref().is_some_and(|kind| kind != "search") {
                bail!("unexpected Brave API web result type");
            }
            web.results
        }
        // The documented web collection is optional. Require a recognizable
        // search envelope before interpreting its absence as no web results.
        None if response.kind.as_deref() == Some("search")
            && response.query.is_some_and(|query| !query.original.is_empty()) => Vec::new(),
        None => bail!("Brave API response lacks a web result collection or a search envelope"),
    };
    let mut items = Vec::new();
    let mut malformed = 0;
    for row in rows {
        let Ok(row) = serde_json::from_value::<BraveResult>(row) else { malformed += 1; continue; };
        if row.paid.excluded() || row.kind.as_deref().is_some_and(|kind| kind != "search_result") { continue; }
        match item(row.title, row.url, row.description) {
            Ok(Some(row)) => items.push(row), Ok(None) => {}, Err(_) => malformed += 1,
        }
        if items.len() >= limit.min(20) { break; }
    }
    let mut result = finish("brave_api", items, malformed, 0)?;
    if limit > 20 {
        result.warnings.push(Warning::new("provider_limit",
            "brave_api: one request returns at most 20 results; no extra page was requested."));
    }
    Ok(result)
}

fn parse_searxng(body: &[u8], limit: usize) -> Result<ProviderResults> {
    let response: SearxngResponse = serde_json::from_slice(body)
        .map_err(|_| anyhow!("invalid SearXNG JSON response or missing results array"))?;
    if response.error.is_some() { bail!("SearXNG returned an error envelope; details withheld"); }
    let mut items = Vec::new();
    let mut malformed = 0;
    for row in response.results {
        let Ok(row) = serde_json::from_value::<SearxngResult>(row) else { malformed += 1; continue; };
        if row.paid.excluded() || row.template != "default.html"
            || (!row.category.is_empty() && row.category != "general") { continue; }
        match item(row.title, row.url, row.content) {
            Ok(Some(row)) => items.push(row), Ok(None) => {}, Err(_) => malformed += 1,
        }
        if items.len() >= limit { break; }
    }
    // The endpoint already merged its engines. Do not turn its engines, positions,
    // or score fields into independent ranking votes or claims of direct retrieval.
    finish("searxng", items, malformed, response.unresponsive_engines.len())
}
