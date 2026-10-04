//! HTTP failures use the engine's typed intent and fixed public messages.
//! Request text, cause chains, paths, URLs, and helper stderr never select a response.
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

impl From<webtool_engine::error::EngineError> for ApiError {
    fn from(error: webtool_engine::error::EngineError) -> Self {
        let public = error.public();
        // EngineError cannot contain a caller-selected code/message/status.
        Self::new(StatusCode::from_u16(public.status.http_status()).expect("typed engine status"),
            public.code, public.message)
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        let error = webtool_engine::error::EngineError::from_anyhow(error);
        if matches!(error.category(), webtool_engine::error::ErrorCategory::Internal | webtool_engine::error::ErrorCategory::Storage) {
            // Cause text can contain signed URLs, private paths, or helper output.
            tracing::error!("Internal engine error at the HTTP boundary");
        }
        error.into()
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
    fn typed_engine_errors_preserve_codes_without_reflecting_details() {
        use webtool_engine::error::ErrorKind;
        for (kind, status, code) in [
            (ErrorKind::BrowserTimeout, StatusCode::GATEWAY_TIMEOUT, "browser_timeout"),
            (ErrorKind::MissingResource, StatusCode::NOT_FOUND, "not_found"),
            (ErrorKind::HtmlInputSizeLimit, StatusCode::PAYLOAD_TOO_LARGE, "size_limit"),
            (ErrorKind::InvalidUrl, StatusCode::BAD_REQUEST, "invalid_request"),
            (ErrorKind::CrawlNotResumable, StatusCode::CONFLICT, "crawl_not_resumable"),
        ] {
            let error = ApiError::from(anyhow::Error::from(kind.context("PRIVATE_MARKER /private/path token"))
                .context("outer diagnostic"));
            assert_eq!(error.status, status);
            assert_eq!(error.code, code);
            assert!(!error.message.contains("PRIVATE_MARKER"));
            let (batch_status, problem) = error.into_problem();
            assert_eq!(batch_status, status.as_u16());
            assert_eq!(problem.code, code);
            assert!(!problem.message.contains("PRIVATE_MARKER"));
        }
    }

    #[test]
    fn unknown_internal_errors_are_not_client_errors() {
        for message in ["database file not found", "transaction requires repair", "internal timeout bookkeeping", "unknown fault",
            "document not found", "browser_timeout: PRIVATE_MARKER", "invalid URL", "source returned HTTP 404"] {
            let error = ApiError::from(anyhow::anyhow!(message));
            assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(error.code, "internal_error");
            assert!(error.message.len() < 128);
        }
    }
}
