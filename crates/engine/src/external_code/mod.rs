//! Optional external indexes. No default endpoint, LLM, redirects, retries, or file fallback.
mod config;
mod context7;
mod sourcegraph;
mod transport;
pub use config::{ExternalCodeConfig, ExternalProviderConfig};
pub(crate) use transport::ExternalCodeService;
use std::{future::Future, time::Duration};
use anyhow::Result;
use tokio::time::Instant;
use webtool_protocol::*;
use crate::Engine;

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ExternalCodeError {
    #[error("Configure the selected provider endpoint and required server environment credential reference.")] Unconfigured,
    #[error("Use explicit index selections, supported filters, and bounded options.")] Invalid,
    #[error("This index operation, filter, source type, or verification is unsupported. No fallback was used.")] Unsupported,
    #[error("The provider or selected library/version is unavailable. No substitute was selected.")] Unavailable,
    #[error("The provider denied access or required a paid plan. No retry or purchase was made.")] Denied,
    #[error("The provider or shared local request budget is in cooldown. No retry was made.")] RateLimited,
    #[error("The external provider redirected the request. The redirect was not followed.")] Redirect,
    #[error("The provider identity or retained source does not match the selected index result.")] Identity,
    #[error("The external index response reached its byte or result limit.")] Limit,
    #[error("The external index deadline was reached, including queueing and pacing.")] Timeout,
    #[error("The external provider request or response failed. Provider details are withheld.")] Upstream,
    #[error("The provider returned credential material. It was not saved or returned.")] Privacy,
}
impl ExternalCodeError {
    pub fn status(self) -> u16 { match self {
        Self::Invalid => 400, Self::Unsupported | Self::Unconfigured => 422, Self::Unavailable => 404,
        Self::RateLimited => 429, Self::Limit => 413, Self::Timeout => 504, _ => 502,
    } }
    pub fn code(self) -> &'static str { match self {
        Self::Unconfigured => "external_provider_unconfigured", Self::Invalid => "external_invalid_request",
        Self::Unsupported => "external_unsupported", Self::Unavailable => "external_unavailable",
        Self::Denied => "external_access_denied", Self::RateLimited => "external_rate_limited",
        Self::Redirect => "external_redirect_refused", Self::Identity => "external_identity_mismatch",
        Self::Limit => "external_limit", Self::Timeout => "external_timeout",
        Self::Upstream => "external_upstream_error", Self::Privacy => "external_credential_response_refused",
    } }
    pub(super) fn problem(self) -> Problem { Problem { code: self.code().into(), message: self.to_string() } }
}
impl Engine {
    pub async fn external_code_status(&self) -> ExternalProvidersResponse { self.external_code.status().await }
    async fn external_run<T, F, Fut>(&self, operation: F) -> Result<T>
    where F: FnOnce(Instant) -> Fut, Fut: Future<Output = Result<T>> {
        let budget = self.config.external_code.timeout_ms.min(self.config.request_timeout_seconds.saturating_mul(1000));
        let deadline = Instant::now() + Duration::from_millis(budget);
        tokio::time::timeout_at(deadline, async {
            let _operation = self.operation_slots.acquire().await?;
            operation(deadline).await
        }).await.map_err(|_| ExternalCodeError::Timeout)?
    }
    pub async fn sourcegraph_search(&self, request: SourcegraphSearchRequest) -> Result<SourcegraphSearchResponse> {
        self.external_run(|deadline| sourcegraph::search(self, request, deadline)).await
    }
    pub async fn sourcegraph_verify(&self, request: SourcegraphVerifyRequest) -> Result<SourcegraphVerifyResponse> {
        self.external_run(|_| sourcegraph::verify(self, request)).await
    }
    pub async fn context7_libraries(&self, request: Context7LibrariesRequest) -> Result<Context7LibrariesResponse> {
        self.external_run(|deadline| context7::libraries(self, request, deadline)).await
    }
    pub async fn context7_context(&self, request: Context7ContextRequest) -> Result<Context7ContextResponse> {
        self.external_run(|deadline| context7::context(self, request, deadline)).await
    }
}

pub(super) fn query(value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) { return Err(ExternalCodeError::Invalid.into()); }
    Ok(())
}
pub(super) fn document_id(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(ExternalCodeError::Invalid.into()); }
    Ok(())
}
