use axum::{
    extract::{DefaultBodyLimit, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use webtool_engine::Engine;
use webtool_protocol::*;

mod batch;
mod code;
mod contract;
mod error;
mod external_code;
mod scholarly;
use contract::*;
use error::{ApiError, ApiJson, ApiMultipart, ApiPath, ApiQuery};

type ApiResult<T> = Result<Json<T>, ApiError>;

/// Collect the HTTP routes and their contract together. Additional adapters can
/// mount beside the returned Axum router without another engine or database.
pub fn api_router() -> OpenApiRouter<Engine> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(health))
        .routes(routes!(read))
        .routes(routes!(search))
        .routes(routes!(archive_lookup))
        .routes(routes!(archive_read))
        .routes(routes!(ingest))
        .routes(routes!(documents))
        .routes(routes!(document))
        .routes(routes!(original))
        .routes(routes!(find))
        .routes(routes!(extract))
        .routes(routes!(annotations, annotate))
        .routes(routes!(libraries, create_library))
        .routes(routes!(library_items, add))
        .routes(routes!(crawl))
        .routes(routes!(jobs))
        .routes(routes!(job))
        .routes(routes!(cancel))
        .routes(routes!(resume))
        .routes(routes!(map))
        .routes(routes!(media))
        .routes(routes!(video_search))
        .routes(routes!(caption_tracks))
        .routes(routes!(captions))
        .routes(routes!(cite))
        .merge(batch::router())
        .merge(code::router())
        .merge(external_code::router())
        .merge(scholarly::routes())
}

pub fn router(engine: Engine) -> Router {
    let max = engine.config.max_bytes;
    let (router, mut spec) = api_router().split_for_parts();
    spec.info.title = "webtool HTTP API".into();
    spec.info.description = Some("Shared research service. No authentication. Keep loopback binding unless a separate network boundary is approved. Metadata and extract data can contain arbitrary JSON. /health is an alias for /v1/health.".into());
    router
        .route("/health", get(health))
        .route("/openapi.json", get(move || std::future::ready(Json(spec.clone()))))
        .fallback(|| async { ApiError::new(StatusCode::NOT_FOUND, "not_found", "API route not found.") })
        .method_not_allowed_fallback(|| async { ApiError::new(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed", "Method not allowed for this route.") })
        .layer(DefaultBodyLimit::max(max.saturating_add(256 * 1024)))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(engine)
}

/// Report compiled/configured capabilities, not live provider readiness.
#[utoipa::path(get, path = "/v1/health", operation_id = "getHealth", tag = "service",
    responses((status = 200, description = "Successful operation.", body = Health), HttpErrors))]
async fn health(State(e): State<Engine>) -> Json<Health> {
    let mut health = e.health().await;
    health.build_commit = Some(option_env!("WEBTOOL_BUILD_COMMIT").unwrap_or("unknown").to_owned());
    Json(health)
}

/// Retrieve and save one source, or return its accepted cached snapshot.
#[utoipa::path(post, path = "/v1/read", operation_id = "readSource", tag = "documents",
    request_body = ReadRequest, responses((status = 200, description = "Successful operation.", body = ReadResponse), HttpErrors))]
async fn read(State(e): State<Engine>, ApiJson(r): ApiJson<ReadRequest>) -> ApiResult<ReadResponse> {
    Ok(Json(e.read(r).await?))
}

/// Search public providers or saved documents. Provider snippets are not fetched evidence.
#[utoipa::path(post, path = "/v1/search", operation_id = "search", tag = "search",
    request_body = SearchRequest, responses((status = 200, description = "Successful operation.", body = SearchResponse), HttpErrors))]
async fn search(State(e): State<Engine>, ApiJson(r): ApiJson<SearchRequest>) -> ApiResult<SearchResponse> {
    Ok(Json(e.search(r).await?))
}

