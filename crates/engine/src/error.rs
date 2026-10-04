//! Typed public failures. Details and underlying causes stay inside the engine.
//! `anyhow` can carry these values through internal contexts, but text never
//! selects a public status, code, or message.
use std::{error::Error, fmt};
use crate::{code::CodeError, external_code::ExternalCodeError,
    scholarly::{ScholarlyError, pmc::PmcError}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    InvalidInput, NotFound, Conflict, SizeLimit, CapabilityUnavailable,
    RateLimited, Upstream, Unavailable, Timeout, Parse, Storage, Internal,
}

/// Response intent is independent of Axum or another client adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StatusIntent {
    InvalidInput = 400, NotFound = 404, Conflict = 409, SizeLimit = 413,
    CapabilityUnavailable = 422, RateLimited = 429, Internal = 500,
    Upstream = 502, Unavailable = 503, Timeout = 504,
}
impl StatusIntent {
    pub fn http_status(self) -> u16 { self as u16 }
    fn from_domain(status: u16) -> Self { match status {
        400 => Self::InvalidInput, 404 => Self::NotFound, 409 => Self::Conflict,
        413 => Self::SizeLimit, 422 => Self::CapabilityUnavailable, 429 => Self::RateLimited,
        502 => Self::Upstream, 503 => Self::Unavailable, 504 => Self::Timeout,
        _ => Self::Internal,
    } }
    pub fn category(self) -> ErrorCategory { match self {
        Self::InvalidInput => ErrorCategory::InvalidInput, Self::NotFound => ErrorCategory::NotFound,
        Self::Conflict => ErrorCategory::Conflict, Self::SizeLimit => ErrorCategory::SizeLimit,
        Self::CapabilityUnavailable => ErrorCategory::CapabilityUnavailable,
        Self::RateLimited => ErrorCategory::RateLimited, Self::Upstream => ErrorCategory::Upstream,
        Self::Unavailable => ErrorCategory::Unavailable, Self::Timeout => ErrorCategory::Timeout,
        Self::Internal => ErrorCategory::Internal,
    } }
}

#[derive(Debug, Clone, Copy)]
pub struct PublicError {
    pub status: StatusIntent,
    pub code: &'static str,
    pub message: &'static str,
}

macro_rules! error_kinds {
    ($($kind:ident => ($status:ident, $code:literal, $message:literal),)*) => {
        #[derive(Debug, Clone, Copy)]
        pub enum ErrorKind {
            $($kind,)*
            /// Actual upstream status is retained internally, not confused with
            /// the status of this service's failure response.
            SourceHttpStatus(u16),
            Code(CodeError), ExternalCode(ExternalCodeError),
            Scholarly(ScholarlyError), Pmc(PmcError),
        }
        impl ErrorKind {
            pub fn public(self) -> PublicError { match self {
                $(Self::$kind => PublicError { status: StatusIntent::$status, code: $code, message: $message },)*
                Self::SourceHttpStatus(status) => PublicError { status: StatusIntent::Upstream,
                    code: "upstream_error", message: match status {
                        403 => "The source returned HTTP 403 (Forbidden).",
                        404 => "The source returned HTTP 404 (Not Found).",
                        429 => "The source returned HTTP 429 (Too Many Requests).",
                        _ => "The source returned an unsuccessful HTTP status.",
                    } },
                Self::Code(e) => PublicError { status: StatusIntent::from_domain(e.status_code()), code: e.code(), message: e.message() },
                Self::ExternalCode(e) => PublicError { status: StatusIntent::from_domain(e.status()), code: e.code(), message: e.message() },
                Self::Scholarly(e) => PublicError { status: StatusIntent::from_domain(e.http_status()), code: e.code(), message: e.message() },
                Self::Pmc(e) => PublicError { status: StatusIntent::from_domain(e.http_status()), code: e.code(), message: e.message() },
            } }
        }
    }
}

