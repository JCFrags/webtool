//! HTTP failures are explicit and never expose request bodies, URLs, or helper stderr.
//! The engine still returns anyhow errors. The small legacy adapter below is not
//! a typed engine error model and must be removed when that model is available.
use axum::{
    extract::{rejection::JsonRejection, FromRequest, FromRequestParts, Multipart, Path, Query, Request},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::de::DeserializeOwned;
use webtool_protocol::Problem;

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self { status, code, message: message.into() }
    }

    pub(crate) fn bad_request(message: &'static str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    pub(crate) fn size_limit() -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, "size_limit", "Request, upload, or source exceeds its byte limit.")
    }

    fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "The server could not complete the operation.")
    }

    /// Reuse the same safe mapping for ordered batch input failures.
    pub(crate) fn into_problem(self) -> (u16, Problem) {
        (self.status.as_u16(), Problem { code: self.code.into(), message: self.message })
    }

    fn rejection(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        if status == StatusCode::PAYLOAD_TOO_LARGE {
            Self::size_limit()
        } else if status.is_server_error() {
            Self::internal()
        } else {
            Self::new(status, code, message)
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(Problem { code: self.code.into(), message: self.message.into() })).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        if let Some(domain) = error.downcast_ref::<webtool_engine::code::CodeError>() {
            return Self { status: StatusCode::from_u16(domain.status_code()).expect("domain status"),
                code: domain.code(), message: domain.to_string() };
        }
        if let Some(domain) = error.downcast_ref::<webtool_engine::scholarly::pmc::PmcError>() {
            return Self { status: StatusCode::from_u16(domain.http_status()).expect("PMC status"),
                code: domain.code(), message: domain.to_string() };
        }
        if let Some(domain) = error.downcast_ref::<webtool_engine::external_code::ExternalCodeError>() {
            return Self { status: StatusCode::from_u16(domain.status()).expect("domain status"),
                code: domain.code(), message: domain.to_string() };
        }
        if let Some(error) = error.downcast_ref::<webtool_engine::scholarly::ScholarlyError>() {
            return Self::new(StatusCode::from_u16(error.http_status()).expect("valid scholarly status"), error.code(), error.message());
        }
        // Match only known prefixes or exact engine validation messages. Broad
        // substrings such as "not found", "requires", or "timeout" can hide bugs.
        for cause in error.chain() {
            if let Some(mapped) = legacy_engine_error(&cause.to_string()) {
                return mapped;
            }
        }
        // Do not log the error text here: it can contain signed URLs or helper output.
        tracing::error!("Unhandled engine error at the HTTP boundary");
        Self::internal()
    }
}

