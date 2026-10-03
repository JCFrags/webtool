//! Thin HTTP adapter for the shared public code/documentation operations.
use axum::{extract::State, Json};
use utoipa_axum::{router::OpenApiRouter, routes};
use webtool_engine::Engine;
use webtool_protocol::*;
use crate::{contract::HttpErrors, error::ApiJson, ApiResult};

pub(crate) fn router() -> OpenApiRouter<Engine> {
    OpenApiRouter::new().routes(routes!(discover)).routes(routes!(map))
        .routes(routes!(search)).routes(routes!(file)).routes(routes!(documentation))
}

/// One unauthenticated GitHub repository-search page. Descriptions are not code evidence.
#[utoipa::path(post, path = "/v1/code/discover", operation_id = "discoverRepositories", tag = "code",
    request_body = RepositoryDiscoverRequest, responses((status = 200, description = "Repository discovery and coverage.", body = RepositoryDiscoverResponse), HttpErrors))]
async fn discover(State(e): State<Engine>, ApiJson(r): ApiJson<RepositoryDiscoverRequest>) -> ApiResult<RepositoryDiscoverResponse> {
    Ok(Json(e.code_discover(r).await?))
}

/// Resolve an explicit ref, retain API originals, and save a bounded repository map.
#[utoipa::path(post, path = "/v1/code/map", operation_id = "mapRepository", tag = "code",
    request_body = RepositoryMapRequest, responses((status = 200, description = "Saved revision-pinned map with explicit coverage.", body = RepositoryMapResponse), HttpErrors))]
async fn map(State(e): State<Engine>, ApiJson(r): ApiJson<RepositoryMapRequest>) -> ApiResult<RepositoryMapResponse> {
    Ok(Json(e.code_map(r).await?))
}

/// Explicit path search or literal search of selected admitted files. No fallback or code execution.
#[utoipa::path(post, path = "/v1/code/search", operation_id = "searchRepository", tag = "code",
    request_body = CodeSearchRequest, responses((status = 200, description = "Exact matches or admitted paths. Partial failures remain visible.", body = CodeSearchResponse), HttpErrors))]
async fn search(State(e): State<Engine>, ApiJson(r): ApiJson<CodeSearchRequest>) -> ApiResult<CodeSearchResponse> {
    Ok(Json(e.code_search(r).await?))
}

/// Read one admitted regular UTF-8 file at the saved map's commit/blob identity.
#[utoipa::path(post, path = "/v1/code/file", operation_id = "readRepositoryFile", tag = "code",
    request_body = CodeFileRequest, responses((status = 200, description = "Saved exact file and consumed budget.", body = CodeFileResponse), HttpErrors))]
async fn file(State(e): State<Engine>, ApiJson(r): ApiJson<CodeFileRequest>) -> ApiResult<CodeFileResponse> {
    Ok(Json(e.code_file(r).await?))
}

/// Read one explicit docs.rs release page or source HTML. Latest/ranges are rejected.
#[utoipa::path(post, path = "/v1/docs/read", operation_id = "readVersionedDocumentation", tag = "documentation",
    request_body = DocumentationRequest, responses((status = 200, description = "Release-identified document with retained HTML and provenance.", body = DocumentationResponse), HttpErrors))]
async fn documentation(State(e): State<Engine>, ApiJson(r): ApiJson<DocumentationRequest>) -> ApiResult<DocumentationResponse> {
    Ok(Json(e.documentation(r).await?))
}