error_kinds! {
    ArchiveInvalidRequest => (InvalidInput, "archive_invalid_request", "Use one original HTTP(S) URL, an exact UTC timestamp YYYYMMDDhhmmss, a lookback of 0 to 3660 days, and 1 to 3 candidates."),
    ArchiveTimeout => (Timeout, "archive_timeout", "The archive operation deadline was reached."),
    ArchiveRateLimited => (RateLimited, "archive_rate_limited", "The archive requested a cooldown. No retry was attempted."),
    ArchiveSizeLimit => (SizeLimit, "archive_size_limit", "The archive response exceeds its byte limit."),
    ArchiveUnavailable => (NotFound, "archive_unavailable", "The exact capture is unavailable or the index reports an unsuccessful capture. No alternate capture was read."),
    ArchiveRedirectRefused => (Upstream, "archive_redirect_refused", "The archive returned a redirect. It was not followed, including live-web or different-date targets."),
    ArchiveIdentityMismatch => (Upstream, "archive_identity_mismatch", "Replay identity does not confirm the selected original URL and capture time."),
    ArchiveUpstreamFailed => (Upstream, "archive_upstream_failed", "The archive request or capture index validation failed."),
    ArchiveContentUnavailable => (CapabilityUnavailable, "archive_content_unavailable", "The capture contains no usable HTTP text or displays an access challenge. No fallback was attempted."),
    ArchiveFormatUnsupported => (CapabilityUnavailable, "archive_format_unsupported", "This archive route supports HTML, XHTML, plain text, and Markdown only."),
    ArxivRateLimited => (RateLimited, "arxiv_rate_limited", "arXiv rate limit reached."),
    ArxivVersionUnavailable => (NotFound, "arxiv_unavailable", "The requested arXiv paper or version is unavailable."),
    ArxivInvalidIdentifier => (InvalidInput, "arxiv_invalid_identifier", "Use a supported arXiv identifier with an optional version."),
    ArxivIdentityMismatch => (Upstream, "arxiv_identity_mismatch", "arXiv returned a different paper identity or version."),
    ArxivInvalidMetadata => (Upstream, "arxiv_metadata_failed", "The arXiv metadata could not be retrieved or validated."),
    ArxivReuseNotEstablished => (CapabilityUnavailable, "arxiv_reuse_not_established", "The selected version has no supported full-text reuse basis. Use scholarly arXiv inspection for metadata and links."),
    ArxivPdfFailed => (Upstream, "arxiv_full_text_failed", "The arXiv full text could not be retrieved or read."),
    CitationMetadataUnavailable => (CapabilityUnavailable, "citation_unavailable", "Saved citation metadata or the requested format is unavailable."),
    GithubRateLimited => (RateLimited, "github_rate_limited", "GitHub rate limit reached."),
    GithubAccessDenied => (Upstream, "github_access_denied", "GitHub denied source access."),
    GithubReferenceAmbiguous => (CapabilityUnavailable, "github_reference_ambiguous", "Use an explicit Git commit or an encoded ref boundary."),
    GithubReferenceLimit => (CapabilityUnavailable, "github_resolution_limit", "The GitHub resolution limit was reached."),
    GithubReferenceNotFound => (NotFound, "github_reference_not_found", "The GitHub reference was not found."),
    GithubRepositoryUnavailable => (NotFound, "github_repository_unavailable", "The GitHub repository is unavailable."),
    GithubPathNotFound => (NotFound, "github_path_not_found", "The GitHub path was not found."),
    GithubListingIncomplete => (Upstream, "github_listing_incomplete", "GitHub returned an incomplete listing."),
    GithubUnsupportedObject => (CapabilityUnavailable, "github_unsupported_object", "This GitHub object is not supported."),
    GithubContentUnavailable => (CapabilityUnavailable, "github_content_unavailable", "The GitHub source content is unavailable."),
    GithubInvalidResponse => (Upstream, "github_api_error", "The GitHub API request failed."),
    HtmlInputSizeLimit => (SizeLimit, "size_limit", "HTML input or decoded text exceeds the configured byte limit."),
    HtmlEncodingUnsupported => (CapabilityUnavailable, "encoding_unsupported", "The source character encoding is not supported by this reader."),
    HtmlEncodingInvalid => (CapabilityUnavailable, "encoding_invalid", "The source does not meet the reader's character encoding requirements."),
    BrowserHelperMissing => (CapabilityUnavailable, "browser_helper_missing", "The requested browser helper is not configured."),
    BrowserTimeout => (Timeout, "browser_timeout", "The browser helper deadline was reached."),
    BrowserSizeLimit => (SizeLimit, "browser_size_limit", "Browser output exceeds the configured byte limit."),
    BrowserNavigationFailed => (Upstream, "browser_navigation_failed", "Browser navigation failed."),
    BrowserEmptyOutput => (CapabilityUnavailable, "browser_empty_output", "The browser returned no readable output."),
    BrowserInvalidOutput => (Upstream, "browser_invalid_output", "The browser returned invalid output."),
    MediaDownloadDisabled => (CapabilityUnavailable, "media_download_disabled", "Media preview and download are disabled until an operator supplies explicit finite budgets and helper configuration."),
    MediaDownloadHelpersMissing => (CapabilityUnavailable, "media_download_helpers_missing", "Media jobs require configured existing absolute yt-dlp, ffmpeg, and ffprobe executables and Linux supervision."),
    MediaDownloadInvalid => (InvalidInput, "media_download_invalid", "Supply one video identity, exact preview format IDs and identities, and finite bounds within operator ceilings."),
    MediaFormatUnavailable => (CapabilityUnavailable, "media_format_unavailable", "The selected single-media format is absent, changed, unsupported, or exceeds constraints. Live and upcoming captures are unsupported."),
    MediaDeadline => (Timeout, "media_deadline", "The media operation deadline was reached."),
    MediaOutputLimit => (SizeLimit, "media_budget_exceeded", "Media output, staging, storage reservation, or free-space limit was reached."),
    MediaBudgetExceeded => (SizeLimit, "media_budget_exceeded", "Media output, staging, storage reservation, or free-space limit was reached."),
    MediaValidationFailed => (Upstream, "media_validation_failed", "Actual media or owned staging could not be safely validated."),
    MediaStagingUnsafe => (Upstream, "media_validation_failed", "Actual media or owned staging could not be safely validated."),
    MediaResumeUnsupported => (Conflict, "media_resume_unsupported", "Compatible media resume is unsupported. Submit a new preview-checked request."),
    MediaArtifactNotFound => (NotFound, "media_artifact_not_found", "The artifact is not retained by the requested job."),
    MediaHelperMissing => (CapabilityUnavailable, "media_helper_missing", "The media helper is not configured."),
    VideoInvalidQuery => (InvalidInput, "video_invalid_query", "Supply a nonempty literal video query of at most 4096 bytes without NUL."),
    VideoInvalidLimit => (InvalidInput, "video_invalid_limit", "Video search limit must be between 1 and 20."),
    MediaRateLimited => (RateLimited, "media_rate_limited", "The media source rate limit was reached. No fallback was attempted."),
    MediaSizeLimit => (SizeLimit, "media_size_limit", "Media helper output exceeds the configured byte limit."),
    MediaIdentityMismatch => (Upstream, "media_identity_mismatch", "The media helper returned a different or inconsistent video identity."),
    MediaSourceBlocked => (Upstream, "media_source_blocked", "The media source denied access."),
    MediaCaptionsUnavailable => (CapabilityUnavailable, "media_captions_unavailable", "The requested captions are unavailable."),
    MediaCaptionsMalformed => (CapabilityUnavailable, "media_captions_malformed", "The supplied captions could not be read."),
    MediaHelperTimeout => (Timeout, "media_helper_timeout", "The media helper deadline was reached."),
    MediaHelperFailed => (Upstream, "media_helper_failed", "The media helper request failed."),
    InvalidCss => (InvalidInput, "invalid_request", "Use a valid CSS selector."),
    ReadTimeout => (Timeout, "timeout", "The read deadline was reached."),
    ReadContentBlocked => (CapabilityUnavailable, "read_content_blocked", "The source requires access or displays a challenge rather than readable content."),
    ReadContentUnavailable => (CapabilityUnavailable, "read_content_unavailable", "No usable source content was available."),
    Internal => (Internal, "internal_error", "The server could not complete the operation."),
    StorageFault => (Internal, "internal_error", "The server could not complete the operation."),
    MissingResource => (NotFound, "not_found", "The requested saved resource was not found."),
    InvalidUrl => (InvalidInput, "invalid_request", "Use an absolute HTTP or HTTPS URL without embedded credentials."),
    InvalidUploadName => (InvalidInput, "invalid_request", "Upload name must be a filename of 1 to 256 bytes, not a path."),
    InvalidLibrary => (InvalidInput, "invalid_request", "Library names must contain 1 to 80 ASCII letters, digits, hyphens, or underscores."),
    InvalidFind => (InvalidInput, "invalid_request", "Use a valid find pattern of 1 to 4096 bytes and a limit from 1 to 1000."),
    InvalidSearch => (InvalidInput, "invalid_request", "Use a nonempty search query of up to 4096 bytes and a public search limit from 1 to 50."),
    InvalidLanguage => (InvalidInput, "invalid_request", "Caption language must be a literal language code, such as en or en-US."),
    InvalidMap => (InvalidInput, "invalid_request", "Map limit must be between 1 and 5000."),
    InvalidCrawlLimits => (InvalidInput, "invalid_request", "Crawl limits are 1 to 500 pages and 0 to 8 levels."),
    CrawlInvalidRequest => (InvalidInput, "invalid_request", "Use bounded same-origin crawl inputs. Resume may only increase the total attempt budget, up to 500."),
    CrawlNotResumable => (Conflict, "crawl_not_resumable", "Resume requires a durable interrupted, cancelled, or partial job with pending work and remaining budget. Increase max_pages if needed."),
    CrawlPolicyRefused => (CapabilityUnavailable, "crawl_policy_refused", "The crawl policy cannot be followed within the configured limits. Permission was not assumed."),
    CrawlMetadataUnsupported => (CapabilityUnavailable, "crawl_metadata_unsupported", "Compressed crawl metadata is not supported. Permission was not assumed."),
    CrawlResultRejected => (Upstream, "crawl_result_rejected", "The crawl result is empty, excluded, or outside its scope. It was not attached."),
    QueueFull => (Unavailable, "queue_full", "The job queue is full. Try again later."),
    InvalidAnnotation => (InvalidInput, "invalid_request", "Supply an actor and at most 64 tags of up to 100 bytes each."),
    SizeLimit => (SizeLimit, "size_limit", "Request, upload, or source exceeds its byte limit."),
    InvalidCitation => (InvalidInput, "invalid_request", "Supply a DOI or saved paper ID and a supported citation format: bibtex, ris, or csl."),
    MissingExpression => (InvalidInput, "invalid_request", "This extraction kind requires an expression."),
    OriginalFormatMismatch => (CapabilityUnavailable, "capability_unavailable", "The extraction kind does not match the saved original format."),
    CapabilityUnavailable => (CapabilityUnavailable, "capability_unavailable", "This operation needs a server feature or helper that is not available."),
    UnsupportedCaptions => (CapabilityUnavailable, "capability_unavailable", "Use a YouTube watch or youtu.be URL for one video without a CSS selector."),
    UnsupportedText => (CapabilityUnavailable, "capability_unavailable", "The source encoding or binary format is not supported by this reader."),
    UnsupportedFormat => (CapabilityUnavailable, "capability_unavailable", "This format is not supported by the selected reader or server build."),
    EmptyCss => (InvalidInput, "invalid_request", "The CSS selector matched no readable content."),
    SourceRequestFailed => (Upstream, "upstream_error", "The source request failed."),
    ParseInvalid => (CapabilityUnavailable, "parse_invalid", "The source is not valid for the selected reader."),
    ParseFailed => (CapabilityUnavailable, "parse_failed", "The selected reader could not extract usable source content."),
    MediaCancelled => (Conflict, "media_cancelled", "Media work was cancelled. Saved artifacts remain available."),
}