fn legacy_engine_error(message: &str) -> Option<ApiError> {
    use StatusCode as S;
    let prefix = message.split_once(':').map(|(code, _)| code).unwrap_or(message);
    let (status, code, safe) = match prefix {
        "archive_invalid_request" => (S::BAD_REQUEST, "archive_invalid_request", "Use one original HTTP(S) URL, an exact UTC timestamp YYYYMMDDhhmmss, a lookback of 0 to 3660 days, and 1 to 3 candidates."),
        "archive_timeout" => (S::GATEWAY_TIMEOUT, "archive_timeout", "The archive operation deadline was reached."),
        "archive_rate_limited" => (S::TOO_MANY_REQUESTS, "archive_rate_limited", "The archive requested a cooldown. No retry was attempted."),
        "archive_size_limit" => (S::PAYLOAD_TOO_LARGE, "archive_size_limit", "The archive response exceeds its byte limit."),
        "archive_unavailable" => (S::NOT_FOUND, "archive_unavailable", "The exact capture is unavailable or the index reports an unsuccessful capture. No alternate capture was read."),
        "archive_redirect_refused" => (S::BAD_GATEWAY, "archive_redirect_refused", "The archive returned a redirect. It was not followed, including live-web or different-date targets."),
        "archive_identity_mismatch" => (S::BAD_GATEWAY, "archive_identity_mismatch", "Replay identity does not confirm the selected original URL and capture time."),
        "archive_upstream_failed" => (S::BAD_GATEWAY, "archive_upstream_failed", "The archive request or capture index validation failed."),
        "archive_content_unavailable" => (S::UNPROCESSABLE_ENTITY, "archive_content_unavailable", "The capture contains no usable HTTP text or displays an access challenge. No fallback was attempted."),
        "archive_format_unsupported" => (S::UNPROCESSABLE_ENTITY, "archive_format_unsupported", "This archive route supports HTML, XHTML, plain text, and Markdown only."),
        "arxiv_rate_limited" => (S::TOO_MANY_REQUESTS, "arxiv_rate_limited", "arXiv rate limit reached."),
        "arxiv_version_unavailable" | "arxiv_empty_result" => (S::NOT_FOUND, "arxiv_unavailable", "The requested arXiv paper or version is unavailable."),
        "arxiv_invalid_identifier" => (S::BAD_REQUEST, "arxiv_invalid_identifier", "Use a supported arXiv identifier with an optional version."),
        "arxiv_identity_mismatch" => (S::BAD_GATEWAY, "arxiv_identity_mismatch", "arXiv returned a different paper identity or version."),
        "arxiv_invalid_metadata" | "arxiv_metadata_failed" => (S::BAD_GATEWAY, "arxiv_metadata_failed", "The arXiv metadata could not be retrieved or validated."),
        "arxiv_reuse_not_established" => (S::UNPROCESSABLE_ENTITY, "arxiv_reuse_not_established", "The selected version has no supported full-text reuse basis. Use scholarly arXiv inspection for metadata and links."),
        "arxiv_pdf_failed" | "arxiv_full_text_failed" => (S::BAD_GATEWAY, "arxiv_full_text_failed", "The arXiv full text could not be retrieved or read."),
        "citation_metadata_unavailable" | "citation_format_unsupported" => (S::UNPROCESSABLE_ENTITY, "citation_unavailable", "Saved citation metadata or the requested format is unavailable."),
        "github_rate_limited" => (S::TOO_MANY_REQUESTS, "github_rate_limited", "GitHub rate limit reached."),
        "github_access_denied" => (S::BAD_GATEWAY, "github_access_denied", "GitHub denied source access."),
        "github_reference_ambiguous" => (S::UNPROCESSABLE_ENTITY, "github_reference_ambiguous", "Use an explicit Git commit or an encoded ref boundary."),
        "github_reference_limit" | "github_path_limit" => (S::UNPROCESSABLE_ENTITY, "github_resolution_limit", "The GitHub resolution limit was reached."),
        "github_reference_not_found" => (S::NOT_FOUND, "github_reference_not_found", "The GitHub reference was not found."),
        "github_repository_unavailable" => (S::NOT_FOUND, "github_repository_unavailable", "The GitHub repository is unavailable."),
        "github_path_not_found" | "github_path_missing" => (S::NOT_FOUND, "github_path_not_found", "The GitHub path was not found."),
        "github_listing_incomplete" => (S::BAD_GATEWAY, "github_listing_incomplete", "GitHub returned an incomplete listing."),
        "github_unsupported_object" => (S::UNPROCESSABLE_ENTITY, "github_unsupported_object", "This GitHub object is not supported."),
        "github_content_unavailable" | "github_readme_unavailable" => (S::UNPROCESSABLE_ENTITY, "github_content_unavailable", "The GitHub source content is unavailable."),
        "github_invalid_response" | "github_api_error" => (S::BAD_GATEWAY, "github_api_error", "The GitHub API request failed."),
        "html_input_size_limit" | "html_decoded_size_limit" => (S::PAYLOAD_TOO_LARGE, "size_limit", "HTML input or decoded text exceeds the configured byte limit."),
        "html_encoding_unsupported" | "xhtml_encoding_unsupported" => (S::UNPROCESSABLE_ENTITY, "encoding_unsupported", "The source character encoding is not supported by this reader."),
        "html_encoding_invalid" | "rendered_encoding_invalid" => (S::UNPROCESSABLE_ENTITY, "encoding_invalid", "The source does not meet the reader's character encoding requirements."),
        "browser_helper_missing" => (S::UNPROCESSABLE_ENTITY, "browser_helper_missing", "The requested browser helper is not configured."),
        "browser_timeout" => (S::GATEWAY_TIMEOUT, "browser_timeout", "The browser helper deadline was reached."),
        "browser_size_limit" => (S::PAYLOAD_TOO_LARGE, "browser_size_limit", "Browser output exceeds the configured byte limit."),
        "browser_navigation_failed" => (S::BAD_GATEWAY, "browser_navigation_failed", "Browser navigation failed."),
        "browser_empty_output" => (S::UNPROCESSABLE_ENTITY, "browser_empty_output", "The browser returned no readable output."),
        "browser_invalid_output" => (S::BAD_GATEWAY, "browser_invalid_output", "The browser returned invalid output."),
        "media_helper_missing" => (S::UNPROCESSABLE_ENTITY, "media_helper_missing", "The media helper is not configured."),
        "video_invalid_query" => (S::BAD_REQUEST, "video_invalid_query", "Supply a nonempty literal video query of at most 4096 bytes without NUL."),
        "video_invalid_limit" => (S::BAD_REQUEST, "video_invalid_limit", "Video search limit must be between 1 and 20."),
        "media_rate_limited" => (S::TOO_MANY_REQUESTS, "media_rate_limited", "The media source rate limit was reached. No fallback was attempted."),
        "media_size_limit" => (S::PAYLOAD_TOO_LARGE, "media_size_limit", "Media helper output exceeds the configured byte limit."),
        "media_identity_mismatch" => (S::BAD_GATEWAY, "media_identity_mismatch", "The media helper returned a different or inconsistent video identity."),
        "media_source_blocked" => (S::BAD_GATEWAY, "media_source_blocked", "The media source denied access."),
        "media_captions_unavailable" => (S::UNPROCESSABLE_ENTITY, "media_captions_unavailable", "The requested captions are unavailable."),
        "media_captions_malformed" => (S::UNPROCESSABLE_ENTITY, "media_captions_malformed", "The supplied captions could not be read."),
        "media_helper_timeout" => (S::GATEWAY_TIMEOUT, "media_helper_timeout", "The media helper deadline was reached."),
        "media_helper_failed" | "media_track_download_failed" => (S::BAD_GATEWAY, "media_helper_failed", "The media helper request failed."),
        "invalid CSS selector" => (S::BAD_REQUEST, "invalid_request", "Use a valid CSS selector."),
        "read_timeout" => (S::GATEWAY_TIMEOUT, "timeout", "The read deadline was reached."),
        "read_content_blocked" => (S::UNPROCESSABLE_ENTITY, "read_content_blocked", "The source requires access or displays a challenge rather than readable content."),
        "read_content_unavailable" | "browser_content_unavailable" => (S::UNPROCESSABLE_ENTITY, "read_content_unavailable", "No usable source content was available."),
        _ => return legacy_validation_error(message),
    };
    Some(ApiError::new(status, code, safe))
}

