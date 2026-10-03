use std::{collections::VecDeque, sync::Arc, time::Duration};
use anyhow::Result;
use chrono::Utc;
use reqwest::header::HeaderValue;
use tokio::{sync::Mutex, time::{timeout_at, Instant}};
use url::Url;
use webtool_protocol::*;
use crate::{config::Config, Engine};
use super::{config::endpoint, ExternalCodeError as E, ExternalProviderConfig};

#[derive(Clone)]
pub(crate) struct ExternalCodeService {
    client: reqwest::Client,
    sourcegraph: Arc<Provider>,
    context7: Arc<Provider>,
    max_bytes: usize,
}
struct Provider { config: ExternalProviderConfig, gate: Mutex<Gate> }
struct Gate { next: Instant, blocked: Instant, starts: VecDeque<Instant> }
impl Gate {
    fn prune(&mut self) { while self.starts.front().is_some_and(|t| t.elapsed() >= Duration::from_secs(60)) { self.starts.pop_front(); } }
}
pub(super) struct Received {
    pub bytes: Vec<u8>,
    pub observation: ExternalIndexObservation,
    pub stopped: Option<Problem>,
}
impl ExternalCodeService {
    pub(crate) fn new(config: &Config) -> Result<Self> {
        let provider = |config: ExternalProviderConfig| Arc::new(Provider { config,
            gate: Mutex::new(Gate { next: Instant::now(), blocked: Instant::now(), starts: VecDeque::new() }) });
        Ok(Self { client: reqwest::Client::builder().user_agent(&config.user_agent)
            .redirect(reqwest::redirect::Policy::none()).referer(false).no_proxy()
            .connect_timeout(Duration::from_secs(5)).build()?,
            sourcegraph: provider(config.external_code.sourcegraph.clone()),
            context7: provider(config.external_code.context7.clone()),
            max_bytes: config.external_code.max_bytes.min(config.max_bytes) })
    }
    fn provider(&self, which: ExternalCodeProvider) -> &Provider {
        match which { ExternalCodeProvider::Sourcegraph => &self.sourcegraph, ExternalCodeProvider::Context7 => &self.context7 }
    }
    pub(super) fn base(&self, which: ExternalCodeProvider) -> Result<Url> {
        endpoint(self.provider(which).config.endpoint.as_deref().ok_or(E::Unconfigured)?)
            .map_err(|_| E::Unconfigured.into())
    }
    pub(crate) async fn status(&self) -> ExternalProvidersResponse {
        let mut providers = Vec::new();
        for which in [ExternalCodeProvider::Sourcegraph, ExternalCodeProvider::Context7] {
            let p = self.provider(which);
            let mut gate = p.gate.lock().await;
            gate.prune();
            let key_ready = credential(&p.config, which).is_ok();
            providers.push(ExternalProviderStatus { provider: which, endpoint_configured: p.config.endpoint.is_some(),
                credential_configured: p.config.credential_env.is_some(), locally_ready: p.config.endpoint.is_some() && key_ready,
                cooldown_seconds: gate.blocked.saturating_duration_since(Instant::now()).as_secs().saturating_add(u64::from(gate.blocked > Instant::now())),
                requests_in_window: gate.starts.len(), max_requests_per_minute: p.config.max_requests_per_minute,
                detail: "Local configuration only. Credentials, endpoints, account access, service health and index coverage are not disclosed or probed.".into() });
        }
        ExternalProvidersResponse { providers }
    }
    pub(super) async fn get(&self, engine: &Engine, which: ExternalCodeProvider, url: Url, deadline: Instant, partial: bool) -> Result<Received> {
        let p = self.provider(which);
        let base = self.base(which)?;
        // URLs are built only from this configured base. Refuse accidental cross-origin callers.
        if url.origin() != base.origin() { return Err(E::Identity.into()); }
        let key = credential(&p.config, which)?;
        let mut gate = p.gate.lock().await;
        gate.prune();
        if gate.blocked > Instant::now() { return Err(E::RateLimited.into()); }
        if gate.starts.len() >= p.config.max_requests_per_minute {
            gate.blocked = *gate.starts.front().expect("nonempty budget") + Duration::from_secs(60);
            return Err(E::RateLimited.into());
        }
        // Keep ordinary network capacity free while this provider is paced.
        tokio::time::sleep_until(gate.next).await;
        let _network = engine.network.acquire().await?;
        // Leave time to parse and retain an admitted partial stream before the whole deadline.
        let body_deadline = (Instant::now() + Duration::from_millis(p.config.timeout_ms)).min(deadline - Duration::from_millis(200));
        if body_deadline <= Instant::now() { return Err(E::Timeout.into()); }
        let mut request = self.client.get(url.clone()).header("Accept", if which == ExternalCodeProvider::Sourcegraph { "text/event-stream" } else { "application/json" });
        if let Some(key) = &key {
            let prefix = if which == ExternalCodeProvider::Sourcegraph { "token" } else { "Bearer" };
            let mut value = HeaderValue::from_str(&format!("{prefix} {key}")).map_err(|_| E::Unconfigured)?;
            value.set_sensitive(true);
            request = request.header("Authorization", value);
        }
        gate.starts.push_back(Instant::now());
        gate.next = Instant::now() + Duration::from_millis(p.config.interval_ms);
        let mut response = timeout_at(body_deadline, request.send()).await.map_err(|_| E::Timeout)?
            .map_err(|_| E::Upstream)?;
        let status = response.status().as_u16();
        let remaining = numeric(response.headers(), "ratelimit-remaining");
        if matches!(status, 401 | 402 | 403 | 429) || remaining == Some(0) {
            let retry = numeric(response.headers(), "retry-after").or_else(|| response.headers().get("retry-after")
                .and_then(|v| v.to_str().ok()).and_then(|s| chrono::DateTime::parse_from_rfc2822(s).ok())
                .map(|t| t.timestamp().saturating_sub(Utc::now().timestamp()).max(0) as u64));
            let reset = numeric(response.headers(), "ratelimit-reset").unwrap_or(0)
                .saturating_sub(Utc::now().timestamp().max(0) as u64);
            gate.blocked = Instant::now() + Duration::from_secs(retry.unwrap_or(60).max(reset).clamp(60, 86400));
        }
        if let Some(error) = status_error(status) { return Err(error.into()); }
        let mime = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").split(';').next().unwrap_or("");
        let expected = if which == ExternalCodeProvider::Sourcegraph { "text/event-stream" } else { "application/json" };
        if mime != expected { return Err(E::Unsupported.into()); }
        if !partial && response.content_length().is_some_and(|n| n > self.max_bytes as u64) { return Err(E::Limit.into()); }
        let mut bytes = Vec::new();
        let mut stopped = None;
        loop {
            match timeout_at(body_deadline, response.chunk()).await {
                Ok(Ok(Some(chunk))) => {
                    let available = self.max_bytes.saturating_sub(bytes.len());
                    bytes.extend_from_slice(&chunk[..chunk.len().min(available)]);
                    if chunk.len() > available { stopped = Some(E::Limit.problem()); break; }
                },
                Ok(Ok(None)) => break,
                Ok(Err(_)) => { stopped = Some(E::Upstream.problem()); break; },
                Err(_) => { stopped = Some(E::Timeout.problem()); break; },
            }
        }
        // Error bodies are never stored. Even a success body cannot become a credential artifact.
        if key.as_ref().is_some_and(|key| bytes.windows(key.len()).any(|w| w == key.as_bytes())) { return Err(E::Privacy.into()); }
        if !partial { if let Some(error) = &stopped { return Err(match error.code.as_str() {
            "external_limit" => E::Limit, "external_timeout" => E::Timeout, _ => E::Upstream,
        }.into()); } }
        if bytes.is_empty() { return Err(E::Upstream.into()); }
        let role = match (which, stopped.is_some()) {
            (ExternalCodeProvider::Sourcegraph, true) => "sourcegraph_partial_stream",
            (ExternalCodeProvider::Sourcegraph, false) => "sourcegraph_index_stream",
            _ => "context7_index_response",
        };
        let artifact = engine.store.put_bytes(&bytes, expected, role).await?;
        Ok(Received { bytes, stopped, observation: ExternalIndexObservation { provider: which,
            url: url.into(), observed_at: Utc::now().to_rfc3339(), status, artifact } })
    }
}
fn numeric(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u64> { headers.get(name)?.to_str().ok()?.parse().ok() }
fn credential(config: &ExternalProviderConfig, which: ExternalCodeProvider) -> Result<Option<String>> {
    match &config.credential_env {
        None if which == ExternalCodeProvider::Sourcegraph => Ok(None),
        None => Err(E::Unconfigured.into()),
        Some(name) => {
            let value = std::env::var(name).map_err(|_| E::Unconfigured)?;
            if value.is_empty() || value.len() > 8192 || !value.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.~/+=".contains(&b)) {
                return Err(E::Unconfigured.into());
            }
            Ok(Some(value))
        },
    }
}
fn status_error(status: u16) -> Option<E> { match status {
    200 => None, 202 | 404 | 410 | 503 => Some(E::Unavailable), 401 | 402 | 403 => Some(E::Denied),
    429 => Some(E::RateLimited), 300..=399 => Some(E::Redirect), 400 => Some(E::Invalid),
    422 => Some(E::Unsupported), _ => Some(E::Upstream),
} }
pub(super) fn route(mut base: Url, parts: &[&str]) -> Result<Url> {
    base.path_segments_mut().map_err(|_| E::Invalid)?.pop_if_empty().extend(parts.iter().copied());
    Ok(base)
}
