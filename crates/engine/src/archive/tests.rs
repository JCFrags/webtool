use super::*;
use std::sync::Mutex as StdMutex;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};

const SOURCE: &str = "https://example.org/history.html";
const FIRST: &str = "20010101120000";
const SECOND: &str = "20010102120000";
const HTML: &[u8] = br#"<!doctype html><html><head><title>Historic policy</title></head><body><article><h1>Historic policy</h1><p>This historical policy applies only when the named conditions hold. The example preserves the exact qualification, not a general promise. The old process used explicit dates, retained original source text, and made failed requests visible. Archived content describes the selected capture, not the current website.</p><pre><code>fn old_policy(enabled: bool) -&gt; bool { enabled }</code></pre><p>Do not infer publication dates or the state of linked pages from this capture. The supplied source has an independent appendix and a dated policy change.</p><a href="/appendix">Appendix</a></article></body></html>"#;

struct Local {
    url: String,
    requests: Arc<StdMutex<Vec<(String, Instant)>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Local { fn drop(&mut self) { self.task.abort(); } }
async fn local() -> Local {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let redirect = format!("{url}/live-target");
    let requests = Arc::new(StdMutex::new(Vec::new()));
    let trace = requests.clone();
    let task = tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                if n == 0 { break; }
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") { break; }
                assert!(bytes.len() <= 8192);
            }
            let text = std::str::from_utf8(&bytes).unwrap();
            let target = text.split_whitespace().nth(1).unwrap().to_string();
            trace.lock().unwrap().push((target.clone(), Instant::now()));
            let parsed = Url::parse(&format!("http://localhost{target}")).unwrap();
            let (status, headers, body) = if parsed.path() == "/cdx/search/cdx" {
                let params: std::collections::HashMap<_, _> = parsed.query_pairs().collect();
                assert_eq!(params["matchType"], "exact"); assert_eq!(params["url"], SOURCE);
                assert_eq!(params["fl"], "timestamp,original,statuscode,mimetype,digest");
                assert!(matches!(params["limit"].as_ref(), "-2" | "-4"));
                let mut rows = vec![json!(["timestamp","original","statuscode","mimetype","digest"])];
                for date in [FIRST, SECOND] {
                    if date >= params["from"].as_ref() && date <= params["to"].as_ref() {
                        rows.push(json!([date,SOURCE,if date == SECOND { "-" } else { "200" },"text/html","PROVIDER-DIGEST"]));
                    }
                }
                ("200 OK", "Content-Type: application/json\r\n".to_string(), serde_json::to_vec(&rows).unwrap())
            } else if parsed.path().starts_with("/web/") {
                let time = if parsed.path().contains(FIRST) { "Mon, 01 Jan 2001 12:00:00 GMT" } else { "Tue, 02 Jan 2001 12:00:00 GMT" };
                ("200 OK", format!("Content-Type: text/html; charset=utf-8\r\nMemento-Datetime: {time}\r\nLink: <https://web.archive.org/>; rel=\"timegate\", <{SOURCE}>; rel=\"original\"\r\n"), HTML.to_vec())
            } else if parsed.path() == "/redirect" {
                ("302 Found", format!("Location: {redirect}\r\n"), Vec::new())
            } else if parsed.path() == "/limit" {
                ("429 Too Many Requests", "Retry-After: 120\r\n".to_string(), Vec::new())
            } else { panic!("unexpected request, including live fallback"); };
            let response = format!("HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        }
    });
    Local { url, requests, task }
}
fn read_request(date: &str) -> ArchiveReadRequest {
    ArchiveReadRequest { url: SOURCE.into(), timestamp: date.into(), refresh: false, library: Some("history".into()), actor: None }
}