fn legacy_validation_error(message: &str) -> Option<ApiError> {
    use StatusCode as S;
    match message {
        "document not found" | "library not found" | "job not found" =>
            Some(ApiError::new(S::NOT_FOUND, "not_found", "The requested saved resource was not found.")),
        "invalid URL" | "only absolute HTTP and HTTPS URLs are accepted" | "credentials embedded in URLs are not supported" =>
            Some(ApiError::bad_request("Use an absolute HTTP or HTTPS URL without embedded credentials.")),
        "upload name must be a filename, not a path" => Some(ApiError::bad_request("Upload name must be a filename of 1 to 256 bytes, not a path.")),
        "library names must contain 1 to 80 ASCII letters, digits, hyphens, or underscores" =>
            Some(ApiError::bad_request("Library names must contain 1 to 80 ASCII letters, digits, hyphens, or underscores.")),
        "find query must contain 1 to 4096 bytes" | "find limit must be between 1 and 1000" | "invalid find pattern" =>
            Some(ApiError::bad_request("Use a valid find pattern of 1 to 4096 bytes and a limit from 1 to 1000.")),
        "search query must not be empty" | "query must contain 1 to 4096 bytes" | "search limit must be between 1 and 50" =>
            Some(ApiError::bad_request("Use a nonempty search query of up to 4096 bytes and a public search limit from 1 to 50.")),
        "caption language must be a literal language code, such as en or en-US" =>
            Some(ApiError::bad_request("Caption language must be a literal language code, such as en or en-US.")),
        "map limit must be between 1 and 5000" => Some(ApiError::bad_request("Map limit must be between 1 and 5000.")),
        "crawl limits are 1 to 500 pages and 0 to 8 levels" => Some(ApiError::bad_request("Crawl limits are 1 to 500 pages and 0 to 8 levels.")),
        "job queue is full" => Some(ApiError::new(S::SERVICE_UNAVAILABLE, "queue_full", "The job queue is full. Try again later.")),
        _ if message.starts_with("crawl_invalid_request:") => Some(ApiError::bad_request("Use bounded same-origin crawl inputs. Resume may only increase the total attempt budget, up to 500.")),
        _ if message.starts_with("crawl_not_resumable:") => Some(ApiError::new(S::CONFLICT, "crawl_not_resumable", "Resume requires a durable interrupted, cancelled, or partial job with pending work and remaining budget. Increase max_pages if needed.")),
        "actor must not be empty" | "too many or oversized tags" => Some(ApiError::bad_request("Supply an actor and at most 64 tags of up to 100 bytes each.")),
        "upload exceeds configured size limit" | "note exceeds 65536 bytes" | "citation response exceeds one megabyte" | "caption file exceeds configured byte limit" | "rendered page exceeds configured size limit" => Some(ApiError::size_limit()),
        "expected a DOI such as 10.1234/example" | "format must be bibtex, ris, or csl" => Some(ApiError::bad_request("Supply a DOI or saved paper ID and a supported citation format: bibtex, ris, or csl.")),
        "a JSON pointer expression is required" | "a CSS selector expression is required" => Some(ApiError::bad_request("This extraction kind requires an expression.")),
        "original is not a JSON document" | "CSS extraction requires an HTML original" => Some(ApiError::new(S::UNPROCESSABLE_ENTITY, "capability_unavailable", "The extraction kind does not match the saved original format.")),
        "fastCRW requires a server built with --features crw-browser" | "Chromium is not configured. Set chromium_path in the server config." |
        "missing crw_renderer config" | "OCR is unavailable in this server build; document_config requires OCR" |
        "binary documents require a server built with --features documents; scanned images additionally require OCR models" |
        "HTML content selection requires the web-extraction feature or an explicit selector" | "this server was built without web-search" =>
            Some(ApiError::new(S::UNPROCESSABLE_ENTITY, "capability_unavailable", "This operation needs a server feature or helper that is not available.")),
        "unsupported captions request: use a YouTube watch/youtu.be URL without a CSS selector" |
        "unsupported media URL; use a YouTube watch or youtu.be URL" |
        "unsupported YouTube video ID; use a watch or youtu.be URL for one video" =>
            Some(ApiError::new(S::UNPROCESSABLE_ENTITY, "capability_unavailable", "Use a YouTube watch or youtu.be URL for one video without a CSS selector.")),
        "text input contains NUL bytes; unsupported encoding or binary format" =>
            Some(ApiError::new(S::UNPROCESSABLE_ENTITY, "capability_unavailable", "The source encoding or binary format is not supported by this reader.")),
        "CSS selector matched no elements" | "no readable blocks found for the explicit CSS selector" =>
            Some(ApiError::bad_request("The CSS selector matched no readable content.")),
        "fetch source" => Some(ApiError::new(S::BAD_GATEWAY, "upstream_error", "The source request failed.")),
        _ if message.starts_with("unsupported format ") && message.ends_with("; install a server built with the documents feature") =>
            Some(ApiError::new(S::UNPROCESSABLE_ENTITY, "capability_unavailable", "This format needs a server built with document support.")),
        _ if message.strip_prefix("source returned HTTP ").is_some_and(|s| s.parse::<u16>().is_ok_and(|n| (100..=599).contains(&n))) =>
            Some(ApiError::new(S::BAD_GATEWAY, "upstream_error", "The source returned an unsuccessful HTTP status.")),
        _ if message.starts_with("source exceeds the ") && (message.ends_with("-byte limit") || message.ends_with("-byte limit after decompression")) => Some(ApiError::size_limit()),
        _ => None,
    }
}

