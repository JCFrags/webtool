use axum::{Router,Json,extract::{DefaultBodyLimit,Multipart,Path,Query,State},http::{StatusCode,header},response::{IntoResponse,Response},routing::{get,post}};
use serde::Deserialize;
use serde_json::{json,Value};
use webtool_engine::Engine;
use webtool_protocol::*;

pub fn router(engine:Engine)->Router{
    let max=engine.config.max_bytes;
    Router::new()
        .route("/health",get(health))
        .route("/v1/health",get(health))
        .route("/v1/read",post(read))
        .route("/v1/search",post(search))
        .route("/v1/ingest",post(ingest))
        .route("/v1/documents",get(documents))
        .route("/v1/documents/{id}",get(document))
        .route("/v1/documents/{id}/original",get(original))
        .route("/v1/documents/{id}/find",post(find))
        .route("/v1/documents/{id}/extract",post(extract))
        .route("/v1/documents/{id}/annotations",get(annotations).post(annotate))
        .route("/v1/libraries",get(libraries).post(create_library))
        .route("/v1/libraries/{name}/items",get(library_items).post(add))
        .route("/v1/crawl",post(crawl))
        .route("/v1/jobs",get(jobs))
        .route("/v1/jobs/{id}",get(job))
        .route("/v1/jobs/{id}/cancel",post(cancel))
        .route("/v1/map",post(map))
        .route("/v1/media",post(media))
        .route("/v1/cite",post(cite))
        .fallback(||async{(StatusCode::NOT_FOUND,Json(Problem{code:"not_found".into(),message:"API route not found".into()}))})
        .layer(DefaultBodyLimit::max(max+256*1024))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(engine)
}
struct ApiError(anyhow::Error);
impl From<anyhow::Error> for ApiError{fn from(e:anyhow::Error)->Self{Self(e)}}
impl IntoResponse for ApiError{
    fn into_response(self)->Response{
        let message=format!("{:#}",self.0);
        let (status,code)=if message.contains("media_helper_missing") {(StatusCode::UNPROCESSABLE_ENTITY,"media_helper_missing")}
            else if message.contains("media_source_blocked") {(StatusCode::BAD_GATEWAY,"media_source_blocked")}
            else if message.contains("media_captions_unavailable") {(StatusCode::UNPROCESSABLE_ENTITY,"media_captions_unavailable")}
            else if message.contains("media_captions_malformed") {(StatusCode::UNPROCESSABLE_ENTITY,"media_captions_malformed")}
            else if message.contains("media_helper_timeout") {(StatusCode::GATEWAY_TIMEOUT,"media_helper_timeout")}
            else if message.contains("media_helper_failed") || message.contains("media_track_download_failed") {(StatusCode::BAD_GATEWAY,"media_helper_failed")}
            else if message.contains("not found"){(StatusCode::NOT_FOUND,"not_found")}
            else if message.contains("exceeds")||message.contains("exceeded")&&message.contains("bytes"){(StatusCode::PAYLOAD_TOO_LARGE,"size_limit")}
            else if message.contains("timeout")||message.contains("deadline"){(StatusCode::GATEWAY_TIMEOUT,"timeout")}
            else if message.contains("source returned HTTP ")||message.contains("fetch source"){(StatusCode::BAD_GATEWAY,"upstream_error")}
            else if message.contains("requires")||message.contains("not configured")||message.contains("unsupported"){(StatusCode::UNPROCESSABLE_ENTITY,"capability_unavailable")}
            else{(StatusCode::BAD_REQUEST,"operation_failed")};
        (status,Json(Problem{code:code.into(),message})).into_response()
    }
}
type ApiResult<T>=Result<Json<T>,ApiError>;
async fn health(State(e):State<Engine>)->Json<Health>{Json(e.health().await)}
async fn read(State(e):State<Engine>,Json(r):Json<ReadRequest>)->ApiResult<ReadResponse>{Ok(Json(e.read(r).await?))}
async fn search(State(e):State<Engine>,Json(r):Json<SearchRequest>)->ApiResult<SearchResponse>{Ok(Json(e.search(r).await?))}
async fn document(State(e):State<Engine>,Path(id):Path<String>)->ApiResult<Document>{Ok(Json(e.store.document(&id).await?))}
#[derive(Deserialize)]struct ListQuery{#[serde(default="default_limit")]limit:usize}
fn default_limit()->usize{100}
async fn documents(State(e):State<Engine>,Query(q):Query<ListQuery>)->ApiResult<Vec<DocumentSummary>>{Ok(Json(e.store.list_documents(None,q.limit).await?))}
async fn original(State(e):State<Engine>,Path(id):Path<String>)->Result<Response,ApiError>{
    let d=e.store.document(&id).await?;let b=e.store.bytes(&d.source.original).await?;
    // Download rather than execute retained HTML in an origin with API access.
    Ok(([(header::CONTENT_TYPE,"application/octet-stream"),(header::CONTENT_DISPOSITION,"attachment")],b).into_response())
}
async fn find(State(e):State<Engine>,Path(id):Path<String>,Json(r):Json<FindRequest>)->ApiResult<FindResponse>{Ok(Json(e.find(&id,r).await?))}
async fn extract(State(e):State<Engine>,Path(id):Path<String>,Json(r):Json<ExtractRequest>)->ApiResult<ExtractResponse>{Ok(Json(e.extract(&id,r).await?))}
async fn libraries(State(e):State<Engine>)->ApiResult<Vec<Library>>{Ok(Json(e.store.libraries().await?))}
async fn create_library(State(e):State<Engine>,Json(r):Json<LibraryCreate>)->ApiResult<Library>{Ok(Json(e.store.create_library(r).await?))}
async fn library_items(State(e):State<Engine>,Path(name):Path<String>,Query(q):Query<ListQuery>)->ApiResult<Vec<DocumentSummary>>{
    e.store.require_library(&name).await?;Ok(Json(e.store.list_documents(Some(name),q.limit).await?))
}
async fn add(State(e):State<Engine>,Path(name):Path<String>,Json(r):Json<LibraryAdd>)->ApiResult<Value>{
    e.store.require_library(&name).await?;e.store.document(&r.document_id).await?;
    e.store.add(&name,&r.document_id,r.actor).await?;Ok(Json(json!({"added":true,"library":name,"document_id":r.document_id})))
}
async fn annotations(State(e):State<Engine>,Path(id):Path<String>)->ApiResult<Vec<Annotation>>{e.store.document(&id).await?;Ok(Json(e.store.annotations(&id).await?))}
async fn annotate(State(e):State<Engine>,Path(id):Path<String>,Json(r):Json<AnnotationCreate>)->ApiResult<Annotation>{Ok(Json(e.store.add_annotation(&id,r).await?))}
async fn crawl(State(e):State<Engine>,Json(r):Json<CrawlRequest>)->ApiResult<Job>{Ok(Json(e.submit(r).await?))}
async fn jobs(State(e):State<Engine>)->ApiResult<Vec<Job>>{Ok(Json(e.store.jobs().await?))}
async fn job(State(e):State<Engine>,Path(id):Path<String>)->ApiResult<Job>{Ok(Json(e.store.job(&id).await?))}
async fn cancel(State(e):State<Engine>,Path(id):Path<String>)->ApiResult<Value>{Ok(Json(e.cancel(&id).await?))}
#[derive(Deserialize)]struct MapRequest{url:String,#[serde(default="default_limit")]limit:usize}
async fn map(State(e):State<Engine>,Json(r):Json<MapRequest>)->ApiResult<Value>{Ok(Json(e.map(&r.url,r.limit).await?))}
#[derive(Deserialize)]struct MediaRequest{url:String,#[serde(default="english")]language:String,library:Option<String>}
fn english()->String{"en".into()}
async fn media(State(e):State<Engine>,Json(r):Json<MediaRequest>)->ApiResult<Document>{Ok(Json(e.media(r.url,r.language,r.library).await?))}
#[derive(Deserialize)]struct CiteRequest{doi:String,format:String}
async fn cite(State(e):State<Engine>,Json(r):Json<CiteRequest>)->ApiResult<Value>{Ok(Json(e.citation(&r.doi,&r.format).await?))}
async fn ingest(State(e):State<Engine>,mut multipart:Multipart)->ApiResult<Document>{
    let mut data=None;let mut name=None;let mut library=None;let mut actor=None;let mut selector=None;
    while let Some(mut field)=multipart.next_field().await.map_err(|e|ApiError(anyhow::anyhow!(e.to_string())))?{
        let key=field.name().unwrap_or("").to_string();
        if key=="file"{
            if data.is_some(){return Err(anyhow::anyhow!("send exactly one file per request").into());}
            let filename=field.file_name().unwrap_or("upload.txt").to_string();
            let mut bytes=Vec::new();
            while let Some(chunk)=field.chunk().await.map_err(|e|ApiError(anyhow::anyhow!(e.to_string())))?{
                if bytes.len().saturating_add(chunk.len())>e.config.max_bytes{return Err(anyhow::anyhow!("upload exceeds configured size limit").into());}
                bytes.extend_from_slice(&chunk);
            }
            data=Some(bytes);if name.is_none(){name=Some(filename);}
        }else{
            let value=field.text().await.map_err(|e|ApiError(anyhow::anyhow!(e.to_string())))?;
            if value.len()>8192{return Err(anyhow::anyhow!("upload metadata exceeds 8192 bytes").into());}
            match key.as_str(){"name"=>name=Some(value),"library"=>library=Some(value),"actor"=>actor=Some(value),"selector"=>selector=Some(value),_=>return Err(anyhow::anyhow!("unknown multipart field: {key}").into())}
        }
    }
    let bytes=data.ok_or_else(||ApiError(anyhow::anyhow!("file field is required")))?;
    Ok(Json(e.ingest(bytes,name.unwrap_or_else(||"upload.txt".into()),library,actor,selector).await?))
}