impl ErrorKind {
    pub fn category(self) -> ErrorCategory {
        match self {
            Self::StorageFault => ErrorCategory::Storage,
            Self::ParseInvalid | Self::ParseFailed | Self::HtmlEncodingInvalid | Self::HtmlEncodingUnsupported => ErrorCategory::Parse,
            _ => self.public().status.category(),
        }
    }
    pub fn error(self) -> EngineError { EngineError { kind: self, cause: None } }
    /// Diagnostic detail is a source, never the public message.
    pub fn context(self, detail: impl Into<String>) -> EngineError {
        EngineError { kind: self, cause: Some(anyhow::anyhow!(detail.into())) }
    }
    pub fn with_source(self, cause: impl Into<anyhow::Error>) -> EngineError {
        EngineError { kind: self, cause: Some(cause.into()) }
    }
}

#[derive(Debug)]
pub struct EngineError { kind: ErrorKind, cause: Option<anyhow::Error> }
impl EngineError {
    pub fn kind(&self) -> ErrorKind { self.kind }
    pub fn category(&self) -> ErrorCategory { self.kind.category() }
    pub fn public(&self) -> PublicError { self.kind.public() }
    pub fn code(&self) -> &'static str { self.public().code }
    pub fn status(&self) -> StatusIntent { self.public().status }
    pub fn public_message(&self) -> &'static str { self.public().message }
    /// Downcasts only. A string, even one identical to a known code/message,
    /// is an unknown internal fault. Outermost typed context owns the intent.
    pub fn from_anyhow(error: anyhow::Error) -> Self {
        let kind = kind(&error).unwrap_or(ErrorKind::Internal);
        Self { kind, cause: Some(error) }
    }
}
impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.public().message) }
}
impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> { self.cause.as_ref().map(|cause| cause.as_ref()) }
}