pub(crate) struct ApiJson<T>(pub T);
impl<S, T> FromRequest<S> for ApiJson<T>
where S: Send + Sync, T: DeserializeOwned {
    type Rejection = ApiError;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state).await.map(|Json(value)| Self(value)).map_err(|error| {
            let (code, message) = match &error {
                JsonRejection::MissingJsonContentType(_) => ("unsupported_media_type", "Use Content-Type: application/json."),
                JsonRejection::JsonSyntaxError(_) => ("invalid_json", "The request body is not valid JSON."),
                JsonRejection::JsonDataError(_) => ("invalid_request", "JSON fields do not match the request schema."),
                _ => ("invalid_body", "The request body could not be read."),
            };
            ApiError::rejection(error.status(), code, message)
        })
    }
}

pub(crate) struct ApiQuery<T>(pub T);
impl<S, T> FromRequestParts<S> for ApiQuery<T>
where S: Send + Sync, T: DeserializeOwned {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state).await.map(|Query(value)| Self(value))
            .map_err(|error| ApiError::rejection(error.status(), "invalid_query", "Query parameters do not match the request schema."))
    }
}

pub(crate) struct ApiPath<T>(pub T);
impl<S, T> FromRequestParts<S> for ApiPath<T>
where S: Send + Sync, T: DeserializeOwned + Send {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Path::<T>::from_request_parts(parts, state).await.map(|Path(value)| Self(value))
            .map_err(|error| ApiError::rejection(error.status(), "invalid_path", "Path parameters do not match the request schema."))
    }
}

