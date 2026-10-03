//! Provider metadata is discovery evidence, not a fetched paper body.
use std::collections::BTreeMap;
use serde::{Deserialize, Serialize};
use crate::{Problem, Warning};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScholarlyProvider { Arxiv, Openalex, Crossref }
impl ScholarlyProvider {
    pub fn name(self) -> &'static str { match self { Self::Arxiv => "arxiv", Self::Openalex => "openalex", Self::Crossref => "crossref" } }
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlySearchRequest {
    pub provider: ScholarlyProvider,
    pub query: String,
    #[serde(default = "five")] pub limit: usize,
    #[serde(default)] pub refresh: bool,
}
fn five() -> usize { 5 }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyDoiRequest { pub doi: String, #[serde(default)] pub refresh: bool }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyArxivRequest {
    /// A literal arXiv identifier with an explicit vN, not a DOI or title.
    pub id: String,
    #[serde(default)] pub full_text: bool,
    #[serde(default)] pub refresh: bool,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyAuthor {
    /// Provider-supplied literal name. Never inferred from given/family parts.
    pub literal: Option<String>,
    pub given: Option<String>,
    pub family: Option<String>,
    pub orcid: Option<String>,
    pub id: Option<String>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyDate {
    /// Source value, with no invented month or day.
    pub value: String,
    /// year, month, day, timestamp, or literal when the format is not established.
    pub precision: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReuseDecision { Permitted, NotPermitted, Unknown }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyRights {
    pub decision: ReuseDecision,
    pub license: Option<String>,
    pub evidence_url: Option<String>,
    /// Scope of this evidence. An OA flag alone is not a reuse license.
    pub basis: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyLocation {
    pub id: Option<String>,
    pub landing_url: Option<String>,
    pub pdf_url: Option<String>,
    pub version: Option<String>,
    pub primary: bool,
    pub rights: ScholarlyRights,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScholarlyContentState { NotRead, AbstractOnly, Unavailable, FullTextRead }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyStatusClaim {
    /// The exact provider field, such as update-to or is_retracted.
    pub field: String,
    /// Preserve direction. An update-to target is not necessarily this work.
    pub related_id: Option<String>,
    pub label: String,
    pub source: String,
    pub date: Option<ScholarlyDate>,
    pub record_id: Option<String>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyStatus {
    pub coverage: String,
    pub claims: Vec<ScholarlyStatusClaim>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyRecord {
    pub id: String,
    pub identifiers: BTreeMap<String, String>,
    pub title: String,
    /// Provider order is retained. Name parts are not guessed.
    pub authors: Vec<ScholarlyAuthor>,
    pub dates: BTreeMap<String, ScholarlyDate>,
    pub abstract_text: Option<String>,
    pub content_state: ScholarlyContentState,
    pub locations: Vec<ScholarlyLocation>,
    pub metadata_license: Option<String>,
    pub status: ScholarlyStatus,
    pub warnings: Vec<Warning>,
}
/// Stored in Document.metadata.scholarly. The original is the provider response.
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlySnapshot {
    pub provider: ScholarlyProvider,
    pub query: Option<String>,
    pub records: Vec<ScholarlyRecord>,
    pub total_reported: Option<u64>,
    pub partial: bool,
    /// Observation of retained metadata, not a later cache validation time.
    pub observed_at: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScholarlyResponse {
    pub snapshot: ScholarlySnapshot,
    pub document_id: String,
    pub cached: bool,
    pub age_seconds: u64,
    pub warnings: Vec<Warning>,
    /// A requested full-text failure can accompany usable saved metadata.
    pub full_text_error: Option<Problem>,
}