#[tokio::test]
async fn retained_capture_identity_cache_and_provider_boundary() {
    let fixture = local().await;
    let root = tempfile::tempdir().unwrap();
    let mut config = Config::default(); config.data_dir = root.path().into();
    let mut engine = Engine::new(config).await.unwrap(); engine.archive.endpoint = Some(fixture.url.clone());
    engine.store.create_library(LibraryCreate { name: "history".into(), description: "diagnostic".into() }).await.unwrap();
    let lookup_request = ArchiveLookupRequest { url: SOURCE.into(), at: SECOND.into(), within_days: 2, limit: 3, refresh: false };
    let lookup = engine.archive_lookup(lookup_request.clone()).await.unwrap();
    assert_eq!(lookup.captures.iter().map(|c| c.timestamp.as_str()).collect::<Vec<_>>(), [SECOND, FIRST]);
    assert!(!lookup.has_more); assert!(!lookup.cached);
    let index = engine.store.bytes(&lookup.index_artifact).await.unwrap();
    assert!(std::str::from_utf8(&index).unwrap().contains(FIRST));
    let clone = engine.clone();
    let cached = clone.archive_lookup(lookup_request).await.unwrap();
    assert!(cached.cached); assert_eq!(cached.retrieved_at, lookup.retrieved_at);
    assert_eq!(fixture.requests.lock().unwrap().len(), 1);
    let first = engine.archive_read(read_request(FIRST)).await.unwrap();
    assert!(!first.cached);
    assert_eq!(engine.store.bytes(&first.document.source.original).await.unwrap(), HTML);
    assert_eq!(first.document.source.original.role, "wayback_replay");
    assert_eq!(first.document.metadata["archive"]["capture"]["capture_status"], 200);
    assert_eq!(first.document.metadata["archive"]["replay_status"], 200);
    assert_eq!(first.document.metadata["archive"]["actual_capture"], FIRST);
    assert!(first.document.text().contains("only when the named conditions hold"));
    assert!(first.document.text().contains("fn old_policy(enabled: bool) -> bool { enabled }"));
    assert!(first.document.links.iter().any(|l| l.url == "https://example.org/appendix"));
    assert!(first.document.warnings.iter().any(|w| w.code == "historical_capture"));
    let again = clone.archive_read(read_request(FIRST)).await.unwrap();
    assert!(again.cached); assert_eq!(again.document.id, first.document.id);
    let second = clone.archive_read(read_request(SECOND)).await.unwrap();
    assert_eq!(first.document.source.original.sha256, second.document.source.original.sha256);
    assert_ne!(first.document.id, second.document.id, "identical bytes at different captures retain different historical identity");
    assert!(second.document.metadata["archive"]["capture"]["capture_status"].is_null());
    assert_eq!(second.document.metadata["archive"]["replay_status"], 200);
    assert!(second.document.warnings.iter().any(|w| w.code == "archive_capture_status_unknown"));
    assert_eq!(engine.store.list_documents(Some("history".into()), 10).await.unwrap().len(), 2);
    assert_eq!(engine.find(&first.document.id, FindRequest { query: "named conditions".into(), regex: false, ignore_case: false, limit: 5 }).await.unwrap().matches.len(), 1);
    let redirect = engine.archive.get(Url::parse("https://web.archive.org/redirect").unwrap(), 1024, false, false).await.err().unwrap();
    assert!(redirect.to_string().starts_with("archive_redirect_refused:"));
    let rate = engine.archive.get(Url::parse("https://web.archive.org/limit").unwrap(), 1024, false, false).await.err().unwrap();
    assert!(rate.to_string().starts_with("archive_rate_limited:"));
    let total = fixture.requests.lock().unwrap().len();
    assert!(clone.archive.get(Url::parse("https://web.archive.org/limit").unwrap(), 1024, false, false).await.is_err());
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(total, 7); assert_eq!(requests.len(), total);
    for pair in requests.windows(2) { assert!(pair[1].1.duration_since(pair[0].1) >= Duration::from_millis(2900)); }
    assert!(!requests.iter().any(|(url, _)| url.contains("live-target")));
    if let Some(path) = std::env::var_os("WEBTOOL_ARCHIVE_EVIDENCE") {
        let path = std::path::PathBuf::from(path);
        std::fs::write(path.join("original.html"), HTML).unwrap();
        std::fs::write(path.join("lookup.json"), serde_json::to_vec_pretty(&lookup).unwrap()).unwrap();
        std::fs::write(path.join("first.json"), serde_json::to_vec_pretty(&first.document).unwrap()).unwrap();
        std::fs::write(path.join("second.json"), serde_json::to_vec_pretty(&second.document).unwrap()).unwrap();
    }
    eprintln!("Archive engine: two exact captures preserve qualifications/code/bytes, distinct IDs for identical bytes, saved library/find/cache across clones, seven paced requests, no redirect target or cooldown retry.");
}

#[test]
fn timestamp_scope_and_replay_validation() {
    for invalid in ["2001", "20010230120000", "20010101246000", "20010101120000x"] { assert!(timestamp(invalid).is_err()); }
    for invalid in ["https://example.org/*", "https://web.archive.org/web/2001/http://example.org/", "file:///local", "https://user:synthetic@example.org/"] { assert!(original(invalid).is_err()); }
    let rows = serde_json::to_vec(&json!([
        ["timestamp","original","statuscode","mimetype","digest"],
        [FIRST,SOURCE,"200","text/html","digest"],
        [SECOND,"http://example.org/history.html","200","text/html","digest"]
    ])).unwrap();
    let (found, more, excluded) = captures(&rows, &original(SOURCE).unwrap(), FIRST, SECOND, 1).unwrap();
    assert_eq!(found.len(), 1); assert!(more); assert_eq!(excluded, 1);
    let capture = &found[0];
    let mut headers = header::HeaderMap::new();
    assert!(verify_replay(&headers, capture).is_err());
    headers.insert("memento-datetime", "Tue, 02 Jan 2001 12:00:00 GMT".parse().unwrap());
    assert!(verify_replay(&headers, capture).is_err());
    headers.insert("memento-datetime", "Mon, 01 Jan 2001 12:00:00 GMT".parse().unwrap());
    headers.insert(header::LINK, format!("<{SOURCE}>; rel=\"original\", <https://web.archive.org/>; rel=\"timegate\"").parse().unwrap());
    assert!(verify_replay(&headers, capture).is_ok());
    headers.insert(header::LINK, "<https://example.org/other>; rel=\"timegate original\"".parse().unwrap());
    assert!(verify_replay(&headers, capture).is_err());
    assert!(retry_after(Some("18446744073709551615")) > Utc::now());
    assert!(retry_after(Some("Wed, 01 Jan 2031 12:00:00 GMT")) > Utc::now());
    eprintln!("Archive validation: invalid calendar/scope rejected, no scheme substitution, candidate truncation visible, missing/wrong date/original headers refused, Retry-After seconds/date/overflow respected.");
}
