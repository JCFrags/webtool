use axum::{body::Body, extract::State, http::header, response::{IntoResponse,Response}, Json};
use utoipa_axum::{router::OpenApiRouter,routes};
use webtool_engine::Engine;
use webtool_protocol::*;
use crate::{ApiResult,error::{ApiError,ApiJson,ApiPath},contract::{HttpErrors,OriginalBytes}};

pub fn router() -> OpenApiRouter<Engine> {
    OpenApiRouter::new().routes(routes!(formats)).routes(routes!(download)).routes(routes!(artifact))
}
/// Observe stable format identities. No media transfer or permission guarantee.
#[utoipa::path(post,path="/v1/video/formats",operation_id="previewMediaFormats",tag="video",request_body=MediaFormatsRequest,
    responses((status=200,description="Bounded format observation, not a future availability or rights guarantee.",body=MediaFormatsResponse),HttpErrors))]
async fn formats(State(e):State<Engine>,ApiJson(r):ApiJson<MediaFormatsRequest>)->ApiResult<MediaFormatsResponse> {Ok(Json(e.media_formats(r).await?))}
/// Submit one explicit constrained video/native-audio job. Disabled without operator allocation.
#[utoipa::path(post,path="/v1/video/download",operation_id="submitMediaDownload",tag="jobs",request_body=MediaDownloadRequest,
    responses((status=200,description="Common queued job. Completion requires validated artifacts.",body=Job),HttpErrors))]
async fn download(State(e):State<Engine>,ApiJson(r):ApiJson<MediaDownloadRequest>)->ApiResult<Job> {Ok(Json(e.submit_media(r).await?))}
/// Stream one retained attachment scoped to this job's accepted artifact list.
#[utoipa::path(get,path="/v1/jobs/{id}/artifacts/{artifact}",operation_id="exportMediaArtifact",tag="jobs",
    params(("id"=String,Path),("artifact"=String,Path)),responses((status=200,description="Exact retained bytes, not a server path.",body=inline(OriginalBytes),content_type="application/octet-stream"),HttpErrors))]
async fn artifact(State(e):State<Engine>,ApiPath((id,artifact)):ApiPath<(String,String)>)->Result<Response,ApiError> {
    let a=e.media_artifact(&id,&artifact).await?;
    let file=e.store.open_artifact(&a.artifact).await?;
    Ok(([(header::CONTENT_TYPE,"application/octet-stream"),(header::CONTENT_DISPOSITION,"attachment"),
        (header::CONTENT_LENGTH,&a.artifact.size.to_string())],Body::from_stream(tokio_util::io::ReaderStream::new(file))).into_response())
}
