//! Thin optional-provider HTTP adapters. No account/key input or configuration writes.
use axum::{extract::State, Json};
use utoipa_axum::{router::OpenApiRouter, routes};
use webtool_engine::Engine;
use webtool_protocol::*;
use crate::{contract::HttpErrors, error::ApiJson, ApiResult};

pub(crate) fn router() -> OpenApiRouter<Engine> {
    OpenApiRouter::new().routes(routes!(status)).routes(routes!(sourcegraph))
        .routes(routes!(verify)).routes(routes!(libraries)).routes(routes!(context))
}
#[utoipa::path(get, path = "/v1/external/providers", operation_id = "getExternalCodeProviders", tag = "external indexes",
    responses((status = 200, description = "Local configuration only. No credentials, endpoints or live checks.", body = ExternalProvidersResponse), HttpErrors))]
async fn status(State(e): State<Engine>) -> ApiResult<ExternalProvidersResponse> { Ok(Json(e.external_code_status().await)) }
#[utoipa::path(post, path = "/v1/external/sourcegraph/search", operation_id = "searchSourcegraphIndex", tag = "external indexes",
    request_body = SourcegraphSearchRequest, responses((status = 200, description = "Bounded index snippets, provenance and partial-search signals. Not source evidence.", body = SourcegraphSearchResponse), HttpErrors))]
async fn sourcegraph(State(e): State<Engine>, ApiJson(r): ApiJson<SourcegraphSearchRequest>) -> ApiResult<SourcegraphSearchResponse> {
    Ok(Json(e.sourcegraph_search(r).await?))
}
#[utoipa::path(post, path = "/v1/external/sourcegraph/verify", operation_id = "verifySourcegraphSavedFile", tag = "external indexes",
    request_body = SourcegraphVerifyRequest, responses((status = 200, description = "Compare one saved index hit with an ordinary retained GitHub file and matching map. Zero network requests.", body = SourcegraphVerifyResponse), HttpErrors))]
async fn verify(State(e): State<Engine>, ApiJson(r): ApiJson<SourcegraphVerifyRequest>) -> ApiResult<SourcegraphVerifyResponse> {
    Ok(Json(e.sourcegraph_verify(r).await?))
}
#[utoipa::path(post, path = "/v1/external/context7/libraries", operation_id = "discoverContext7Libraries", tag = "external indexes",
    request_body = Context7LibrariesRequest, responses((status = 200, description = "Bounded library index metadata. fast=true, no automatic selection.", body = Context7LibrariesResponse), HttpErrors))]
async fn libraries(State(e): State<Engine>, ApiJson(r): ApiJson<Context7LibrariesRequest>) -> ApiResult<Context7LibrariesResponse> {
    Ok(Json(e.context7_libraries(r).await?))
}
#[utoipa::path(post, path = "/v1/external/context7/context", operation_id = "readContext7IndexContext", tag = "external indexes",
    request_body = Context7ContextRequest, responses((status = 200, description = "Explicit library/version or tracked-index selection. Third-party snippets, not revision-exact publisher text.", body = Context7ContextResponse), HttpErrors))]
async fn context(State(e): State<Engine>, ApiJson(r): ApiJson<Context7ContextRequest>) -> ApiResult<Context7ContextResponse> {
    Ok(Json(e.context7_context(r).await?))
}
