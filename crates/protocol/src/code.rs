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
    /// Glob search of admitted paths only. No provider request.
    PathGlob,
    /// Bounded regular expression over explicitly selected UTF-8 files.
    Regex,
    /// Lexical declaration-name search in explicitly selected supported files.
    Symbols,
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
    /// Exact admitted paths, required for literal, regex, or symbol search. No implicit expansion.
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

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GitHubKind { Issue, PullRequest, Release }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GitHubState { Open, Closed, #[default] All }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GitHubPage { pub page: usize, pub limit: usize }
impl Default for GitHubPage { fn default() -> Self { Self { page: 1, limit: 5 } } }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubListRequest {
    pub repository: String,
    pub kind: GitHubKind,
    #[serde(default)] pub state: GitHubState,
    #[serde(default)] pub pagination: GitHubPage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubListResponse {
    pub document_id: String,
    pub repository: String,
    pub kind: GitHubKind,
    pub observed_at: String,
    pub pagination: GitHubPage,
    /// Caller-selected next page. Never fetched automatically.
    pub next_page: Option<usize>,
    /// Provider metadata for discovery. These are not accepted object bodies.
    pub items: Vec<Value>,
    pub observation: CodeObservation,
    pub coverage: CodeCoverage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubReadRequest {
    pub repository: String,
    pub kind: GitHubKind,
    /// Required for an issue or pull request. Releases require a tag instead.
    pub number: Option<u64>,
    /// Exact release tag. This is not an immutable repository commit.
    pub tag: Option<String>,
    /// Optional one-page issue conversation comments, not PR reviews or timelines.
    pub comments: Option<GitHubPage>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubReadResponse {
    pub document: Document,
    /// Separate saved JSON snapshot. Its locators address its own original.
    pub comments: Option<Document>,
    pub next_comment_page: Option<usize>,
    pub coverage: CodeCoverage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeCompareRequest {
    pub base_map_id: String,
    pub head_map_id: String,
    /// Opt in to one pinned GitHub comparison page. Otherwise compare admitted entries offline.
    #[serde(default)] pub provider: bool,
    #[serde(default)] pub pagination: GitHubPage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeMapChange {
    pub path: String,
    pub base: Option<RepositoryEntry>,
    pub head: Option<RepositoryEntry>,
    /// Missing entries mean not admitted when the corresponding map is incomplete.
    pub status: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeCompareResponse {
    pub document_id: String,
    pub base_map_id: String,
    pub head_map_id: String,
    pub repository: String,
    pub base_requested_ref: String,
    pub head_requested_ref: String,
    pub base_commit: String,
    pub head_commit: String,
    pub changes: Vec<CodeMapChange>,
    /// Separate retained provider comparison snapshot with JSON-pointer patch locators.
    pub provider_document: Option<Document>,
    pub next_page: Option<usize>,
    pub coverage: CodeCoverage,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeContextRequest {
    pub map_id: String,
    pub file_id: String,
    /// One-based line in the retained UTF-8 file. No provider request.
    pub line: usize,
    #[serde(default = "three")] pub before: usize,
    #[serde(default = "eight")] pub after: usize,
}
fn three() -> usize { 3 }
fn eight() -> usize { 8 }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeContextResponse {
    pub map_id: String,
    pub file_id: String,
    pub repository: String,
    pub requested_ref: String,
    pub resolved_commit: String,
    pub path: String,
    pub blob_sha: String,
    pub url: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Half-open original-file UTF-8 byte range. Text is that exact substring.
    pub byte_range: [usize; 2],
    pub text: String,
    pub coverage: CodeCoverage,
}
