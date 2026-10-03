use std::{sync::Arc, time::Duration};

use anyhow::{bail, Result};
use futures_util::{stream::FuturesUnordered, StreamExt};
use tokio::{sync::{Mutex, Semaphore}, time::{sleep_until, timeout, timeout_at, Instant}};
use webtool_protocol::{SearchRequest, SearchResponse, Warning};

use crate::config::{Config, SearchProviderConfig};
use super::{json::{Failure, JsonProvider}, merge, organic, ProviderResults};

/// Pooled clients and one set of limits for all Engine clones in this service.
#[derive(Clone)]
pub(crate) struct SearchService {
    client: reqwest::Client,
    json_client: reqwest::Client,
    providers: Arc<Vec<Provider>>,
    slots: Arc<Semaphore>,
    timeout: Duration,
    max_bytes: usize,
}

struct Provider {
    name: String,
    json: Option<JsonProvider>,
    config: SearchProviderConfig,
    slots: Semaphore,
    next_start: Mutex<Instant>,
    #[cfg(test)]
    endpoint: Option<String>,
}

impl SearchService {
    pub(crate) fn new(config: &Config) -> Result<Self> {
        config.validate()?;
        let providers = config.search_engines.iter().map(|name| {
            let json = match name.as_str() {
                "brave_api" => Some(JsonProvider::Brave(config.search.brave_api.clone())),
                "searxng" => Some(JsonProvider::Searxng(config.search.searxng.clone())),
                _ => None,
            };
            let config = config.search.providers.get(name).cloned().unwrap_or_default();
            Provider {
                name: name.clone(),
                json,
                slots: Semaphore::new(config.concurrency),
                config,
                next_start: Mutex::new(Instant::now()),
                #[cfg(test)]
                endpoint: None,
            }
        }).collect();
        Ok(Self {
            client: metadata_search_engine_rs::engines::build_http_client()?,
            // Do not share HTML cookies/default headers with a keyed API. Refuse all
            // JSON redirects so credentials cannot cross an origin or downgrade TLS.
            json_client: reqwest::Client::builder().user_agent(&config.user_agent)
                .redirect(reqwest::redirect::Policy::none()).referer(false)
                .timeout(Duration::from_secs(20)).build()?,
            providers: Arc::new(providers),
            slots: Arc::new(Semaphore::new(config.search.concurrency)),
            timeout: Duration::from_millis(config.search.timeout_ms),
            max_bytes: config.search.max_bytes.min(config.max_bytes),
        })
    }

