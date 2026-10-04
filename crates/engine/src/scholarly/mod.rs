//! Explicit, source-backed scholarly discovery. No provider fanout or LLM calls.
mod parse;
mod transport;
pub mod pmc;
use std::time::Duration;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use webtool_protocol::*;
use crate::{arxiv, config::Config, readers::Parsed, Engine};
use transport::{ProviderClient, Response};

const PARSER: &str = "scholarly-metadata/1";
const CACHE_SECONDS: u64 = 86400;
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ScholarlyError {
    #[error("Use an explicit supported scholarly provider, a query of 1 to 4096 bytes, and a limit from 1 to 20.")]
    InvalidRequest,
    #[error("Use a DOI such as 10.1234/example.")]
    InvalidDoi,
    #[error("Use a literal arXiv identifier with an explicit vN.")]
    InvalidArxiv,
    #[error("The selected scholarly provider reached its rate or budget limit. No retry was made.")]
    RateLimited,
    #[error("The scholarly operation reached its overall deadline, including queueing.")]
    Timeout,
    #[error("The provider response exceeds the configured scholarly byte limit.")]
    TooLarge,
    #[error("The scholarly provider denied access. No alternate provider was tried.")]
    AccessDenied,
    #[error("The selected scholarly record or version is unavailable.")]
    NotFound,
    #[error("The scholarly provider request failed. No retry or fallback was made.")]
    ProviderFailed,
    #[error("The provider returned invalid or contradictory scholarly metadata.")]
    Malformed,
}
impl ScholarlyError {
    pub fn code(&self) -> &'static str { match self {
        Self::InvalidRequest | Self::InvalidDoi | Self::InvalidArxiv => "scholarly_invalid_request",
        Self::RateLimited => "scholarly_rate_limited", Self::Timeout => "scholarly_timeout",
        Self::TooLarge => "scholarly_size_limit", Self::AccessDenied => "scholarly_access_denied",
        Self::NotFound => "scholarly_not_found", Self::ProviderFailed => "scholarly_provider_failed",
        Self::Malformed => "scholarly_invalid_metadata",
    } }
    pub fn http_status(&self) -> u16 { match self {
        Self::InvalidRequest | Self::InvalidDoi | Self::InvalidArxiv => 400,
        Self::RateLimited => 429, Self::Timeout => 504, Self::TooLarge => 413,
        Self::NotFound => 404, Self::AccessDenied | Self::ProviderFailed | Self::Malformed => 502,
    } }
    pub fn message(&self) -> &'static str { match self {
        Self::InvalidRequest => "Use arxiv or openalex search, a query of 1 to 4096 bytes, and a limit from 1 to 20.",
        Self::InvalidDoi => "Use a DOI such as 10.1234/example.",
        Self::InvalidArxiv => "Use a literal arXiv identifier with an explicit vN.",
        Self::RateLimited => "The provider reached its rate or budget limit. No retry was made.",
        Self::Timeout => "The scholarly operation reached its overall deadline, including queueing.",
        Self::TooLarge => "The provider response exceeds the scholarly byte limit.",
        Self::AccessDenied => "The provider denied access. No alternate provider was tried.",
        Self::NotFound => "The selected record or version is unavailable.",
        Self::ProviderFailed => "The provider request failed. No retry or fallback was made.",
        Self::Malformed => "The provider returned invalid or contradictory scholarly metadata.",
    } }
}
#[derive(Clone)]
pub(crate) struct ScholarlyService { arxiv: ProviderClient, openalex: ProviderClient, crossref: ProviderClient, pmc: ProviderClient, timeout: Duration }
impl ScholarlyService {
    pub fn new(config: &Config) -> Result<Self> {
        let timeout = Duration::from_secs(config.request_timeout_seconds);
        let cap = config.max_bytes.min(4 * 1024 * 1024);
        Ok(Self { arxiv: ProviderClient::new(ScholarlyProvider::Arxiv, timeout, cap)?,
            openalex: ProviderClient::new(ScholarlyProvider::Openalex, timeout, cap)?,
            crossref: ProviderClient::new(ScholarlyProvider::Crossref, timeout, cap)?,
            pmc: ProviderClient::new(ScholarlyProvider::Pmc, timeout, cap)?, timeout })
    }
}
fn key(operation: &str, input: &str, limit: usize) -> String {
    hex::encode(Sha256::digest(serde_json::to_vec(&json!({"scholarly":PARSER,"arxiv":arxiv::VERSION,"operation":operation,"input":input,"limit":limit})).expect("serializable cache key")))
}
fn response(document: &Document, cached: bool) -> Result<ScholarlyResponse> {
    let snapshot: ScholarlySnapshot = serde_json::from_value(document.metadata["scholarly"].clone()).context("invalid saved scholarly snapshot")?;
    let observed = DateTime::parse_from_rfc3339(&snapshot.observed_at)?;
    let age_seconds = Utc::now().timestamp().saturating_sub(observed.timestamp()).max(0) as u64;
    Ok(ScholarlyResponse { snapshot, document_id: document.id.clone(), cached, age_seconds, warnings: document.warnings.clone(), full_text_error: None })
}
fn valid_doi(input: &str) -> std::result::Result<String, ScholarlyError> {
    let doi = input.trim().trim_start_matches("https://doi.org/").trim_start_matches("doi:");
    if doi.len() > 2048 || !doi.starts_with("10.") || !doi.contains('/') || doi.chars().any(|c| c.is_whitespace() || c.is_control()) { return Err(ScholarlyError::InvalidDoi); }
    let prefix = doi.split('/').next().unwrap().trim_start_matches("10.");
    if !(4..=9).contains(&prefix.len()) || !prefix.bytes().all(|b| b.is_ascii_digit()) || doi.ends_with('/') { return Err(ScholarlyError::InvalidDoi); }
    Ok(doi.to_owned())
}
fn wanted_arxiv(id: &str) -> std::result::Result<arxiv::Identity, ScholarlyError> {
    // Do not let URL parsing normalize paths or accept query/fragment decorations.
    if id.contains(['?', '#', '%']) || id.chars().any(|c| c.is_whitespace() || c.is_control()) { return Err(ScholarlyError::InvalidArxiv); }
    let url = url::Url::parse(&format!("https://arxiv.org/abs/{id}")).map_err(|_| ScholarlyError::InvalidArxiv)?;
    let identity = arxiv::identify(&url).map_err(|_| ScholarlyError::InvalidArxiv)?.ok_or(ScholarlyError::InvalidArxiv)?;
    if identity.version.is_none() || identity.full() != id { return Err(ScholarlyError::InvalidArxiv); }
    Ok(identity)
}
impl Engine {
    pub async fn scholarly_search(&self, request: ScholarlySearchRequest) -> Result<ScholarlyResponse> {
        tokio::time::timeout(self.scholarly.timeout, self.scholarly_search_inner(request)).await.map_err(|_| ScholarlyError::Timeout)?
    }
    async fn scholarly_search_inner(&self, request: ScholarlySearchRequest) -> Result<ScholarlyResponse> {
        if request.query.trim().is_empty() || request.query.len() > 4096 || !(1..=20).contains(&request.limit) || !matches!(request.provider, ScholarlyProvider::Arxiv | ScholarlyProvider::Openalex) { return Err(ScholarlyError::InvalidRequest.into()); }
        let _operation = self.operation_slots.acquire().await?;
        let cache_key = key(request.provider.name(), &request.query, request.limit);
        let lock = self.scholarly_lock(&cache_key).await; let _same_source = lock.lock().await;
        if !request.refresh { if let Some(d) = self.store.cached(&cache_key, CACHE_SECONDS).await? { return response(&d, true); } }
        let (mut url, client, mime) = match request.provider {
            ScholarlyProvider::Arxiv => (url::Url::parse("https://export.arxiv.org/api/query")?, &self.scholarly.arxiv, "application/atom+xml"),
            ScholarlyProvider::Openalex => (url::Url::parse("https://api.openalex.org/works")?, &self.scholarly.openalex, "application/json"),
            ScholarlyProvider::Crossref | ScholarlyProvider::Pmc => unreachable!(),
        };
        if request.provider == ScholarlyProvider::Arxiv {
            url.query_pairs_mut().append_pair("search_query", &request.query).append_pair("start", "0").append_pair("max_results", &request.limit.to_string());
        } else {
            url.query_pairs_mut().append_pair("search", &request.query).append_pair("per-page", &request.limit.to_string());
        }
        let raw = client.get(url).await?;
        let original = self.store.put_bytes(&raw.bytes, mime, "scholarly_metadata").await?;
        let bytes = raw.bytes.clone(); let provider = request.provider; let limit = request.limit;
        let permit = self.parse_slots.clone().acquire_owned().await?;
        let collection = tokio::task::spawn_blocking(move || { let _permit = permit; match provider {
            ScholarlyProvider::Arxiv => parse::arxiv(&bytes, limit),
            ScholarlyProvider::Openalex => parse::openalex(&bytes, limit), ScholarlyProvider::Crossref | ScholarlyProvider::Pmc => unreachable!(),
        } }).await??;
        let d = self.save_scholarly(raw, original, provider, Some(request.query), collection, None).await?;
        self.store.cache(cache_key, d.id.clone()).await?; response(&d, false)
    }
    pub async fn scholarly_doi(&self, request: ScholarlyDoiRequest) -> Result<ScholarlyResponse> {
        tokio::time::timeout(self.scholarly.timeout, self.scholarly_doi_inner(request)).await.map_err(|_| ScholarlyError::Timeout)?
    }
    async fn scholarly_doi_inner(&self, request: ScholarlyDoiRequest) -> Result<ScholarlyResponse> {
        let doi = valid_doi(&request.doi)?;
        let _operation = self.operation_slots.acquire().await?;
        let cache_key = key("crossref-doi", &doi, 1);
        let lock = self.scholarly_lock(&cache_key).await; let _same_source = lock.lock().await;
        if !request.refresh { if let Some(d) = self.store.cached(&cache_key, CACHE_SECONDS).await? { return response(&d, true); } }
        let mut url = url::Url::parse("https://api.crossref.org/works/")?;
        // Encode the complete DOI as one path segment. Query/fragment characters
        // in valid DOI suffixes must not change the provider request.
        url.path_segments_mut().expect("hierarchical URL").pop_if_empty().push(&doi);
        let raw = self.scholarly.crossref.get(url).await?;
        let original = self.store.put_bytes(&raw.bytes, "application/json", "scholarly_metadata").await?;
        let bytes = raw.bytes.clone(); let selected_doi = doi.clone();
        let permit = self.parse_slots.clone().acquire_owned().await?;
        let collection = tokio::task::spawn_blocking(move || { let _permit = permit; parse::crossref(&bytes, &selected_doi) }).await??;
        let d = self.save_scholarly(raw, original, ScholarlyProvider::Crossref, None, collection, None).await?;
        self.store.cache(cache_key, d.id.clone()).await?; response(&d, false)
    }
    pub async fn scholarly_arxiv(&self, request: ScholarlyArxivRequest) -> Result<ScholarlyResponse> {
        let timeout = self.scholarly.timeout.saturating_add(Duration::from_secs(self.config.helper_timeout_seconds));
        tokio::time::timeout(timeout, self.scholarly_arxiv_inner(request)).await.map_err(|_| ScholarlyError::Timeout)?
    }
    async fn scholarly_arxiv_inner(&self, request: ScholarlyArxivRequest) -> Result<ScholarlyResponse> {
        let wanted = wanted_arxiv(&request.id)?;
        let _operation = self.operation_slots.acquire().await?;
        let meta_key = key("arxiv-selected", &wanted.full(), 1);
        let lock = self.scholarly_lock(&meta_key).await; let _same_source = lock.lock().await;
        let metadata = if !request.refresh { self.store.cached(&meta_key, CACHE_SECONDS).await? } else { None };
        let (metadata, cached) = if let Some(d) = metadata { (d, true) } else {
            let raw = self.scholarly.arxiv.get(url::Url::parse(&format!("https://arxiv.org/abs/{}", wanted.full()))?).await?;
            let original = self.store.put_bytes(&raw.bytes, "text/html", "arxiv_metadata").await?;
            let bytes = raw.bytes.clone(); let resolved = raw.url.clone(); let selected = wanted.clone();
            let permit = self.parse_slots.clone().acquire_owned().await?;
            let paper = tokio::task::spawn_blocking(move || { let _permit = permit;
                arxiv::parse_html(&bytes, &resolved, &selected).map_err(|_| ScholarlyError::Malformed)
            }).await??;
            let collection = parse::Collection { records: vec![parse::selected_arxiv(&paper)], total: None, partial: false, warnings: vec![] };
            let d = self.save_scholarly(raw, original, ScholarlyProvider::Arxiv, None, collection, Some(&paper)).await?;
            self.store.cache(meta_key, d.id.clone()).await?; (d, false)
        };
        let mut result = response(&metadata, cached)?;
        if !request.full_text { return Ok(result); }
        let paper: arxiv::Paper = serde_json::from_value(metadata.metadata["arxiv"].clone())?;
        if paper.rights().decision != ReuseDecision::Permitted {
            result.warnings.push(Warning::new("scholarly_full_text_not_permitted", "No PDF was fetched. The selected version has no supported item-specific full-text reuse basis. Saved metadata and links remain available."));
            result.full_text_error = Some(Problem { code: "arxiv_reuse_not_established".into(), message: "The selected version has no supported full-text reuse basis.".into() });
            return Ok(result);
        }
        let full_key = key("arxiv-full-text", &paper.versioned_id, 1);
        if !request.refresh { if let Some(d) = self.store.cached(&full_key, CACHE_SECONDS).await? { return response(&d, true); } }
        match self.scholarly_arxiv_pdf(&paper, &metadata).await {
            Ok(d) => { self.store.cache(full_key, d.id.clone()).await?; response(&d, false) },
            Err(_) => {
                result.warnings.push(Warning::new("scholarly_full_text_unavailable", "The requested PDF could not be fetched or read. This result is saved metadata, not full text. No mirror or alternate reader was tried."));
                result.full_text_error = Some(Problem { code: "arxiv_full_text_unavailable".into(), message: "The selected full text could not be fetched or read.".into() });
                Ok(result)
            },
        }
    }
    async fn scholarly_arxiv_pdf(&self, paper: &arxiv::Paper, metadata: &Document) -> Result<Document> {
        let pdf = { let _network = self.network.acquire().await?;
            self.scholarly.arxiv.get_capped(url::Url::parse(&paper.pdf_url)?, self.config.max_bytes).await?
        };
        let returned = arxiv::identify(&url::Url::parse(&pdf.url)?)?.context("missing selected PDF identity")?;
        if returned.full() != paper.versioned_id || !pdf.bytes.starts_with(b"%PDF-") { return Err(ScholarlyError::Malformed.into()); }
        let original = self.store.put_bytes(&pdf.bytes, "application/pdf", "arxiv_pdf").await?;
        let source = Source { requested: paper.pdf_url.clone(), resolved: pdf.url, retrieved_at: pdf.observed_at, status: Some(pdf.status), version: Some(paper.versioned_id.clone()), original };
        let mut parsed = self.parse(pdf.bytes, format!("{}.pdf", paper.versioned_id.replace('/', "_")), "application/pdf".into(), None).await?;
        if parsed.blocks.is_empty() { return Err(ScholarlyError::ProviderFailed.into()); }
        // This adapter adds a source-backed scholarly snapshot. Keep its cache
        // identity distinct from older PDF documents that lack that snapshot.
        parsed.parser = format!("{}+{PARSER}", parsed.parser);
        parsed.title = paper.title.clone(); parsed.links.push(Link { url: paper.abstract_url.clone(), text: format!("arXiv {}", paper.versioned_id) });
        let mut snapshot: ScholarlySnapshot = serde_json::from_value(metadata.metadata["scholarly"].clone())?;
        snapshot.records[0].content_state = ScholarlyContentState::FullTextRead;
        parsed.metadata["scholarly"] = serde_json::to_value(snapshot)?;
        parsed.metadata["arxiv"] = metadata.metadata["arxiv"].clone();
        parsed.warnings.push(Warning::new("arxiv_license_terms", paper.rights().basis));
        self.finish(parsed, source, vec![]).await
    }
    async fn scholarly_lock(&self, key: &str) -> std::sync::Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().await; locks.retain(|_, v| v.strong_count() > 0);
        if let Some(lock) = locks.get(key).and_then(std::sync::Weak::upgrade) { lock } else {
            let lock = std::sync::Arc::new(tokio::sync::Mutex::new(())); locks.insert(key.into(), std::sync::Arc::downgrade(&lock)); lock
        }
    }
    async fn save_scholarly(&self, raw: Response, original: Artifact, provider: ScholarlyProvider, query: Option<String>, collection: parse::Collection, paper: Option<&arxiv::Paper>) -> Result<Document> {
        let snapshot = ScholarlySnapshot { provider, query, records: collection.records, total_reported: collection.total, partial: collection.partial, observed_at: raw.observed_at.clone() };
        let title = snapshot.query.as_ref().map(|q| format!("{} discovery: {q}", provider.name()))
            .or_else(|| snapshot.records.first().map(|r| r.title.clone())).unwrap_or_else(|| "Scholarly discovery: no results".into());
        let mut parsed = Parsed::new(&title, PARSER);
        for (index, record) in snapshot.records.iter().enumerate() {
            parsed.push(Content::Heading { level: 2, text: record.title.clone() }, Locator::Derived { index: index + 1 });
            parsed.push(Content::Paragraph { text: format!("Metadata identity: {}. This record is not a fetched paper body.", record.id) }, Locator::Derived { index: index + 1 });
            if let Some(abstract_text) = &record.abstract_text { parsed.push(Content::Paragraph { text: abstract_text.clone() }, Locator::Derived { index: index + 1 }); }
            for loc in &record.locations { if let Some(url) = &loc.landing_url { parsed.links.push(Link { url: url.clone(), text: record.id.clone() }); } }
            parsed.warnings.extend(record.warnings.clone());
        }
        parsed.metadata["scholarly"] = serde_json::to_value(&snapshot)?;
        if let Some(p) = paper {
            parsed.metadata["arxiv"] = serde_json::to_value(p)?;
            parsed.metadata["arxiv"]["provenance"] = json!({"source_url":raw.url,"status":raw.status,"retrieved_at":raw.observed_at,"artifact":original,"metadata_origin":p.metadata_origin});
        }
        parsed.warnings.extend(collection.warnings);
        parsed.warnings.push(Warning::new("scholarly_metadata_only", "Provider metadata and abstracts are not fetched full-text evidence. Inspect selected locations and rights before requesting a paper body."));
        let source = Source { requested: raw.url.clone(), resolved: raw.url, retrieved_at: raw.observed_at, status: Some(raw.status),
            version: paper.map(|p| p.versioned_id.clone()), original };
        self.finish(parsed, source, vec![]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_input_boundaries() {
        assert!(wanted_arxiv("2401.12345").is_err()); assert!(wanted_arxiv("2401.12345v2#fragment").is_err());
        assert_eq!(wanted_arxiv("cond-mat/0207270v1").unwrap().full(), "cond-mat/0207270v1");
        assert!(valid_doi("10.1234/item?literal#suffix").is_ok()); assert!(valid_doi("10.12/empty").is_err());
    }
    #[tokio::test]
    async fn saved_metadata_keeps_original_observation_and_citation() {
        let temp = tempfile::tempdir().unwrap(); let mut config = Config::default(); config.data_dir = temp.path().join("data");
        let engine = Engine::new(config).await.unwrap();
        let paper: arxiv::Paper = serde_json::from_value(json!({"resolver":arxiv::VERSION,"id":"2401.12345","version":"2","versioned_id":"2401.12345v2","requested_id":"2401.12345v2",
            "title":"Literal title","authors":["B. First","A. Second"],"abstract_text":"Abstract only","categories":[],"source_dates":{"selected_submission":"2024-01-02T03:04:05+00:00"},"updated":"2024-01-02T03:04:05+00:00",
            "abstract_url":"https://arxiv.org/abs/2401.12345v2","pdf_url":"https://arxiv.org/pdf/2401.12345v2","license_evidence":["https://arxiv.org/licenses/nonexclusive-distrib/1.0/"]})).unwrap();
        let bytes = b"<html>retained synthetic metadata</html>".to_vec(); let original = engine.store.put_bytes(&bytes, "text/html", "arxiv_metadata").await.unwrap();
        let collection = || parse::Collection { records: vec![parse::selected_arxiv(&paper)], total: None, partial: false, warnings: vec![] };
        let raw = |time: &str| Response { bytes: bytes.clone(), url: paper.abstract_url.clone(), status: 200, observed_at: time.into() };
        let first = engine.save_scholarly(raw("2024-01-02T03:04:05+00:00"), original.clone(), ScholarlyProvider::Arxiv, None, collection(), Some(&paper)).await.unwrap();
        let again = engine.save_scholarly(raw("2025-01-02T03:04:05+00:00"), original, ScholarlyProvider::Arxiv, None, collection(), Some(&paper)).await.unwrap();
        assert_eq!(first.id, again.id); assert_eq!(again.source.retrieved_at, first.source.retrieved_at);
        assert_eq!(response(&again, true).unwrap().snapshot.observed_at, first.source.retrieved_at);
        let citation = engine.citation(&again.id, "csl").await.unwrap(); let csl: serde_json::Value = serde_json::from_str(citation["text"].as_str().unwrap()).unwrap();
        assert_eq!(csl["version"], "2"); assert_eq!(csl["author"][0]["literal"], "B. First");
        assert_eq!(parse::selected_arxiv(&paper).locations[0].rights.decision, ReuseDecision::NotPermitted);
    }
}
