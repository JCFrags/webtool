//! Request-scoped composition of ordinary reads. No separate engine or jobs.
use axum::{extract::{DefaultBodyLimit, State}, Json};
use utoipa_axum::{router::OpenApiRouter, routes};
use webtool_engine::Engine;
use webtool_protocol::*;
use crate::{contract::HttpErrors, error::{ApiError, ApiJson}, ApiResult};

pub(crate) fn router() -> OpenApiRouter<Engine> {
    OpenApiRouter::new().routes(routes!(read_batch))
        .layer(DefaultBodyLimit::max(BATCH_READ_BODY_LIMIT))
}

fn failed(index: usize, error: ApiError) -> BatchReadOutcome {
    let (http_status, problem) = error.into_problem();
    BatchReadOutcome::Error { index, http_status, problem }
}

async fn read_one(engine: &Engine, index: usize, input: BatchReadInput) -> BatchReadOutcome {
    let Some(url) = input.url else {
        return failed(index, ApiError::bad_request("Each input needs an absolute HTTP or HTTPS URL."));
    };
    if url.len() > 8192 || [input.language.as_str(), input.library.as_deref().unwrap_or(""),
        input.selector.as_deref().unwrap_or(""), input.actor.as_deref().unwrap_or("")]
        .iter().any(|value| value.len() > 4096) {
        return failed(index, ApiError::size_limit());
    }
    let renderer = match input.renderer.as_str() {
        "auto" => Renderer::Auto, "http" => Renderer::Http, "captions" => Renderer::Captions,
        "lightpanda" => Renderer::Lightpanda, "chromium" => Renderer::Chromium, "crw" => Renderer::Crw,
        _ => return failed(index, ApiError::bad_request("Use auto, http, captions, lightpanda, chromium, or crw as renderer.")),
    };
    let request = ReadRequest { url, refresh: input.refresh, renderer,
        language: input.language, library: input.library, selector: input.selector, actor: input.actor };
    match engine.read(request).await {
        Ok(read) => BatchReadOutcome::Saved { index, document_id: read.document.id,
            cached: read.cached, warning_count: read.document.warnings.len() },
        Err(error) => failed(index, error.into()),
    }
}

/// Read one to five inputs with at most two ordinary reads active at a time.
/// Every admitted input has an ordered saved reference or safe Problem. A 200
/// response can contain failures. Successful documents remain saved on cancellation.
#[utoipa::path(post, path = "/v1/read/batch", operation_id = "readSourceBatch", tag = "documents",
    request_body = BatchReadRequest,
    responses((status = 200, description = "Ordered per-input references or safe errors. At most 8 KiB, without full documents or input echoes.", body = BatchReadResponse), HttpErrors))]
async fn read_batch(State(engine): State<Engine>, ApiJson(request): ApiJson<BatchReadRequest>) -> ApiResult<BatchReadResponse> {
    if !(1..=BATCH_READ_INPUT_LIMIT).contains(&request.inputs.len()) {
        return Err(ApiError::bad_request("Batch inputs must contain one to five ordinary read requests."));
    }
    let mut inputs = request.inputs.into_iter().enumerate();
    let mut results = Vec::new();
    while let Some((index, input)) = inputs.next() {
        if let Some((next, other)) = inputs.next() {
            // Both futures use the engine's existing admission and source locks.
            // Dropping this request drops their waits, not already saved documents.
            let (first, second) = tokio::join!(read_one(&engine, index, input), read_one(&engine, next, other));
            results.extend([first, second]);
        } else {
            results.push(read_one(&engine, index, input).await);
        }
    }
    let response = BatchReadResponse { results };
    let bytes = serde_json::to_vec(&response).map_err(anyhow::Error::from)?;
    if bytes.len() > BATCH_READ_RESPONSE_LIMIT {
        return Err(ApiError::new(axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error", "The server could not form the bounded batch response. Saved documents remain available."));
    }
    Ok(Json(response))
}
