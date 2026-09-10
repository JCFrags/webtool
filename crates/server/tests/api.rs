//! API tests run in-process with local data. No external providers are contacted.
use axum::{body::{Body, to_bytes}, http::{Request, StatusCode}};
use tower::ServiceExt;
use webtool_engine::{config::Config, Engine};
use serde_json::{json, Value};

async fn app() -> (tempfile::TempDir, axum::Router) {
    let directory = tempfile::tempdir().unwrap();
    let e = Engine::new(Config { data_dir:directory.path().into(),..Default::default() }).await.unwrap();
    (directory,webtool_server::router(e))
}
async fn json_response(app: axum::Router, method: &str, path: &str, value: Value) -> (StatusCode, Value) {
    let request = Request::builder().method(method).uri(path)
        .header("content-type","application/json").body(Body::from(value.to_string())).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024*1024).await.unwrap();
    (status,serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn health_describes_capabilities_without_claiming_live_readiness() {
    let (_directory,app)=app().await;
    let (status,value)=json_response(app,"GET","/v1/health",Value::Null).await;
    assert_eq!(status,StatusCode::OK); assert_eq!(value["api_version"],"v1");
    assert!(value["capabilities"].as_array().unwrap().len()>=5);
}
#[tokio::test]
async fn libraries_are_visible_to_every_client() {
    let (_directory,app)=app().await;
    let (status,_)=json_response(app.clone(),"POST","/v1/libraries",json!({"name":"team","description":"Shared references"})).await;
    assert_eq!(status,StatusCode::OK);
    let (_,value)=json_response(app,"GET","/v1/libraries",Value::Null).await;
    assert_eq!(value[0]["name"],"team");
}
#[tokio::test]
async fn missing_document_returns_structured_error() {
    let (_directory,app)=app().await;
    let (status,value)=json_response(app,"GET","/v1/documents/not-present",Value::Null).await;
    assert_eq!(status,StatusCode::NOT_FOUND); assert_eq!(value["code"],"not_found");
}
#[tokio::test]
async fn multipart_upload_retains_exact_original_bytes() {
    let (_directory,app)=app().await;
    let body="--fixture\r\nContent-Disposition: form-data; name=\"file\"; filename=\"note.txt\"\r\nContent-Type: text/plain\r\n\r\nexact evidence 0\r\n--fixture--\r\n";
    let request=Request::builder().method("POST").uri("/v1/ingest")
        .header("content-type","multipart/form-data; boundary=fixture").body(Body::from(body)).unwrap();
    let response=app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(),StatusCode::OK);
    let bytes=to_bytes(response.into_body(),1024*1024).await.unwrap();
    let d:Value=serde_json::from_slice(&bytes).unwrap();
    let path=format!("/v1/documents/{}/original",d["id"].as_str().unwrap());
    let response=app.oneshot(Request::builder().uri(path).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.headers()["content-disposition"],"attachment");
    assert_eq!(&to_bytes(response.into_body(),1024).await.unwrap()[..],b"exact evidence 0");
}
#[tokio::test]
async fn invalid_scheme_is_not_retrieved() {
    let (_directory,app)=app().await;
    let (status,value)=json_response(app,"POST","/v1/read",json!({"url":"file:///etc/passwd"})).await;
    assert_eq!(status,StatusCode::BAD_REQUEST);
    assert!(value["message"].as_str().unwrap().contains("HTTP"));
}