/// Recover a typed producer decision without inspecting Display text.
pub fn kind(error: &anyhow::Error) -> Option<ErrorKind> {
    if let Some(e) = error.downcast_ref::<EngineError>() { return Some(e.kind()); }
    if let Some(e) = error.downcast_ref::<CodeError>() { return Some(ErrorKind::Code(*e)); }
    if let Some(e) = error.downcast_ref::<ExternalCodeError>() { return Some(ErrorKind::ExternalCode(*e)); }
    if let Some(e) = error.downcast_ref::<ScholarlyError>() { return Some(ErrorKind::Scholarly(*e)); }
    if let Some(e) = error.downcast_ref::<PmcError>() { return Some(ErrorKind::Pmc(*e)); }
    // Preserve the existing distinct Auto-recovery signal. The HTTP boundary
    // does not initiate recovery or interpret this error's diagnostic code.
    if error.is::<crate::readers::html::MissingContent>() { return Some(ErrorKind::ReadContentUnavailable); }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Context;
    #[test]
    fn context_retains_intent_and_private_causes() {
        let source = anyhow::anyhow!("PRIVATE_MARKER /private/path token");
        let error = Err::<(), _>(source).context(ErrorKind::InvalidUrl.error()).unwrap_err()
            .context("outer internal context");
        let typed = EngineError::from_anyhow(error);
        assert_eq!(typed.public().code, "invalid_request");
        assert_eq!(typed.public().status, StatusIntent::InvalidInput);
        assert!(!typed.to_string().contains("PRIVATE_MARKER"));
        assert!(typed.source().unwrap().to_string().contains("outer internal context"));
        assert!(typed.cause.as_ref().unwrap().chain().any(|cause| cause.to_string().contains("PRIVATE_MARKER")));
        let wrapped = ErrorKind::BrowserNavigationFailed.with_source(anyhow::anyhow!("PRIVATE_MARKER"));
        assert!(wrapped.source().unwrap().to_string().contains("PRIVATE_MARKER"));
        assert!(!wrapped.to_string().contains("PRIVATE_MARKER"));
    }
    #[test]
    fn outer_typed_context_replaces_inner_intent() {
        let error = anyhow::Error::from(ErrorKind::InvalidUrl.error())
            .context(ErrorKind::BrowserInvalidOutput.error());
        assert!(matches!(kind(&error), Some(ErrorKind::BrowserInvalidOutput)));
    }
    #[test]
    fn supplied_source_status_is_safe_and_distinct_from_service_status() {
        for status in [403, 404, 429] {
            let error = ErrorKind::SourceHttpStatus(status).context("PRIVATE_MARKER /private/path");
            assert_eq!(error.status(), StatusIntent::Upstream);
            assert_eq!(error.code(), "upstream_error");
            assert!(error.public_message().contains(&format!("HTTP {status}")));
            assert!(!error.public_message().contains("PRIVATE_MARKER"));
        }
    }
    #[test]
    fn arbitrary_strings_never_classify_faults() {
        for text in ["document not found", "browser_timeout: secret", "invalid URL", "source returned HTTP 404"] {
            let error = EngineError::from_anyhow(anyhow::anyhow!(text));
            assert_eq!(error.public().status, StatusIntent::Internal);
            assert_eq!(error.public().code, "internal_error");
        }
    }
}