/// Inspect up to three historical capture candidates at or before an exact UTC date.
#[utoipa::path(post, path = "/v1/archive/lookup", operation_id = "lookupArchive", tag = "archives",
    request_body = ArchiveLookupRequest, responses((status = 200, description = "Bounded capture index, not page evidence.", body = ArchiveLookupResponse), HttpErrors))]
async fn archive_lookup(State(e): State<Engine>, ApiJson(r): ApiJson<ArchiveLookupRequest>) -> ApiResult<ArchiveLookupResponse> {
    Ok(Json(e.archive_lookup(r).await?))
}

/// Read one explicitly selected capture. No redirect, browser, or live fallback.
#[utoipa::path(post, path = "/v1/archive/read", operation_id = "readArchive", tag = "archives",
    request_body = ArchiveReadRequest, responses((status = 200, description = "Saved historical replay with separate capture and replay status.", body = ReadResponse), HttpErrors))]
async fn archive_read(State(e): State<Engine>, ApiJson(r): ApiJson<ArchiveReadRequest>) -> ApiResult<ReadResponse> {
    Ok(Json(e.archive_read(r).await?))
}

/// Retrieve a saved document without a network read.
#[utoipa::path(get, path = "/v1/documents/{id}", operation_id = "getDocument", tag = "documents",
    params(("id" = String, Path, description = "Saved document ID")),
    responses((status = 200, description = "Successful operation.", body = Document), HttpErrors))]
async fn document(State(e): State<Engine>, ApiPath(id): ApiPath<String>) -> ApiResult<Document> {
    Ok(Json(e.store.document(&id).await?))
}

/// List saved documents, newest first.
#[utoipa::path(get, path = "/v1/documents", operation_id = "listDocuments", tag = "documents",
    params(ListQuery), responses((status = 200, description = "Successful operation.", body = [DocumentSummary]), HttpErrors))]
async fn documents(State(e): State<Engine>, ApiQuery(q): ApiQuery<ListQuery>) -> ApiResult<Vec<DocumentSummary>> {
    Ok(Json(e.store.list_documents(None, q.limit).await?))
}

/// Download retained original bytes as an attachment, never executable HTML.
///
/// Inspect Document.source.original for the artifact role, source media type,
/// size, and SHA-256. A rendered_dom original is a DOM snapshot, not an HTTP response.
#[utoipa::path(get, path = "/v1/documents/{id}/original", operation_id = "downloadOriginal", tag = "documents",
    params(("id" = String, Path, description = "Saved document ID")),
    responses((status = 200, description = "Successful operation.", body = inline(OriginalBytes), content_type = "application/octet-stream",
        headers(("Content-Disposition" = String, description = "attachment; filename=rendered-dom.html for DOM snapshots, otherwise attachment"))), HttpErrors))]
async fn original(State(e): State<Engine>, ApiPath(id): ApiPath<String>) -> Result<Response, ApiError> {
    let d = e.store.document(&id).await?;
    let b = e.store.bytes(&d.source.original).await?;
    let disposition = if d.source.original.role == "rendered_dom" { "attachment; filename=rendered-dom.html" } else { "attachment" };
    Ok(([(header::CONTENT_TYPE, "application/octet-stream"), (header::CONTENT_DISPOSITION, disposition)], b).into_response())
}

/// Find literal or regex matches. Offsets are half-open UTF-8 byte ranges in block text.
#[utoipa::path(post, path = "/v1/documents/{id}/find", operation_id = "findInDocument", tag = "documents",
    params(("id" = String, Path)), request_body = FindRequest,
    responses((status = 200, description = "Successful operation.", body = FindResponse), HttpErrors))]
async fn find(State(e): State<Engine>, ApiPath(id): ApiPath<String>, ApiJson(r): ApiJson<FindRequest>) -> ApiResult<FindResponse> {
    Ok(Json(e.find(&id, r).await?))
}

/// Extract selected structures. The data shape depends on kind.
///
/// A JSON pointer result has pointer, found, and value fields. Missing paths have
/// found=false even though value is null. Metadata and other data are arbitrary JSON.
#[utoipa::path(post, path = "/v1/documents/{id}/extract", operation_id = "extractFromDocument", tag = "documents",
    params(("id" = String, Path)), request_body = ExtractRequest,
    responses((status = 200, description = "Successful operation.", body = ExtractResponse), HttpErrors))]
