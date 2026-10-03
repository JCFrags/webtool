//! One service-owned connection per provider, bounded reads, and no retries.
use std::{sync::Arc, time::Duration};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::{header::HeaderMap, Client, StatusCode};
use tokio::{sync::Mutex, time::Instant};
use webtool_protocol::ScholarlyProvider;
use super::ScholarlyError;

#[derive(Clone)]
pub(super) struct ProviderClient {
    client: Client,
    provider: ScholarlyProvider,
    budget: Arc<Mutex<Budget>>,
    timeout: Duration,
    cap: usize,
}
struct Budget { next: Instant, cooldown: Instant, spacing: Duration }
pub(super) struct Response { pub bytes: Vec<u8>, pub url: String, pub status: u16, pub observed_at: String }
impl ProviderClient {
    pub fn new(provider: ScholarlyProvider, timeout: Duration, cap: usize) -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent(concat!("webtool/", env!("CARGO_PKG_VERSION"), " scholarly-metadata (no automatic retries)"))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(10)).timeout(timeout)
            .pool_max_idle_per_host(1).build()?;
        Ok(Self { client, provider, budget: Arc::new(Mutex::new(Budget {
            next: Instant::now(), cooldown: Instant::now(), spacing: Duration::from_secs(1),
        })), timeout, cap })
    }
    pub async fn get(&self, url: url::Url) -> Result<Response, ScholarlyError> {
        self.get_capped(url, self.cap).await
    }
    pub async fn get_capped(&self, url: url::Url, cap: usize) -> Result<Response, ScholarlyError> {
        tokio::time::timeout(self.timeout, self.get_inner(url, cap)).await.map_err(|_| ScholarlyError::Timeout)?
    }
    async fn get_inner(&self, url: url::Url, cap: usize) -> Result<Response, ScholarlyError> {
        let mut budget = self.budget.lock().await;
        if budget.cooldown > Instant::now() { return Err(ScholarlyError::RateLimited); }
        tokio::time::sleep_until(budget.next).await;
        // All arXiv paths, including ordinary reads, share the existing gate.
        let mut arxiv_slot = if self.provider == ScholarlyProvider::Arxiv { Some(crate::arxiv::slot().await) } else { None };
        // Reserve spacing before sending so cancellation cannot erase pacing.
        budget.next = Instant::now() + budget.spacing;
        let response = self.client.get(url).send().await.map_err(|e|
            if e.is_timeout() { ScholarlyError::Timeout } else { ScholarlyError::ProviderFailed })?;
        let status = response.status();
        let cooldown = cooldown(response.headers(), status);
        budget.spacing = provider_spacing(response.headers()).max(budget.spacing);
        if let Some(delay) = cooldown {
            budget.cooldown = Instant::now() + delay;
            if let Some(slot) = arxiv_slot.as_mut() { slot.cooldown(delay); }
        }
        check_status(status)?;
        if response.content_length().is_some_and(|size| size > cap as u64) { return Err(ScholarlyError::TooLarge); }
        let resolved = response.url().to_string();
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(part) = stream.next().await {
            let part = part.map_err(|_| ScholarlyError::ProviderFailed)?;
            if bytes.len().saturating_add(part.len()) > cap { return Err(ScholarlyError::TooLarge); }
            bytes.extend_from_slice(&part);
        }
        budget.next = Instant::now() + budget.spacing;
        Ok(Response { bytes, url: resolved, status: status.as_u16(), observed_at: Utc::now().to_rfc3339() })
    }
}
fn check_status(status: StatusCode) -> Result<(), ScholarlyError> {
    match status.as_u16() {
        200 => Ok(()),
        429 => Err(ScholarlyError::RateLimited),
        404 | 410 => Err(ScholarlyError::NotFound),
        401 | 403 => Err(ScholarlyError::AccessDenied),
        _ => Err(ScholarlyError::ProviderFailed),
    }
}
fn cooldown(headers: &HeaderMap, status: StatusCode) -> Option<Duration> {
    let retry = headers.get("retry-after").and_then(|v| v.to_str().ok()).and_then(|v| {
        v.parse::<u32>().ok().map(u64::from).or_else(|| DateTime::parse_from_rfc2822(v).ok()
            .map(|d| (d.timestamp() - Utc::now().timestamp()).max(1) as u64))
    });
    // OpenAlex may report a depleted daily budget instead of Retry-After.
    let reset = if headers.get("x-ratelimit-remaining").and_then(|v| v.to_str().ok()) == Some("0") {
        headers.get("x-ratelimit-reset").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<i64>().ok())
            .map(|at| (at - Utc::now().timestamp()).max(1) as u64)
    } else { None };
    retry.into_iter().chain(reset).max().or_else(|| (status == StatusCode::TOO_MANY_REQUESTS).then_some(60))
        .map(Duration::from_secs)
}
fn provider_spacing(headers: &HeaderMap) -> Duration {
    let count = headers.get("x-rate-limit-limit").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u32>().ok()).filter(|n| *n > 0);
    let seconds = headers.get("x-rate-limit-interval").and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_suffix('s')).and_then(|v| v.parse::<u32>().ok());
    match (count, seconds) {
        (Some(n), Some(s)) => Duration::from_secs_f64((s as f64 / n as f64).max(1.0)),
        _ => Duration::from_secs(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rate_and_error_contract() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "120".parse().unwrap());
        assert_eq!(cooldown(&headers, StatusCode::TOO_MANY_REQUESTS), Some(Duration::from_secs(120)));
        assert!(matches!(check_status(StatusCode::FORBIDDEN), Err(ScholarlyError::AccessDenied)));
        assert!(matches!(check_status(StatusCode::MOVED_PERMANENTLY), Err(ScholarlyError::ProviderFailed)));
        assert!(matches!(check_status(StatusCode::TOO_MANY_REQUESTS), Err(ScholarlyError::RateLimited)));
        headers.insert("x-rate-limit-limit", "1".parse().unwrap());
        headers.insert("x-rate-limit-interval", "3s".parse().unwrap());
        assert_eq!(provider_spacing(&headers), Duration::from_secs(3));
    }
    #[tokio::test]
    async fn streamed_cap_and_shared_cooldown() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = url::Url::parse(&format!("http://{}/", listener.local_addr().unwrap())).unwrap();
        let server = tokio::spawn(async move {
            for reply in ["HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n12345\r\n0\r\n\r\n", "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 60\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096]; let _ = socket.read(&mut request).await;
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let client = ProviderClient::new(ScholarlyProvider::Crossref, Duration::from_secs(5), 4).unwrap();
        assert!(matches!(client.get(url.clone()).await, Err(ScholarlyError::TooLarge)));
        assert!(matches!(client.get(url.clone()).await, Err(ScholarlyError::RateLimited)));
        server.await.unwrap();
        // No third request reaches the closed listener; the shared cooldown wins.
        assert!(matches!(client.clone().get(url).await, Err(ScholarlyError::RateLimited)));
    }
}
