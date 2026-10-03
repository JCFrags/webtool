//! HTTP-only messages. The engine and CLI continue to share protocol types.
use serde::Deserialize;
use utoipa::{IntoParams, IntoResponses, ToSchema};
use webtool_protocol::{JobState, Link, Problem};

// IntoResponses references Problem but does not collect its schema automatically.
#[derive(utoipa::OpenApi)]
#[openapi(components(schemas(Problem)))]
pub(crate) struct ApiDoc;

#[allow(dead_code)]
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
pub(crate) struct OriginalBytes(Vec<u8>);

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ListQuery {
    /// Maximum results. Values above 1000 are clamped to 1000.
    #[serde(default = "default_limit")]
    #[param(default = 100)]
    pub limit: usize,
}
fn default_limit() -> usize { 100 }

#[derive(Deserialize, ToSchema)]
pub(crate) struct MapRequest {
    pub url: String,
    #[serde(default = "default_limit")]
    #[schema(default = 100, minimum = 1, maximum = 5000)]
    pub limit: usize,
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct MediaRequest {
    pub url: String,
    #[serde(default = "english")]
    #[schema(default = "en")]
    pub language: String,
    pub library: Option<String>,
}
fn english() -> String { "en".into() }

#[derive(Deserialize)]
pub(crate) struct CiteRequest {
    #[serde(alias = "document_id")]
    pub doi: String,
    pub format: String,
}

// These schemas describe the multipart boundary and the engine's current JSON
// values, not a second operation implementation. Their shapes are exercised by
// focused HTTP checks. Do not narrow arbitrary Document.metadata or extract data.
#[allow(dead_code)]
#[derive(ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct IngestForm {
    /// Exactly one file part. Its bytes are retained without modification.
    #[schema(value_type = String, format = Binary)]
    file: Vec<u8>,
    /// Filename, not a URL or server path. Defaults to the file part's filename.
    #[schema(nullable = false)]
    name: Option<String>,
    /// Existing shared library name.
    #[schema(nullable = false)]
    library: Option<String>,
    #[schema(nullable = false)]
    actor: Option<String>,
    /// Optional explicit HTML CSS selector.
    #[schema(nullable = false)]
    selector: Option<String>,
}

#[allow(dead_code)]
#[derive(ToSchema)]
#[serde(untagged)]
pub(crate) enum CiteInput {
    Doi {
        /// DOI or saved paper document ID. Do not send both doi and document_id.
        doi: String,
        /// bibtex, ris, or csl. Saved arXiv papers support bibtex and csl only.
        format: String,
    },
    SavedDocument {
        /// Compatibility alias for doi. Must identify a saved paper document.
        document_id: String,
        format: String,
    },
}

#[allow(dead_code)]
#[derive(ToSchema)]
pub(crate) struct LibraryAddResponse {
    added: bool,
    library: String,
    document_id: String,
}

#[allow(dead_code)]
#[derive(ToSchema)]
pub(crate) struct CancelResponse {
    id: String,
    /// True means cancellation was requested, not that the job has stopped.
    cancel_requested: bool,
    /// State when the cancellation request was processed. Poll the job afterward.
    state: JobState,
}

#[allow(dead_code)]
#[derive(ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum MapResponse {
    PageLinks { source: String, links: Vec<Link>, truncated: bool },
    Urlset { source: String, urls: Vec<String>, truncated: bool, nested_sitemaps_expanded: bool },
    Sitemapindex { source: String, urls: Vec<String>, truncated: bool, nested_sitemaps_expanded: bool },
}

#[allow(dead_code)]
#[derive(ToSchema)]
#[serde(untagged)]
pub(crate) enum CiteResponse {
    Doi { doi: String, format: String, source: String, text: String },
    SavedPaper {
        document_id: String, format: String, source: String,
        version: String, metadata_source: String, text: String,
    },
}

/// Shared failures. Specific engine conditions depend on the selected operation.
#[allow(dead_code)]
#[derive(IntoResponses)]
pub(crate) enum HttpErrors {
    #[response(status = 400, description = "Malformed JSON, query, path, multipart body, or a known invalid request.")]
    BadRequest(Problem),
    #[response(status = 404, description = "The saved resource or requested upstream reference was not found.")]
    NotFound(Problem),
    #[response(status = 405, description = "Method not allowed for this route.")]
    MethodNotAllowed(Problem),
    #[response(status = 409, description = "The crawl job has no resumable work in its current state or budget.")]
    Conflict(Problem),
    #[response(status = 413, description = "Request, upload, or source byte limit exceeded.")]
    TooLarge(Problem),
    #[response(status = 415, description = "The JSON endpoint requires Content-Type: application/json.")]
    MediaType(Problem),
    #[response(status = 422, description = "JSON does not match the request schema, or a known capability/content condition prevents the operation.")]
    Unprocessable(Problem),
    #[response(status = 429, description = "A known upstream rate limit was reached.")]
    RateLimit(Problem),
    #[response(status = 500, description = "Internal failure or an engine failure not yet represented by a known error code. Details are not exposed.")]
    Internal(Problem),
    #[response(status = 502, description = "A known source or helper request failed.")]
    Upstream(Problem),
    #[response(status = 503, description = "The job queue is full.")]
    Unavailable(Problem),
    #[response(status = 504, description = "A known operation or helper deadline was reached.")]
    Timeout(Problem),
}