async fn extract(State(e): State<Engine>, ApiPath(id): ApiPath<String>, ApiJson(r): ApiJson<ExtractRequest>) -> ApiResult<ExtractResponse> {
    Ok(Json(e.extract(&id, r).await?))
}

/// List shared libraries visible to every connected client.
#[utoipa::path(get, path = "/v1/libraries", operation_id = "listLibraries", tag = "libraries",
    responses((status = 200, description = "Successful operation.", body = [Library]), HttpErrors))]
async fn libraries(State(e): State<Engine>) -> ApiResult<Vec<Library>> {
    Ok(Json(e.store.libraries().await?))
}

/// Create a shared library.
#[utoipa::path(post, path = "/v1/libraries", operation_id = "createLibrary", tag = "libraries",
    request_body = LibraryCreate, responses((status = 200, description = "Successful operation.", body = Library), HttpErrors))]
async fn create_library(State(e): State<Engine>, ApiJson(r): ApiJson<LibraryCreate>) -> ApiResult<Library> {
    Ok(Json(e.store.create_library(r).await?))
}

/// List documents attached to a shared library.
#[utoipa::path(get, path = "/v1/libraries/{name}/items", operation_id = "listLibraryItems", tag = "libraries",
    params(("name" = String, Path, description = "Shared library name"), ListQuery),
    responses((status = 200, description = "Successful operation.", body = [DocumentSummary]), HttpErrors))]
async fn library_items(State(e): State<Engine>, ApiPath(name): ApiPath<String>, ApiQuery(q): ApiQuery<ListQuery>) -> ApiResult<Vec<DocumentSummary>> {
    e.store.require_library(&name).await?;
    Ok(Json(e.store.list_documents(Some(name), q.limit).await?))
}

/// Attach an existing saved document. An existing attachment is not duplicated.
#[utoipa::path(post, path = "/v1/libraries/{name}/items", operation_id = "addLibraryItem", tag = "libraries",
    params(("name" = String, Path)), request_body = LibraryAdd,
    responses((status = 200, description = "Successful operation.", body = LibraryAddResponse), HttpErrors))]
async fn add(State(e): State<Engine>, ApiPath(name): ApiPath<String>, ApiJson(r): ApiJson<LibraryAdd>) -> ApiResult<Value> {
    e.store.require_library(&name).await?;
    e.store.document(&r.document_id).await?;
    e.store.add(&name, &r.document_id, r.actor).await?;
    Ok(Json(json!({"added":true,"library":name,"document_id":r.document_id})))
}

/// List document annotations.
#[utoipa::path(get, path = "/v1/documents/{id}/annotations", operation_id = "listAnnotations", tag = "documents",
    params(("id" = String, Path)), responses((status = 200, description = "Successful operation.", body = [Annotation]), HttpErrors))]
async fn annotations(State(e): State<Engine>, ApiPath(id): ApiPath<String>) -> ApiResult<Vec<Annotation>> {
    e.store.document(&id).await?;
    Ok(Json(e.store.annotations(&id).await?))
}

/// Add a note and tags to a saved document.
#[utoipa::path(post, path = "/v1/documents/{id}/annotations", operation_id = "addAnnotation", tag = "documents",
    params(("id" = String, Path)), request_body = AnnotationCreate,
    responses((status = 200, description = "Successful operation.", body = Annotation), HttpErrors))]
async fn annotate(State(e): State<Engine>, ApiPath(id): ApiPath<String>, ApiJson(r): ApiJson<AnnotationCreate>) -> ApiResult<Annotation> {
    e.store.document(&id).await?;
    Ok(Json(e.store.add_annotation(&id, r).await?))
}