pub(crate) struct ApiMultipart(pub Multipart);
impl<S: Send + Sync> FromRequest<S> for ApiMultipart {
    type Rejection = ApiError;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Multipart::from_request(request, state).await.map(Self)
            .map_err(|error| ApiError::rejection(error.status(), "invalid_multipart", "Use multipart/form-data with a valid boundary."))
    }
}
impl From<axum::extract::multipart::MultipartError> for ApiError {
    fn from(error: axum::extract::multipart::MultipartError) -> Self {
        Self::rejection(error.status(), "invalid_multipart", "The multipart body is incomplete or malformed.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_adapter_preserves_known_codes_without_reflecting_details() {
        let error = ApiError::from(anyhow::anyhow!("browser_timeout: private helper output"));
        assert_eq!(error.status, StatusCode::GATEWAY_TIMEOUT);
        assert_eq!(error.code, "browser_timeout");
        assert!(!error.message.contains("private"));
        let error = ApiError::from(anyhow::anyhow!("document not found"));
        assert_eq!(error.status, StatusCode::NOT_FOUND);
        let error = ApiError::from(anyhow::anyhow!("html_decoded_size_limit: decoded HTML exceeds 4096 UTF-8 bytes"));
        assert_eq!(error.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(error.code, "size_limit");
    }

    #[test]
    fn unknown_internal_errors_are_not_client_errors() {
        for message in ["database file not found", "transaction requires repair", "internal timeout bookkeeping", "unknown fault"] {
            let error = ApiError::from(anyhow::anyhow!(message));
            assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(error.code, "internal_error");
            assert!(error.message.len() < 128);
        }
    }
}
