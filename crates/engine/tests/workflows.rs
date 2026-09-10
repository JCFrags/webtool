//! Local end-to-end engine tests. They never depend on a public website.
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use webtool_engine::{config::Config, Engine};
use webtool_protocol::*;

async fn engine() -> (tempfile::TempDir, Engine) {
    let directory = tempfile::tempdir().unwrap();
    let config = Config { data_dir: directory.path().into(), ..Default::default() };
    let engine = Engine::new(config).await.unwrap();
    (directory, engine)
}
fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures").join(name);
    std::fs::read(path).unwrap()
}
async fn mock_http() -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let counter = counter.clone();
            tokio::spawn(async move {
                let mut input = [0u8; 8192];
                let mut n = 0;
                while n < input.len() && !input[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                    let received = socket.read(&mut input[n..]).await.unwrap_or(0);
                    if received == 0 { break; }
                    n += received;
                }
                let request = String::from_utf8_lossy(&input[..n]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let (status, mime, body) = match path {
                    "/robots.txt" => ("200 OK", "text/plain", b"User-agent: *\nAllow: /\n".to_vec()),
                    "/missing" => ("404 Not Found", "text/plain", b"Not found".to_vec()),
                    "/large" => ("200 OK", "text/plain", vec![b'x'; 4096]),
                    _ => {
                        counter.fetch_add(1, Ordering::SeqCst);
                        ("200 OK", "text/html", b"<!doctype html><html><head><title>Local source</title></head><body><main><h1>Fixture</h1><p>Exact evidence: 0 and 17.</p><pre>  let n = 0;\n</pre><a href='/second'>Second page</a></main></body></html>".to_vec())
                    }
                };
                let head = format!("HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });
    (url, hits, task)
}
fn request(url: String) -> ReadRequest {
    ReadRequest { url, refresh: false, renderer: Renderer::Http,
        library: None, selector: Some("main".into()), actor: None }
}

#[tokio::test]
async fn one_document_can_belong_to_multiple_shared_libraries() {
    let (_directory, e) = engine().await;
    for name in ["alice", "bob"] {
        e.store.create_library(LibraryCreate { name: name.into(), description: String::new() }).await.unwrap();
    }
    let d = e.ingest(b"source evidence".to_vec(), "source.txt".into(), Some("alice".into()), Some("Alice".into()), None).await.unwrap();
    e.store.add("bob", &d.id, Some("Bob".into())).await.unwrap();
    e.store.add("bob", &d.id, Some("Bob".into())).await.unwrap();
    assert_eq!(e.store.list_documents(None, 10).await.unwrap().len(), 1);
    let libraries = e.store.libraries().await.unwrap();
    assert!(libraries.iter().all(|l| l.items == 1));
}

#[tokio::test]
async fn originals_survive_ingestion_byte_for_byte() {
    let (_directory, e) = engine().await;
    let bytes = b"a\r\n  b\t\n".to_vec();
    let d = e.ingest(bytes.clone(), "sample.txt".into(), None, None, None).await.unwrap();
    assert_eq!(e.store.bytes(&d.source.original).await.unwrap(), bytes);
}

#[tokio::test]
async fn literal_find_reports_utf8_byte_offsets() {
    let (_directory, e) = engine().await;
    let d = e.ingest("é漢字 🦀 17".as_bytes().to_vec(), "utf8.txt".into(), None, None, None).await.unwrap();
    let found = e.find(&d.id, FindRequest { query:"🦀".into(), regex:false, ignore_case:false, limit:10 }).await.unwrap();
    let m = &found.matches[0];
    assert_eq!(&m.text[m.ranges[0][0]..m.ranges[0][1]], "🦀");
}

#[tokio::test]
async fn missing_json_pointer_differs_from_a_null_value() {
    let (_directory, e) = engine().await;
    let d = e.ingest(br#"{"present":null,"zero":0}"#.to_vec(), "sample.json".into(), None, None, None).await.unwrap();
    for (pointer, present) in [("/present",true),("/missing",false)] {
        let r = e.extract(&d.id, ExtractRequest { kind:ExtractKind::JsonPointer, expression:Some(pointer.into()) }).await.unwrap();
        assert_eq!(r.data["found"], present);
    }
}

#[tokio::test]
async fn csv_table_export_does_not_infer_headers_or_drop_empty_fields() {
    let (_directory, e) = engine().await;
    let d = e.ingest(fixture("values.csv"), "values.csv".into(), None, None, None).await.unwrap();
    let r = e.extract(&d.id, ExtractRequest { kind:ExtractKind::Tables, expression:None }).await.unwrap();
    assert_eq!(r.data[0]["content"]["rows"][1][0]["text"], "0");
    assert_eq!(r.data[0]["content"]["rows"][1][1]["text"], "");
    assert_eq!(r.data[0]["content"]["rows"][0][0]["header"], false);
}

#[tokio::test]
async fn repeat_reads_use_saved_results_and_refresh_retrieves_again() {
    let (_directory, e) = engine().await;
    let (base, hits, task) = mock_http().await;
    let r = request(format!("{base}/page.html"));
    let first = e.read(r.clone()).await.unwrap();
    let second = e.read(r.clone()).await.unwrap();
    assert!(!first.cached); assert!(second.cached);
    assert_eq!(first.document.id, second.document.id);
    let mut refreshed = r; refreshed.refresh = true;
    e.read(refreshed).await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    task.abort();
}

#[tokio::test]
async fn simultaneous_reads_coalesce_one_source_fetch() {
    let (_directory, e) = engine().await;
    let (base, hits, task) = mock_http().await;
    let r = request(format!("{base}/page.html"));
    let (a,b,c) = tokio::join!(e.read(r.clone()),e.read(r.clone()),e.read(r));
    assert_eq!(a.unwrap().document.id, b.unwrap().document.id);
    assert!(c.unwrap().cached);
    assert_eq!(hits.load(Ordering::SeqCst),1);
    task.abort();
}

#[tokio::test]
async fn http_error_pages_are_not_saved_as_successful_documents() {
    let (_directory, e) = engine().await;
    let (base, _, task) = mock_http().await;
    let error = e.read(request(format!("{base}/missing"))).await.unwrap_err();
    assert!(format!("{error:#}").contains("404"));
    assert!(e.store.list_documents(None,10).await.unwrap().is_empty());
    task.abort();
}

#[tokio::test]
async fn response_size_limit_is_enforced() {
    let (base, _, task) = mock_http().await;
    let client = reqwest::Client::new();
    assert!(webtool_engine::fetch::http(&client,&format!("{base}/large"),1024).await.is_err());
    task.abort();
}

#[tokio::test]
async fn library_search_uses_the_retained_text() {
    let (_directory, e) = engine().await;
    e.ingest(b"uniquequasar evidence".to_vec(), "note.txt".into(), None, None, None).await.unwrap();
    let r = e.search(SearchRequest { query:"uniquequasar".into(), limit:10, library:Some("*".into()) }).await.unwrap();
    assert_eq!(r.results.len(),1);
    assert!(r.results[0].document_id.is_some());
}

#[tokio::test]
async fn failed_upload_does_not_create_a_document() {
    let (_directory, e) = engine().await;
    assert!(e.ingest(b"not JSON".to_vec(), "bad.json".into(), None, None, None).await.is_err());
    assert!(e.store.list_documents(None,10).await.unwrap().is_empty());
}

#[tokio::test]
async fn running_jobs_become_interrupted_on_restart() {
    let (_directory, e) = engine().await;
    let job = Job { id:"restart-test".into(), state:JobState::Running,
        request:CrawlRequest { url:"https://example.invalid".into(),max_pages:1,max_depth:0,library:None,actor:None },
        created_at:"2026-01-01T00:00:00Z".into(),updated_at:"2026-01-01T00:00:00Z".into(),
        document_ids:vec![],visited:0,warnings:vec![],error:None };
    e.store.put_job(job).await.unwrap();
    e.recover_jobs().await.unwrap();
    assert_eq!(e.store.job("restart-test").await.unwrap().state, JobState::Interrupted);
}

#[tokio::test]
async fn fixture_readers_preserve_representative_content() {
    let (_directory, e) = engine().await;
    for (name, marker) in [("source.md","Exact code"),("captions.vtt","Spoken evidence"),("feed.atom","Research entry"),("notebook.ipynb","print(0)")] {
        let d = e.ingest(fixture(name),name.into(),None,None,None).await.unwrap();
        assert!(d.text().contains(marker), "{name} lost expected content");
    }
}