/// Submit a bounded HTTP crawl. Poll the returned job ID for progress and partial results.
#[utoipa::path(post, path = "/v1/crawl", operation_id = "submitCrawl", tag = "jobs",
    request_body = CrawlRequest, responses((status = 200, description = "Successful operation.", body = Job), HttpErrors))]
async fn crawl(State(e): State<Engine>, ApiJson(r): ApiJson<CrawlRequest>) -> ApiResult<Job> {
    Ok(Json(e.submit(r).await?))
}

/// List up to 1000 jobs, with active jobs first.
#[utoipa::path(get, path = "/v1/jobs", operation_id = "listJobs", tag = "jobs",
    responses((status = 200, description = "Successful operation.", body = [Job]), HttpErrors))]
async fn jobs(State(e): State<Engine>) -> ApiResult<Vec<Job>> {
    Ok(Json(e.store.jobs().await?))
}

/// Retrieve persistent job state, warnings, and saved document IDs.
#[utoipa::path(get, path = "/v1/jobs/{id}", operation_id = "getJob", tag = "jobs",
    params(("id" = String, Path, description = "Job ID")), responses((status = 200, description = "Successful operation.", body = Job), HttpErrors))]
async fn job(State(e): State<Engine>, ApiPath(id): ApiPath<String>) -> ApiResult<Job> {
    Ok(Json(e.store.job(&id).await?))
}

/// Request job cancellation. Saved documents remain available. Poll for the final state.
#[utoipa::path(post, path = "/v1/jobs/{id}/cancel", operation_id = "cancelJob", tag = "jobs",
    params(("id" = String, Path)), responses((status = 200, description = "Successful operation.", body = CancelResponse), HttpErrors))]
async fn cancel(State(e): State<Engine>, ApiPath(id): ApiPath<String>) -> ApiResult<Value> {
    Ok(Json(e.cancel(&id).await?))
}

/// Explicitly resume a durable interrupted, cancelled, or partial crawl.
/// Past attempt charges remain. Only pending and interrupted entries can run.
#[utoipa::path(post, path = "/v1/jobs/{id}/resume", operation_id = "resumeJob", tag = "jobs",
    params(("id" = String, Path)), request_body = CrawlResumeRequest,
    responses((status = 200, description = "Resumed job queued with its retained frontier and budget.", body = Job), HttpErrors))]
async fn resume(State(e): State<Engine>, ApiPath(id): ApiPath<String>, ApiJson(r): ApiJson<CrawlResumeRequest>) -> ApiResult<Job> {
    Ok(Json(e.resume(&id, r).await?))
}

/// List page links or sitemap locations. Nested sitemap expansion is not performed.
#[utoipa::path(post, path = "/v1/map", operation_id = "mapSource", tag = "discovery",
    request_body = MapRequest, responses((status = 200, description = "Successful operation.", body = MapResponse), HttpErrors))]
async fn map(State(e): State<Engine>, ApiJson(r): ApiJson<MapRequest>) -> ApiResult<Value> {
    Ok(Json(e.map(&r.url, r.limit).await?))
}

/// Retrieve supplied captions as a saved document. This does not transcribe or download media.
#[utoipa::path(post, path = "/v1/media", operation_id = "readCaptions", tag = "discovery",
    request_body = MediaRequest, responses((status = 200, description = "Successful operation.", body = Document), HttpErrors))]
async fn media(State(e): State<Engine>, ApiJson(r): ApiJson<MediaRequest>) -> ApiResult<Document> {
    Ok(Json(e.media(r.url, r.language, r.library).await?))
}

/// Search bounded flat video metadata. Does not fetch each video, captions, or media.
#[utoipa::path(post, path = "/v1/video/search", operation_id = "searchVideos", tag = "video",
    request_body = VideoSearchRequest, responses((status = 200, description = "Provider-ordered metadata, not transcript evidence.", body = VideoSearchResponse), HttpErrors))]
async fn video_search(State(e): State<Engine>, ApiJson(r): ApiJson<VideoSearchRequest>) -> ApiResult<VideoSearchResponse> {
    Ok(Json(e.video_search(r).await?))
}

