//! Explicit public code navigation. Discovery is not fetched file evidence.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::{Artifact, Document, Problem, Warning};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryDiscoverRequest {
    pub query: String,
    #[serde(default = "five")] pub limit: usize,
}
fn five() -> usize { 5 }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryHit {
    pub repository: String,
    pub url: String,
    /// Provider description, not a fetched file or code match.
    pub description: Option<String>,
    pub default_branch: String,
    /// Provider license metadata, not revision-specific permission to reuse files.
    pub license: Value,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeObservation {
    pub url: String,
    pub status: u16,
    pub artifact: Artifact,
    pub rate_remaining: Option<u64>,
    pub rate_reset_unix: Option<u64>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryDiscoverResponse {
    pub document_id: String,
    pub query: String,
    pub observed_at: String,
    pub total_count: Option<u64>,
    pub incomplete: bool,
    pub results: Vec<RepositoryHit>,
    pub observation: CodeObservation,
    pub warnings: Vec<Warning>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MapLimits {
    /// Root entries are depth one. Subtrees at the limit remain unvisited.
    pub depth: usize,
    pub max_entries: usize,
    pub max_requests: usize,
    /// Cumulative decoded API response bytes, including commit/tree metadata.
    pub max_bytes: usize,
}
impl Default for MapLimits {
    fn default() -> Self { Self { depth: 2, max_entries: 200, max_requests: 8, max_bytes: 2 * 1024 * 1024 } }
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryMapRequest {
    pub repository: String,
    /// Explicit branch, tag, or commit. No implicit default/latest selection.
    pub reference: String,
    #[serde(default)] pub path: String,
    #[serde(default)] pub limits: MapLimits,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryEntry {
    pub path: String,
    /// Git object type. Symlinks and submodules are listed but never followed.
    pub kind: String,
    pub mode: String,
    pub object_sha: String,
    pub size: Option<u64>,
    pub url: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeCoverage {
    pub requests: usize,
    pub response_bytes: usize,
    pub files_read: usize,
    pub file_bytes: usize,
    pub incomplete: bool,
    pub stopped: Option<Problem>,
    pub warnings: Vec<Warning>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMap {
    pub repository: String,
    pub requested_ref: String,
    pub resolved_commit: String,
    pub root_tree: String,
    pub path: String,
    pub limits: MapLimits,
    pub entries: Vec<RepositoryEntry>,
    pub observations: Vec<CodeObservation>,
    pub coverage: CodeCoverage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMapResponse { pub document_id: String, pub map: RepositoryMap }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CodeSearchMode {
    /// Search only admitted path names. Does not fetch files.
    Paths,
    /// Fetch and search only the explicitly listed admitted files.
    Literal,
    /// Unavailable: authentication is not configured or inherited.
    GithubCode,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FileLimits {
    pub max_files: usize,
    pub max_file_bytes: usize,
    /// Total retained file bytes. API envelope bytes are also bounded separately.
    pub max_bytes: usize,
    pub max_requests: usize,
}
impl Default for FileLimits {
    fn default() -> Self { Self { max_files: 2, max_file_bytes: 256 * 1024, max_bytes: 1024 * 1024, max_requests: 2 } }
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeSearchRequest {
    pub map_id: String,
    /// Required. No hidden fallback from authenticated search to file scanning.
    pub mode: CodeSearchMode,
    pub query: String,
    /// Exact admitted paths, required for literal search. No implicit glob expansion.
    #[serde(default)] pub paths: Vec<String>,
    #[serde(default = "five")] pub limit: usize,
    #[serde(default)] pub limits: FileLimits,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeMatch {
    pub path: String,
    pub blob_sha: String,
    pub url: String,
    pub document_id: String,
    /// One-based line number, with LF as the line separator.
    pub line: usize,
    /// Half-open byte offsets into the exact retained UTF-8 file, not the excerpt.
    pub byte_range: [usize; 2],
    pub excerpt: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFileOutcome {
    pub path: String,
    pub document_id: Option<String>,
    pub error: Option<Problem>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchResponse {
    pub map_id: String,
    pub repository: String,
    pub requested_ref: String,
    pub resolved_commit: String,
    pub mode: CodeSearchMode,
    pub query: String,
    pub paths: Vec<RepositoryEntry>,
    pub matches: Vec<CodeMatch>,
    pub files: Vec<CodeFileOutcome>,
    pub coverage: CodeCoverage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeFileRequest {
    pub map_id: String,
    pub path: String,
    #[serde(default)] pub limits: FileLimits,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFileResponse { pub document: Document, pub coverage: CodeCoverage }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentationKind { Page, Source }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentationRequest {
    pub crate_name: String,
    /// Exact release only, such as 0.4.3. Ranges, latest, and newest are rejected.
    pub version: String,
    /// Page path after crate/version, or file path after crate/version/source.
    pub path: String,
    pub kind: DocumentationKind,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationResponse { pub document: Document, pub coverage: CodeCoverage }