    pub(crate) async fn search(&self, request: SearchRequest) -> Result<SearchResponse> {
        if request.query.trim().is_empty() || request.query.len() > 4096 {
            bail!("query must contain 1 to 4096 bytes");
        }
        if !(1..=50).contains(&request.limit) { bail!("search limit must be between 1 and 50"); }
        let start = Instant::now();
        let deadline = start + self.timeout;
        let mut pending = FuturesUnordered::new();
        for (index, provider) in self.providers.iter().enumerate() {
            let request = &request;
            pending.push(async move {
                let outcome = timeout_at(deadline, self.run(provider, request)).await
                    .unwrap_or_else(|_| Err(Warning::new("provider_timeout", format!(
                        "{}: global search deadline of {} ms reached, including admission and pacing waits.",
                        provider.name, self.timeout.as_millis()))));
                (index, outcome)
            });
        }
        let mut answers: Vec<_> = (0..self.providers.len()).map(|_| None).collect();
        // Keep completed providers when another provider exhausts its budget.
        while let Some((index, answer)) = pending.next().await { answers[index] = Some(answer); }
        drop(pending);
        let mut rows = Vec::new();
        let mut warnings = Vec::new();
        // Completion order must not select titles/snippets or change ranking sums.
        for (provider, answer) in self.providers.iter().zip(answers) {
            match answer.expect("every provider future returned an outcome") {
                Ok(ProviderResults { items, warnings: provider_warnings }) => {
                    if items.is_empty() {
                        warnings.push(Warning::new("provider_empty", format!(
                            "{} returned no parsed organic results. This can mean no matches, excluded ads, or unrecognized upstream markup.", provider.name)));
                    }
                    warnings.extend(provider_warnings);
                    rows.push((provider.name.clone(), items));
                }
                Err(warning) => warnings.push(warning),
            }
        }
        Ok(SearchResponse {
            query: request.query,
            results: merge(rows, request.limit),
            warnings,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn run(&self, provider: &Provider, request: &SearchRequest) -> Result<ProviderResults, Warning> {
        let client = if provider.json.is_some() { &self.json_client } else { &self.client };
        // Configuration and unsupported-query failures make no request and consume
        // no pacing reservation. A missing optional provider does not stop others.
        let outgoing = match &provider.json {
            Some(json) => json.request(client, request),
            None => organic::request(client, &provider.name, &request.query),
        }.map_err(|error| failure(&provider.name, error))?;
        #[cfg(test)]
        let outgoing = match &provider.endpoint {
            Some(endpoint) => {
                let mut outgoing = outgoing.build().unwrap();
                let mut url = url::Url::parse(endpoint).unwrap();
                url.set_query(outgoing.url().query());
                *outgoing.url_mut() = url;
                reqwest::RequestBuilder::from_parts(client.clone(), outgoing)
            }
            None => outgoing,
        };
        // Provider admission includes its pacing wait. Do not hold global capacity
        // during that wait, and do not reserve future starts for canceled requests.
        let _provider = provider.slots.acquire().await.expect("search semaphore stays open");
        let mut next_start = provider.next_start.lock().await;
        sleep_until(*next_start).await;
        let _global = self.slots.acquire().await.expect("search semaphore stays open");
        *next_start = Instant::now() + Duration::from_millis(provider.config.interval_ms);
        drop(next_start);

        let read = async {
            match &provider.json {
                Some(json) => json.search(outgoing, request.limit, self.max_bytes).await,
                None => organic::search(outgoing, &provider.name, request.limit, self.max_bytes).await
                    .map(|items| ProviderResults { items, warnings: Vec::new() }),
            }
        };
        timeout(Duration::from_millis(provider.config.timeout_ms), read).await
            .map_err(|_| Warning::new("provider_timeout", format!(
                "{}: provider request/body budget of {} ms reached.", provider.name, provider.config.timeout_ms)))?
            .map_err(|error| failure(&provider.name, error))
    }
}

fn failure(provider: &str, error: anyhow::Error) -> Warning {
    let code = if let Some(json) = error.downcast_ref::<Failure>() {
        json.code
    } else if error.is::<organic::Blocked>() {
        "provider_blocked"
    } else if let Some(http) = error.downcast_ref::<reqwest::Error>() {
        if http.is_timeout() { "provider_timeout" }
        else if http.status().is_some_and(|status| matches!(status.as_u16(), 401 | 403 | 429 | 451)) { "provider_blocked" }
        else { "provider_error" }
    } else { "provider_error" };
    Warning::new(code, format!("{provider}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex as StdMutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    type Events = Arc<StdMutex<Vec<(String, bool, Instant)>>>;
    struct LocalProviders {
        url: String,
        events: Events,
        requests: Arc<StdMutex<Vec<String>>>,
        task: tokio::task::JoinHandle<()>,
    }
    impl Drop for LocalProviders {
        fn drop(&mut self) { self.task.abort(); }
    }
    async fn local(routes: Vec<(&'static str, u64, &'static str, String)>) -> LocalProviders {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let routes: Arc<HashMap<_, _>> = Arc::new(routes.into_iter().map(|(path, delay, status, body)|
            (path, (delay, status, body))).collect());
        let events: Events = Arc::new(StdMutex::new(Vec::new()));
        let trace = events.clone();
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let capture = requests.clone();
        let task = tokio::spawn(async move {
            let mut children = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut socket, _)) = accepted else { break; };
                        let routes = routes.clone();
                        let trace = trace.clone();
                        let capture = capture.clone();
                        children.spawn(async move {
                            let mut input = [0; 8192];
                            let mut n = 0;
                            while n < input.len() && !input[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                                let size = socket.read(&mut input[n..]).await.unwrap_or(0);
                                if size == 0 { return; }
                                n += size;
                            }
                            let request = String::from_utf8_lossy(&input[..n]);
                            capture.lock().unwrap().push(request.to_string());
                            let path = request.split_whitespace().nth(1).unwrap().split('?').next().unwrap();
                            let (delay, status, body) = routes.get(path).unwrap();
                            trace.lock().unwrap().push((path.into(), true, Instant::now()));
                            tokio::time::sleep(Duration::from_millis(*delay)).await;
                            // Redirect cases use the synthetic body as their Location.
                            let location = if status.starts_with("302") { format!("Location: {body}\r\n") } else { String::new() };
                            let response = format!("HTTP/1.1 {status}\r\n{location}Content-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                            trace.lock().unwrap().push((path.into(), false, Instant::now()));
                            let _ = socket.write_all(response.as_bytes()).await;
                        });
                    }
                    Some(_) = children.join_next(), if !children.is_empty() => {}
                }
            }
        });
        LocalProviders { url, events, requests, task }
    }
    fn card(provider: &str) -> String {
        match provider {
            "duckduckgo" => "<div class='result web-result'><a class='result__a' href='https://example.org/source?x=1'>First provider</a><a class='result__snippet'>DDG summary</a></div>",
            "brave" => "<div data-type='web'><a class='l1' href='https://example.org/source?x=1'><div class='search-snippet-title'>Second provider</div></a><div class='generic-snippet'>Brave summary</div></div>",
            _ => "",
        }.into()
    }
    fn configured(local: &LocalProviders, names: &[&str], deadline: u64, concurrency: usize, interval: u64) -> SearchService {
        let mut config = Config::default();
        config.search_engines = names.iter().map(|name| (*name).into()).collect();
        config.search.timeout_ms = deadline;
        config.search.concurrency = concurrency;
        for name in names {
            config.search.providers.insert((*name).into(), SearchProviderConfig {
                timeout_ms: 2000, interval_ms: interval, concurrency: 1,
            });
        }
        with_endpoints(&config, local)
    }
    fn with_endpoints(config: &Config, local: &LocalProviders) -> SearchService {
        let mut service = SearchService::new(config).unwrap();
        for provider in Arc::get_mut(&mut service.providers).unwrap() {
            provider.endpoint = Some(format!("{}/{}", local.url, provider.name));
        }
        service
    }
    fn request() -> SearchRequest {
        SearchRequest { query: "literal query & no rewrite".into(), limit: 5, library: None }
    }

    const SYNTHETIC_KEY: &str = "synthetic-search-key-not-a-real-credential";
    fn json_config(local: &LocalProviders) -> Config {
        let mut config = Config::default();
        config.search_engines = vec!["brave_api".into(), "searxng".into(), "duckduckgo".into()];
        config.search.timeout_ms = 2000;
        config.search.concurrency = 1;
        config.search.brave_api.api_key_env = Some(format!("WEBTOOL_SYNTHETIC_{}", uuid::Uuid::new_v4().simple()));
        config.search.searxng.endpoint = Some(format!("{}/searxng", local.url));
        for name in &config.search_engines {
            config.search.providers.insert(name.clone(), SearchProviderConfig {
                timeout_ms: 1000, interval_ms: 300, concurrency: 1,
            });
        }
        config
    }
    fn fixture_endpoint(service: &mut SearchService, name: &str, endpoint: String) {
        Arc::get_mut(&mut service.providers).unwrap().iter_mut().find(|p| p.name == name).unwrap().endpoint = Some(endpoint);
    }
    fn assert_redacted(answer: &SearchResponse, local: &LocalProviders) {
        for warning in &answer.warnings {
            for forbidden in [SYNTHETIC_KEY, local.url.as_str(), "literal", "raw-error-sentinel", "x-subscription-token"] {
                assert!(!warning.message.to_lowercase().contains(forbidden), "unsafe provider warning");
            }
        }
    }
    fn brave_json() -> String {
        serde_json::json!({"type":"search","web":{"type":"search","results":[
            {"type":"ad","title":"Paid type","url":"https://example.org/paid-type"},
            {"type":"search_result","is_sponsored":true,"title":"Paid flag","url":"https://example.org/paid-flag"},
            {"type":"search_result","title":"Paid URL","url":"https://example.org/?gclid=paid"},
            {"type":"search_result","title":"Advertising research","url":"https://example.org/source?x=1","description":"Sponsored content is the topic."}
        ]},"ads":{"results":[{"title":"Ignore ads","url":"https://example.org/other"}]},
        "videos":{"results":[{"title":"Ignore video","url":"https://example.org/video"}]}}).to_string()
    }
    fn searxng_json() -> String {
        serde_json::json!({"results":[
            {"template":"images.html","category":"images","title":"Image","url":"https://example.org/image"},
            {"template":"default.html","sponsored":true,"title":"Paid","url":"https://example.org/paid"},
            {"template":"default.html","title":"Paid URL","url":"https://example.org/?utm_medium=cpc"},
            {"template":"default.html","title":42,"url":"https://example.org/malformed"},
            {"template":"default.html","category":"general","title":"SearXNG summary","url":"https://example.org/source?x=1","content":"A snippet, not fetched evidence.","engines":["brave","duckduckgo","google"],"positions":[1,1,1],"score":999}
        ],"answers":[{"answer":"Not a web result"}],"unresponsive_engines":[["synthetic-upstream","raw-error-sentinel"]]}).to_string()
    }

    #[tokio::test]
    async fn json_organic_selection_and_shared_budgets() {
        let local = local(vec![
            ("/brave_api", 20, "200 OK", brave_json()),
            ("/searxng", 20, "200 OK", searxng_json()),
            ("/duckduckgo", 20, "200 OK", card("duckduckgo")),
        ]).await;
        let config = json_config(&local);
        let name = config.search.brave_api.api_key_env.as_ref().unwrap();
        std::env::set_var(name, SYNTHETIC_KEY);
        let service = with_endpoints(&config, &local);
        let mut query = request(); query.limit = 1;
        let outgoing = service.providers[0].json.as_ref().unwrap().request(&service.json_client, &query).unwrap().build().unwrap();
        let token = &outgoing.headers()["X-Subscription-Token"];
        assert!(token.is_sensitive());
        assert!(!format!("{outgoing:?}").contains(SYNTHETIC_KEY));
        assert_eq!(outgoing.url().scheme(), "https");
        assert_eq!(outgoing.url().host_str(), Some("api.search.brave.com"));
        assert!(!format!("{config:?}").contains(SYNTHETIC_KEY));
        assert!(!serde_json::to_string(&config).unwrap().contains(SYNTHETIC_KEY));
        let clone = service.clone();
        let (a, b) = tokio::join!(service.search(query.clone()), clone.search(query));
        for answer in [a.unwrap(), b.unwrap()] {
            assert_eq!(answer.results.len(), 1);
            let row = &answer.results[0];
            assert_eq!(row.title, "Advertising research");
            assert_eq!(row.snippet, "Sponsored content is the topic.");
            assert_eq!(row.providers, ["brave_api", "searxng", "duckduckgo"]);
            assert!((row.score - 3.0 / 61.0).abs() < 1e-12);
            assert_eq!(answer.warnings.len(), 1);
            assert_eq!(answer.warnings[0].code, "provider_partial");
            assert!(answer.warnings[0].message.contains("1 malformed rows and 1 reported upstream failures"));
            assert_redacted(&answer, &local);
        }
        let mut active = 0;
        let mut peak = 0;
        let mut last: HashMap<&str, Instant> = HashMap::new();
        let events = local.events.lock().unwrap();
        for (path, start, time) in events.iter() {
            if *start {
                active += 1; peak = peak.max(active);
                if let Some(previous) = last.insert(path, *time) {
                    assert!(*time - previous >= Duration::from_millis(280));
                }
            } else { active -= 1; }
        }
        assert_eq!(peak, 1);
        assert_eq!(last.len(), 3);
        for raw in local.requests.lock().unwrap().iter() {
            let target = raw.split_whitespace().nth(1).unwrap();
            let url = url::Url::parse(&format!("http://localhost{target}")).unwrap();
            let params: HashMap<_, _> = url.query_pairs().collect();
            assert_eq!(params["q"], request().query);
            if url.path() == "/brave_api" {
                assert!(raw.contains(SYNTHETIC_KEY));
                assert_eq!(params["spellcheck"], "false");
                assert_eq!(params["result_filter"], "web");
                assert_eq!(params["text_decorations"], "false");
            } else { assert!(!raw.contains(SYNTHETIC_KEY)); }
            if url.path() == "/searxng" {
                assert_eq!(params["format"], "json"); assert_eq!(params["categories"], "general");
            }
        }
        std::env::remove_var(name);
        eprintln!("JSON providers: organic fields only, paid rows excluded before limit, one SearXNG vote, shared global peak 1 and pacing >=280 ms; key header sensitive and isolated");
    }

    #[tokio::test]
    async fn json_failures_keep_partial_results_without_disclosing_credentials() {
        let target = local(vec![("/target", 0, "200 OK", brave_json())]).await;
        let local = local(vec![
            ("/brave_api", 0, "429 Too Many Requests", format!("{SYNTHETIC_KEY} raw-error-sentinel")),
            ("/searxng", 0, "200 OK", searxng_json()),
            ("/duckduckgo", 0, "200 OK", card("duckduckgo")),
            ("/malformed", 0, "200 OK", format!("{{\"results\":\"{SYNTHETIC_KEY} raw-error-sentinel\"}}")),
            ("/slow", 1000, "200 OK", brave_json()),
            ("/redirect", 0, "302 Found", format!("{}/target", target.url)),
            ("/error", 0, "200 OK", format!("{{\"error\":\"{SYNTHETIC_KEY} raw-error-sentinel\"}}")),
            ("/large", 0, "200 OK", "x".repeat(2048)),
        ]).await;
        let mut config = json_config(&local);
        let name = config.search.brave_api.api_key_env.clone().unwrap();
        config.search.timeout_ms = 500;
        for provider in config.search.providers.values_mut() { provider.interval_ms = 0; provider.timeout_ms = 100; }
        // This random test-owned reference has no value. No real key is read.
        config.search.searxng.endpoint = None;
        let answer = with_endpoints(&config, &local).search(request()).await.unwrap();
        assert_eq!(answer.results[0].providers, ["duckduckgo"]);
        assert_eq!(answer.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(), ["provider_unconfigured", "provider_unconfigured"]);
        assert_eq!(local.requests.lock().unwrap().len(), 1);
        assert_redacted(&answer, &local);
        config.search.searxng.endpoint = Some(format!("{}/searxng", local.url));
        let mut service = with_endpoints(&config, &local);
        fixture_endpoint(&mut service, "searxng", format!("{}/malformed", local.url));
        let answer = service.search(request()).await.unwrap();
        assert_eq!(answer.results[0].providers, ["duckduckgo"]);
        assert_eq!(answer.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(), ["provider_unconfigured", "provider_error"]);
        assert_redacted(&answer, &local);
        std::env::set_var(&name, SYNTHETIC_KEY);
        for (path, code) in [("brave_api", "provider_blocked"), ("slow", "provider_timeout"),
            ("redirect", "provider_error"), ("error", "provider_error"), ("large", "provider_error")] {
            let mut service = with_endpoints(&config, &local);
            fixture_endpoint(&mut service, "brave_api", format!("{}/{path}", local.url));
            if path == "large" { service.max_bytes = 1024; }
            let answer = service.search(request()).await.unwrap();
            assert_eq!(answer.results[0].providers, ["searxng", "duckduckgo"], "{path}");
            assert_eq!(answer.warnings[0].code, code, "{path}");
            assert!(answer.elapsed_ms < 500);
            assert_redacted(&answer, &local);
            if path == "redirect" { assert!(answer.warnings[0].message.contains("redirect refused (HTTP 302)")); }
        }
        assert!(target.requests.lock().unwrap().is_empty(), "the credentialed redirect must not be followed");
        let mut query = request(); query.query = "x".repeat(601);
        let answer = with_endpoints(&config, &local).search(query).await.unwrap();
        assert_eq!(answer.warnings[0].code, "provider_unsupported");
        std::env::remove_var(name);
        eprintln!("JSON provider failures: partial results survive missing config/key, malformed JSON, HTTP 429, timeout, redirect, error envelope, and byte cap; no redirected key or raw error disclosure");
    }

    #[tokio::test]
    async fn fast_results_survive_slow_blocked_and_failed_providers() {
        let local = local(vec![
            ("/duckduckgo", 0, "200 OK", card("duckduckgo")),
            ("/brave", 1000, "200 OK", card("brave")),
            ("/startpage", 0, "403 Forbidden", "blocked".into()),
            ("/yahoo", 0, "500 Internal Server Error", "failed".into()),
        ]).await;
        let mut service = configured(&local, &["duckduckgo", "brave", "startpage", "yahoo"], 700, 4, 0);
        Arc::get_mut(&mut service.providers).unwrap()[1].config.timeout_ms = 150;
        let answer = service.search(request()).await.unwrap();
        assert_eq!(answer.results.len(), 1);
        assert_eq!(answer.results[0].providers, ["duckduckgo"]);
        assert_eq!(answer.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(),
            ["provider_timeout", "provider_blocked", "provider_error"]);
        for (warning, name) in answer.warnings.iter().zip(["brave", "startpage", "yahoo"]) {
            assert!(warning.message.starts_with(name));
            assert!(!warning.message.contains("literal"));
            assert!(!warning.message.contains(&local.url));
        }
        assert!(answer.elapsed_ms < 700, "{}", answer.elapsed_ms);
        eprintln!("partial provider result: {} ms, 1 result, timeout/blocked/error", answer.elapsed_ms);
    }

    #[tokio::test]
    async fn empty_challenge_and_oversize_responses_are_distinct() {
        let local = local(vec![
            ("/duckduckgo", 0, "200 OK", "<form id='challenge-form'></form>".into()),
            ("/brave", 0, "200 OK", "<main>No results</main>".into()),
            ("/startpage", 0, "200 OK", "x".repeat(2048)),
        ]).await;
        let mut service = configured(&local, &["duckduckgo", "brave", "startpage"], 1000, 4, 0);
        service.max_bytes = 1024;
        let answer = service.search(request()).await.unwrap();
        assert!(answer.results.is_empty());
        assert_eq!(answer.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(),
            ["provider_blocked", "provider_empty", "provider_error"]);
        assert!(answer.warnings[2].message.contains("1024-byte limit"));
    }

    #[tokio::test]
    async fn global_deadline_includes_shared_pacing_and_cancellation_does_not_reserve_a_start() {
        let local = local(vec![("/duckduckgo", 10, "200 OK", card("duckduckgo"))]).await;
        let service = configured(&local, &["duckduckgo"], 220, 1, 300);
        let clone = service.clone();
        let (first, queued) = tokio::join!(service.search(request()), clone.search(request()));
        let first = first.unwrap();
        let queued = queued.unwrap();
        assert_eq!(first.results.len() + queued.results.len(), 1);
        let expired = if first.results.is_empty() { first } else { queued };
        assert_eq!(expired.warnings[0].code, "provider_timeout");
        assert!(expired.warnings[0].message.contains("global search deadline"));
        assert_eq!(local.events.lock().unwrap().iter().filter(|(_, start, _)| *start).count(), 1);
        let next = service.search(request()).await.unwrap();
        assert_eq!(next.results.len(), 1, "{:?}", next.warnings);
        let starts: Vec<_> = local.events.lock().unwrap().iter().filter(|(_, start, _)| *start).map(|(_, _, time)| *time).collect();
        assert_eq!(starts.len(), 2);
        assert!(starts[1] - starts[0] >= Duration::from_millis(280));
        eprintln!("shared pacing: {:?} between local starts; queued search expired at {} ms", starts[1] - starts[0], expired.elapsed_ms);
    }

    #[tokio::test]
    async fn service_clones_share_admission_and_merge_in_configured_not_completion_order() {
        let local = local(vec![
            ("/duckduckgo", 100, "200 OK", card("duckduckgo")),
            ("/brave", 5, "200 OK", card("brave")),
        ]).await;
        let service = configured(&local, &["duckduckgo", "brave"], 2000, 1, 0);
        let clone = service.clone();
        let (a, b) = tokio::join!(service.search(request()), clone.search(request()));
        for answer in [a.unwrap(), b.unwrap()] {
            assert!(answer.warnings.is_empty());
            assert_eq!(answer.results[0].title, "First provider");
            assert_eq!(answer.results[0].providers, ["duckduckgo", "brave"]);
            assert_eq!(answer.results[0].score, 2.0 / 61.0);
        }
        let mut active = 0;
        let mut peak = 0;
        for (_, start, _) in local.events.lock().unwrap().iter() {
            if *start { active += 1; peak = peak.max(active); } else { active -= 1; }
        }
        assert_eq!(peak, 1, "global provider-request admission is shared by clones");
        // Allow distinct providers together, while keeping each provider at one.
        let service = configured(&local, &["duckduckgo", "brave"], 2000, 4, 0);
        local.events.lock().unwrap().clear();
        let clone = service.clone();
        let (a, b) = tokio::join!(service.search(request()), clone.search(request()));
        for answer in [a.unwrap(), b.unwrap()] {
            assert_eq!(answer.results[0].title, "First provider");
            assert_eq!(answer.results[0].providers, ["duckduckgo", "brave"]);
        }
        let mut active = HashMap::new();
        for (name, start, _) in local.events.lock().unwrap().iter() {
            let count = active.entry(name.clone()).or_insert(0);
            if *start { *count += 1; assert_eq!(*count, 1); } else { *count -= 1; }
        }
        eprintln!("shared admission: global peak 1; per-provider peak 1; stable title and RRF order");
    }
}