/// List selectable untranslated VTT tracks for one video, without signed track URLs.
#[utoipa::path(post, path = "/v1/video/tracks", operation_id = "listCaptionTracks", tag = "video",
    request_body = CaptionTracksRequest, responses((status = 200, description = "Supplied track inventory. Empty inventory includes a warning.", body = CaptionTracksResponse), HttpErrors))]
async fn caption_tracks(State(e): State<Engine>, ApiJson(r): ApiJson<CaptionTracksRequest>) -> ApiResult<CaptionTracksResponse> {
    Ok(Json(e.caption_tracks(r).await?))
}

/// Save one exact-language track with explicit origin choice. No substitution, translation, or ASR.
#[utoipa::path(post, path = "/v1/video/captions", operation_id = "readSelectedCaptions", tag = "video",
    request_body = CaptionReadRequest, responses((status = 200, description = "Saved caption document and cache state.", body = ReadResponse), HttpErrors))]
async fn captions(State(e): State<Engine>, ApiJson(r): ApiJson<CaptionReadRequest>) -> ApiResult<ReadResponse> {
    Ok(Json(e.captions(r).await?))
}

/// Retrieve DOI bibliography data or cite a saved arXiv paper without another network read.
#[utoipa::path(post, path = "/v1/cite", operation_id = "citeSource", tag = "discovery",
    request_body = CiteInput, responses((status = 200, description = "Successful operation.", body = CiteResponse), HttpErrors))]
async fn cite(State(e): State<Engine>, ApiJson(r): ApiJson<CiteRequest>) -> ApiResult<Value> {
    Ok(Json(e.citation(&r.doi, &r.format).await?))
}

/// Upload exactly one file and retain its exact bytes before parsing.
///
/// Optional text fields are limited to 8192 bytes each. File bytes are limited to
/// configured max_bytes. The total request limit is max_bytes plus 256 KiB.
#[utoipa::path(post, path = "/v1/ingest", operation_id = "ingestFile", tag = "documents",
    request_body(content = IngestForm, content_type = "multipart/form-data"),
    responses((status = 200, description = "Successful operation.", body = Document), HttpErrors))]
async fn ingest(State(e): State<Engine>, ApiMultipart(mut multipart): ApiMultipart) -> ApiResult<Document> {
    let mut data = None;
    let mut name = None;
    let mut library = None;
    let mut actor = None;
    let mut selector = None;
    while let Some(mut field) = multipart.next_field().await? {
        let key = field.name().unwrap_or("").to_owned();
        if key == "file" {
            if data.is_some() { return Err(ApiError::bad_request("Send exactly one file per request.")); }
            let filename = field.file_name().unwrap_or("upload.txt").to_owned();
            let mut bytes = Vec::new();
            while let Some(chunk) = field.chunk().await? {
                if bytes.len().saturating_add(chunk.len()) > e.config.max_bytes { return Err(ApiError::size_limit()); }
                bytes.extend_from_slice(&chunk);
            }
            data = Some(bytes);
            if name.is_none() { name = Some(filename); }
        } else {
            if !matches!(key.as_str(), "name" | "library" | "actor" | "selector") {
                return Err(ApiError::bad_request("Unknown multipart field. Use file, name, library, actor, or selector."));
            }
            // Keep multipart charset/BOM decoding. The total body limit applies
            // while this field is read, before the decoded metadata limit.
            let value = field.text().await?;
            if value.len() > 8192 {
                return Err(ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "size_limit", "Upload metadata exceeds 8192 bytes."));
            }
            match key.as_str() {
                "name" => name = Some(value), "library" => library = Some(value),
                "actor" => actor = Some(value), "selector" => selector = Some(value), _ => unreachable!(),
            }
        }
    }
    let bytes = data.ok_or_else(|| ApiError::bad_request("The file field is required."))?;
    Ok(Json(e.ingest(bytes, name.unwrap_or_else(|| "upload.txt".into()), library, actor, selector).await?))
}
