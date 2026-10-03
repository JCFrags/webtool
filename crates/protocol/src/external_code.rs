//! Explicit third-party index operations. Index snippets are not fetched source evidence.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::{Artifact, Document, Problem, Warning};

fn five() -> usize { 5 }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExternalCodeProvider { Sourcegraph, Context7 }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProviderStatus {
    pub provider: ExternalCodeProvider,
    pub endpoint_configured: bool,
    pub credential_configured: bool,
    /// Local configuration only. No probe, account, permission, or coverage check.
    pub locally_ready: bool,
    pub cooldown_seconds: u64,
    pub requests_in_window: usize,
    pub max_requests_per_minute: usize,
    pub detail: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalProvidersResponse { pub providers: Vec<ExternalProviderStatus> }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExternalIndexCoverage {
    pub requests: usize,
    pub response_bytes: usize,
    pub incomplete: bool,
    pub stopped: Option<Problem>,
    pub omitted_results: usize,
    pub warnings: Vec<Warning>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalIndexObservation {
    pub provider: ExternalCodeProvider,
    pub url: String,
    pub observed_at: String,
    pub status: u16,
    pub artifact: Artifact,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourcegraphMode { Literal, Regexp, Path, Symbol }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcegraphSearchRequest {
    pub query: String,
    pub mode: SourcegraphMode,
    /// Exact indexed repository name, such as github.com/owner/repository.
    #[serde(default)] pub repository: Option<String>,
    /// One explicit ref. Requires an exact repository. No revision lists or time queries.
    #[serde(default)] pub reference: Option<String>,
    /// Literal path substring, not an arbitrary provider expression.
    #[serde(default)] pub path: Option<String>,
    #[serde(default)] pub language: Option<String>,
    #[serde(default = "five")] pub limit: usize,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcegraphLine {
    pub text: String,
    pub line_number_zero_based: usize,
    /// Provider coordinates, not original-file byte offsets.
    pub offset_and_lengths: Value,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcegraphHit {
    pub kind: String,
    pub repository: String,
    pub path: String,
    pub commit: Option<String>,
    pub lines: Vec<SourcegraphLine>,
    pub indexed_url: String,
    /// Exact admitted provider object, including absent versus null fields.
    pub native: Value,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcegraphSearchResponse {
    pub document_id: String,
    pub request: SourcegraphSearchRequest,
    pub provider_query: String,
    pub hits: Vec<SourcegraphHit>,
    pub progress: Vec<Value>,
    pub alerts: Vec<Value>,
    pub filters: Vec<Value>,
    pub stream_done: bool,
    pub observation: ExternalIndexObservation,
    pub coverage: ExternalIndexCoverage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcegraphVerifyRequest {
    pub search_id: String,
    /// Zero-based admitted hit index.
    pub hit: usize,
    pub map_id: String,
    pub file_id: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedIndexLine {
    pub line: usize,
    /// Half-open offsets into the retained original file, not the index snippet.
    pub byte_range: [usize; 2],
    pub text: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcegraphVerifyResponse {
    pub search_id: String,
    pub hit: usize,
    pub map_id: String,
    pub file_id: String,
    pub repository: String,
    pub commit: String,
    pub path: String,
    pub blob_sha: String,
    pub original: Artifact,
    pub lines: Vec<VerifiedIndexLine>,
    pub warnings: Vec<Warning>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context7LibrariesRequest {
    pub library_name: String,
    pub query: String,
    #[serde(default = "five")] pub limit: usize,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context7Library {
    pub id: String,
    pub title: String,
    /// Exact provider metadata. A version label or branch is not a source commit.
    pub native: Value,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context7LibrariesResponse {
    pub document_id: String,
    pub request: Context7LibrariesRequest,
    pub libraries: Vec<Context7Library>,
    pub search_filter_applied: Value,
    pub observation: ExternalIndexObservation,
    pub coverage: ExternalIndexCoverage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Context7VersionSelection {
    Listed { version: String },
    /// Explicit tracked index selection, not an installed/latest release assertion.
    Tracked,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context7ContextRequest {
    pub discovery_id: String,
    pub library_id: String,
    pub selection: Context7VersionSelection,
    pub query: String,
    #[serde(default = "five")] pub limit: usize,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context7ContextResponse {
    pub document: Document,
    pub discovery_id: String,
    pub library: Context7Library,
    pub selection: Context7VersionSelection,
    pub requested_library_id: String,
    pub code_snippets: Vec<Value>,
    pub info_snippets: Vec<Value>,
    pub observation: ExternalIndexObservation,
    pub coverage: ExternalIndexCoverage,
}
