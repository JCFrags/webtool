//! Explicit PMC OAI selection. No Europe PMC, PDF, media, or provider fallback.
mod metadata;
mod jats;
mod citation;
#[cfg(test)] mod tests;
use anyhow::{Context, Result};
use serde_json::json;
use webtool_protocol::*;
use crate::{readers::Parsed, Engine};
use super::{response, transport::Response, ScholarlyError, CACHE_SECONDS};
pub use citation::citation;
const OAI_URL: &str = "https://pmc.ncbi.nlm.nih.gov/api/oai/v1/mh/";
const METADATA_PARSER: &str = "pmc-oai-frontmatter/1";
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum PmcError {
    #[error("Use pmc:PMCdigits, optionally .N, and a valid exact OAI datestamp assertion.")]
    InvalidRequest,
    #[error("PMC returned a different source identifier or contradictory version/instance.")]
    IdentityMismatch,
    #[error("The asserted version is not the version returned by PMC OAI. Historical version retrieval is unsupported.")]
    VersionUnavailable,
    #[error("The selected source datestamp or front matter changed. Refresh and inspect metadata before another full-text request.")]
    SourceChanged,
    #[error("PMC returned invalid or contradictory selected metadata.")]
    InvalidMetadata,
    #[error("The source is not supported UTF-8 OAI/JATS XML. DTDs and external entities are not processed.")]
    InvalidXml,
    #[error("The JATS node, nesting, table, or block limit was reached.")]
    SizeLimit,
    #[error("The selected full text is unavailable, suppressed, or outside the reusable OAI set.")]
    Unavailable,
    #[error("The selected article has no supported item-specific full-text reuse basis.")]
    ReuseNotEstablished,
    #[error("The selected source has no complete supplied version/instance for full-text acceptance.")]
    IdentityIncomplete,
    #[error("The full JATS source has no readable body. Metadata is not full text.")]
    BodyUnavailable,
}
impl PmcError {
    pub fn message(self) -> &'static str { match self {
        Self::InvalidRequest => "Use pmc:PMCdigits, optionally .N, and a valid exact OAI datestamp assertion.",
        Self::IdentityMismatch => "PMC returned a different source identifier or contradictory version/instance.",
        Self::VersionUnavailable => "The asserted version is not the version returned by PMC OAI. Historical version retrieval is unsupported.",
        Self::SourceChanged => "The selected source datestamp or front matter changed. Refresh and inspect metadata before another full-text request.",
        Self::InvalidMetadata => "PMC returned invalid or contradictory selected metadata.",
        Self::InvalidXml => "The source is not supported UTF-8 OAI/JATS XML. DTDs and external entities are not processed.",
        Self::SizeLimit => "The JATS node, nesting, table, or block limit was reached.",
        Self::Unavailable => "The selected full text is unavailable, suppressed, or outside the reusable OAI set.",
        Self::ReuseNotEstablished => "The selected article has no supported item-specific full-text reuse basis.",
        Self::IdentityIncomplete => "The selected source has no complete supplied version/instance for full-text acceptance.",
        Self::BodyUnavailable => "The full JATS source has no readable body. Metadata is not full text.",
    } }
    pub fn code(&self) -> &'static str { match self {
        Self::InvalidRequest => "pmc_invalid_request", Self::IdentityMismatch => "pmc_identity_mismatch",
        Self::VersionUnavailable => "pmc_version_unavailable", Self::SourceChanged => "pmc_source_changed",
        Self::InvalidMetadata => "pmc_invalid_metadata", Self::InvalidXml => "pmc_invalid_xml",
        Self::SizeLimit => "pmc_size_limit", Self::Unavailable => "pmc_unavailable",
        Self::ReuseNotEstablished => "pmc_reuse_not_established", Self::IdentityIncomplete => "pmc_identity_incomplete",
        Self::BodyUnavailable => "pmc_body_unavailable",
    } }
    pub fn http_status(&self) -> u16 { match self {
        Self::InvalidRequest => 400, Self::VersionUnavailable | Self::Unavailable => 404, Self::SourceChanged => 409,
        Self::SizeLimit => 413, Self::IdentityIncomplete | Self::ReuseNotEstablished | Self::BodyUnavailable => 422,
        _ => 502,
    } }
    fn problem(&self) -> Problem { Problem { code: self.code().into(), message: self.to_string() } }
}
#[derive(Debug, Clone)]
struct Wanted { pmcid: String, version: Option<String>, expected_datestamp: Option<String> }
fn positive_digits(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && !s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit())
}
impl Wanted {
    fn parse(id: &str) -> std::result::Result<Self, PmcError> {
        let value = id.strip_prefix("pmc:PMC").ok_or(PmcError::InvalidRequest)?;
        let (number, version) = value.split_once('.').map_or((value, None), |(n, v)| (n, Some(v)));
        if !positive_digits(number, 12) || version.is_some_and(|v| !positive_digits(v, 9)) { return Err(PmcError::InvalidRequest); }
        Ok(Self { pmcid: format!("PMC{number}"), version: version.map(str::to_owned), expected_datestamp: None })
    }
    fn number(&self) -> &str { &self.pmcid[3..] }
    fn oai(&self) -> String { format!("oai:pubmedcentral.nih.gov:{}", self.number()) }
    fn id(&self) -> String { format!("pmc:{}{}", self.pmcid, self.version.as_ref().map(|v| format!(".{v}")).unwrap_or_default()) }
    fn url(&self, prefix: &str) -> url::Url {
        let mut url = url::Url::parse(OAI_URL).expect("constant URL");
        url.query_pairs_mut().append_pair("verb", "GetRecord").append_pair("identifier", &self.oai()).append_pair("metadataPrefix", prefix); url
    }
    fn check(&self, meta: &metadata::Metadata) -> std::result::Result<(), PmcError> {
        if meta.pmcid != self.pmcid { return Err(PmcError::IdentityMismatch); }
        if self.version.as_ref().is_some_and(|v| meta.versioned_pmcid.as_ref() != Some(&format!("{}.{v}", self.pmcid)) || meta.pmc_version.as_ref() != Some(v)) { return Err(PmcError::VersionUnavailable); }
        if self.expected_datestamp.as_ref().is_some_and(|v| v != &meta.oai_datestamp) { return Err(PmcError::SourceChanged); }
        Ok(())
    }
}
fn cache_key(operation: &str, input: &str) -> String { super::key(operation, &format!("{METADATA_PARSER}:{}:{input}", jats::PARSER), 1) }
fn full_permission(meta: &metadata::Metadata, record: &ScholarlyRecord) -> std::result::Result<(), PmcError> {
    if matches!(record.content_state, ScholarlyContentState::Unavailable) || !meta.sets.iter().any(|s| s == "pmc-open") { return Err(PmcError::Unavailable); }
    if !record.locations.iter().any(|l| l.primary && l.rights.decision == ReuseDecision::Permitted) { return Err(PmcError::ReuseNotEstablished); }
    if meta.versioned_pmcid.is_none() || meta.pmc_version.is_none() || meta.article_instance.is_none() { return Err(PmcError::IdentityIncomplete); }
    Ok(())
}
impl Engine {
    pub async fn scholarly_pmc(&self, request: ScholarlyPmcRequest) -> Result<ScholarlyResponse> {
        // Includes queueing, pacing, metadata, optional full text, parsing, and persistence.
        tokio::time::timeout(self.scholarly.timeout, self.scholarly_pmc_inner(request)).await.map_err(|_| ScholarlyError::Timeout)?
    }
    async fn scholarly_pmc_inner(&self, request: ScholarlyPmcRequest) -> Result<ScholarlyResponse> {
        let mut wanted = Wanted::parse(&request.id)?;
        if request.expected_datestamp.as_ref().is_some_and(|v| metadata::datestamp(v).is_none()) { return Err(PmcError::InvalidRequest.into()); }
        wanted.expected_datestamp = request.expected_datestamp;
        let _operation = self.operation_slots.acquire().await?;
        let meta_key = cache_key("pmc-selected", &wanted.id());
        let lock = self.scholarly_lock(&meta_key).await; let _same_source = lock.lock().await;
        let cached = if request.refresh { None } else { self.store.cached(&meta_key, CACHE_SECONDS).await? };
        let (metadata, was_cached) = if let Some(document) = cached { (document, true) } else {
            let raw = self.scholarly.pmc.get(wanted.url("pmc_fm")).await?;
            let original = self.store.put_bytes(&raw.bytes, "application/xml", "pmc_oai_metadata").await?;
            let bytes = raw.bytes.clone(); let selected = wanted.clone();
            let permit = self.parse_slots.clone().acquire_owned().await?;
            let parsed = tokio::task::spawn_blocking(move || { let _permit = permit; metadata::parse(&bytes, &selected, "pmc_fm") }).await??;
            let document = self.save_pmc_metadata(raw, original, parsed).await?;
            self.store.cache(meta_key, document.id.clone()).await?; (document, false)
        };
        let meta: metadata::Metadata = serde_json::from_value(metadata.metadata["pmc"]["selection"].clone()).context("invalid saved PMC metadata")?;
        wanted.check(&meta)?;
        let mut result = response(&metadata, was_cached)?;
        if !request.full_text { return Ok(result); }
        if let Err(error) = full_permission(&meta, &result.snapshot.records[0]) {
            result.warnings.push(Warning::new("pmc_full_text_not_fetched", "This is saved metadata/abstract only. Full text was not fetched because availability, selected identity, or item-specific reuse was not established."));
            result.full_text_error = Some(error.problem()); return Ok(result);
        }
        // Bind the body cache to the selected front bytes and currency, not just PMCID.
        let full_key = cache_key("pmc-full-text", &format!("{}:{}:{}", result.snapshot.records[0].id, meta.oai_datestamp, meta.front_sha256));
        if !request.refresh { if let Some(document) = self.store.cached(&full_key, CACHE_SECONDS).await? { return response(&document, true); } }
        match self.pmc_full_document(&wanted, &metadata, &meta).await {
            Ok(document) => { self.store.cache(full_key, document.id.clone()).await?; response(&document, false) },
            Err(error) => {
                let problem = if let Some(e) = error.downcast_ref::<PmcError>() { Some(e.problem()) }
                    else { error.downcast_ref::<ScholarlyError>().map(|e| Problem { code: e.code().into(), message: e.message().into() }) };
                let Some(problem) = problem else { return Err(error); };
                result.warnings.push(Warning::new("pmc_full_text_unavailable", "Requested full text was not accepted. Saved metadata remains available. Inspect full_text_error. No retry, alternate version, associated file, or fallback provider was tried."));
                result.full_text_error = Some(problem); Ok(result)
            },
        }
    }
    async fn save_pmc_metadata(&self, raw: Response, original: Artifact, selected: metadata::Selected) -> Result<Document> {
        let mut parsed = Parsed::new(&selected.record.title, METADATA_PARSER);
        parsed.push(Content::Paragraph { text: format!("Selected PMC metadata: {}. This is not a fetched paper body.", selected.record.id) }, Locator::Derived { index: 1 });
        if let Some(text) = &selected.record.abstract_text { parsed.push(Content::Paragraph { text: text.clone() }, Locator::Derived { index: 2 }); }
        parsed.links.extend(selected.record.locations.iter().filter_map(|l| l.landing_url.as_ref()).map(|u| Link { url: u.clone(), text: selected.record.id.clone() }));
        parsed.warnings.extend(selected.record.warnings.clone());
        parsed.warnings.push(Warning::new("scholarly_metadata_only", "PMC front matter and abstracts are not full-text evidence. Version suffixes assert the delivered OAI version; arbitrary historical retrieval is unsupported."));
        parsed.metadata["pmc"] = json!({"selection":selected.meta,"metadata_provenance":{"source_url":raw.url,"status":raw.status,"observed_at":raw.observed_at,"artifact":original}});
        parsed.metadata["scholarly"] = serde_json::to_value(ScholarlySnapshot { provider: ScholarlyProvider::Pmc, query: None, records: vec![selected.record], total_reported: None, partial: false, observed_at: raw.observed_at.clone() })?;
        let source = Source { requested: raw.url.clone(), resolved: raw.url, retrieved_at: raw.observed_at, status: Some(raw.status), version: selected.meta.versioned_pmcid, original };
        self.finish(parsed, source, vec![]).await
    }
    async fn pmc_full_document(&self, wanted: &Wanted, metadata: &Document, selected: &metadata::Metadata) -> Result<Document> {
        let raw = self.scholarly.pmc.get_capped(wanted.url("pmc"), self.config.max_bytes.min(16 * 1024 * 1024)).await?;
        let original = self.store.put_bytes(&raw.bytes, "application/xml", "pmc_oai_jats").await?;
        let bytes = raw.bytes.clone(); let wanted = wanted.clone(); let before = selected.clone();
        let permit = self.parse_slots.clone().acquire_owned().await?;
        let (mut parsed, full) = tokio::task::spawn_blocking(move || {
            let _permit = permit; let full = metadata::parse(&bytes, &wanted, "pmc")?;
            if !before.same_selection(&full.meta) { return Err(PmcError::SourceChanged); }
            full_permission(&full.meta, &full.record)?;
            let parsed = jats::parse(&bytes, &full)?; Ok::<_, PmcError>((parsed, full))
        }).await??;
        let mut snapshot = ScholarlySnapshot { provider: ScholarlyProvider::Pmc, query: None, records: vec![full.record], total_reported: None,
            partial: parsed.metadata["jats"]["partial"].as_bool().unwrap_or(true), observed_at: raw.observed_at.clone() };
        snapshot.records[0].content_state = ScholarlyContentState::FullTextRead;
        parsed.metadata["scholarly"] = serde_json::to_value(&snapshot)?;
        parsed.metadata["pmc"] = metadata.metadata["pmc"].clone();
        parsed.metadata["pmc"]["full_text_selection"] = serde_json::to_value(&full.meta)?;
        parsed.metadata["pmc"]["full_text_provenance"] = json!({"source_url":raw.url,"status":raw.status,"observed_at":raw.observed_at,"artifact":original,"metadata_document_id":metadata.id});
        parsed.metadata["jats"]["artifact"] = serde_json::to_value(&original)?;
        parsed.warnings.push(Warning::new("pmc_license_terms", snapshot.records[0].locations[0].rights.basis.clone()));
        let source = Source { requested: raw.url.clone(), resolved: raw.url, retrieved_at: raw.observed_at, status: Some(raw.status), version: full.meta.versioned_pmcid, original };
        self.finish(parsed, source, vec![]).await
    }
}
