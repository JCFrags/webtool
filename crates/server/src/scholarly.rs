//! Separate explicit scholarly routes. Ordinary web search remains unchanged.
use axum::{extract::State, Json};
use utoipa_axum::{router::OpenApiRouter, routes};
use webtool_engine::Engine;
use webtool_protocol::*;
use crate::{contract::HttpErrors, error::ApiJson, ApiResult};

pub(crate) fn routes() -> OpenApiRouter<Engine> {
    OpenApiRouter::new().routes(routes!(search)).routes(routes!(doi)).routes(routes!(arxiv))
}
/// Search one explicit metadata provider, at most 20 results. Abstracts are not full text.
#[utoipa::path(post, path = "/v1/scholarly/search", operation_id = "searchScholarly", tag = "scholarly",
    request_body = ScholarlySearchRequest, responses((status = 200, description = "Source-backed provider metadata, including partial warnings and observation age.", body = ScholarlyResponse), HttpErrors))]
async fn search(State(e): State<Engine>, ApiJson(r): ApiJson<ScholarlySearchRequest>) -> ApiResult<ScholarlyResponse> {
    Ok(Json(e.scholarly_search(r).await?))
}
/// Check the selected Crossref DOI record, not an exhaustive retraction search.
#[utoipa::path(post, path = "/v1/scholarly/doi", operation_id = "inspectScholarlyDoi", tag = "scholarly",
    request_body = ScholarlyDoiRequest, responses((status = 200, description = "Selected metadata and attributed update relations. Provider silence is uncertain.", body = ScholarlyResponse), HttpErrors))]
async fn doi(State(e): State<Engine>, ApiJson(r): ApiJson<ScholarlyDoiRequest>) -> ApiResult<ScholarlyResponse> {
    Ok(Json(e.scholarly_doi(r).await?))
}
/// Inspect one exact arXiv version. Full text needs explicit request and item-specific reuse evidence.
#[utoipa::path(post, path = "/v1/scholarly/arxiv", operation_id = "inspectScholarlyArxiv", tag = "scholarly",
    request_body = ScholarlyArxivRequest, responses((status = 200, description = "Selected metadata, or explicitly permitted full text. Check content_state and full_text_error.", body = ScholarlyResponse), HttpErrors))]
async fn arxiv(State(e): State<Engine>, ApiJson(r): ApiJson<ScholarlyArxivRequest>) -> ApiResult<ScholarlyResponse> {
    Ok(Json(e.scholarly_arxiv(r).await?))
}
