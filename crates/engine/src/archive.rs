//! Explicit, bounded Wayback operations. No live fallback, browser, or capture creation.
use std::{collections::VecDeque, sync::Arc, time::Duration};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::{header, Client};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::{sync::Mutex, time::{Instant, sleep_until, timeout}};
use url::Url;
use webtool_protocol::*;
use crate::{config::Config, fetch, read_recovery, readers, Engine};

#[cfg(test)] mod tests;

const VERSION: &str = "wayback-replay/1";
const INDEX_URL: &str = "https://web.archive.org/cdx/search/cdx";
const INDEX_LIMIT: usize = 64 * 1024;
const CACHE_SECONDS: u64 = 600;
const CACHE_ENTRIES: usize = 64;

#[derive(Clone)]
pub(crate) struct ArchiveService {
    client: Client,
    state: Arc<Mutex<State>>,
    #[cfg(test)]
    endpoint: Option<String>,
}
struct State {
    next_start: Instant,
    cooldown: Option<DateTime<Utc>>,
    cache: VecDeque<(String, Instant, Received)>,
}
#[derive(Clone)]
struct Received {
    bytes: Vec<u8>,
    headers: header::HeaderMap,
    status: u16,
    retrieved_at: String,
}
impl ArchiveService {
    pub(crate) fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            client: Client::builder().user_agent(&config.user_agent)
                .redirect(reqwest::redirect::Policy::none()).referer(false)
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(config.request_timeout_seconds.min(30))).build()?,
            state: Arc::new(Mutex::new(State { next_start: Instant::now(), cooldown: None, cache: VecDeque::new() })),
            #[cfg(test)] endpoint: None,
        })
    }

    async fn get(&self, url: Url, max: usize, cache: bool, refresh: bool) -> Result<(Received, bool)> {
        // One request at a time across Engine clones, including its body. The outer
        // operation deadline includes admission. No retries or future reservations.
        let mut state = self.state.lock().await;
        let key = url.to_string();
        state.cache.retain(|(_, when, _)| when.elapsed() < Duration::from_secs(CACHE_SECONDS));
        if cache && !refresh {
            if let Some((_, _, received)) = state.cache.iter().find(|(k, _, _)| k == &key) {
                return Ok((received.clone(), true));
            }
        }
        if state.cooldown.is_some_and(|until| until > Utc::now()) {
            bail!("archive_rate_limited: shared archive cooldown is active; no request sent");
        }
        sleep_until(state.next_start).await;
        state.next_start = Instant::now() + Duration::from_secs(3);
        #[cfg(test)]
        let url = if let Some(endpoint) = &self.endpoint {
            let mut local = Url::parse(endpoint)?;
            local.set_path(url.path()); local.set_query(url.query()); local
        } else { url };
        let mut response = self.client.get(url).send().await
            .map_err(|_| anyhow!("archive_upstream_failed: archive request failed"))?;
        let status = response.status().as_u16();
        if status == 429 || status == 503 {
            let retry = response.headers().get(header::RETRY_AFTER).and_then(|v| v.to_str().ok());
            state.cooldown = Some(retry_after(retry));
            bail!("archive_rate_limited: archive requested a cooldown; no retry attempted");
        }
        if response.status().is_redirection() {
            // Refuse even same-archive redirects. In particular, never follow a
            // different date, original URL, or a live-web Location.
            bail!("archive_redirect_refused: archive returned HTTP {status}; no redirect followed");
        }
        if status != 200 {
            bail!("archive_upstream_failed: archive returned HTTP {status}");
        }
        if response.content_length().is_some_and(|n| n > max as u64) {
            bail!("archive_size_limit: archive response exceeds the byte limit");
        }
        let headers = response.headers().clone();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await
            .map_err(|_| anyhow!("archive_upstream_failed: archive response body failed"))? {
            if bytes.len().saturating_add(chunk.len()) > max {
                bail!("archive_size_limit: archive response exceeds the decoded transfer byte limit");
            }
            bytes.extend_from_slice(&chunk);
        }
        let received = Received { bytes, headers, status, retrieved_at: Utc::now().to_rfc3339() };
        if cache {
            if state.cache.len() >= CACHE_ENTRIES { state.cache.pop_front(); }
            state.cache.retain(|(k, _, _)| k != &key);
            state.cache.push_back((key, Instant::now(), received.clone()));
        }
        Ok((received, false))
    }
}

fn retry_after(value: Option<&str>) -> DateTime<Utc> {
    let now = Utc::now();
    if let Some(seconds) = value.and_then(|s| s.parse::<u64>().ok()) {
        return i64::try_from(seconds).ok().and_then(chrono::Duration::try_seconds)
            .and_then(|d| now.checked_add_signed(d)).unwrap_or(DateTime::<Utc>::MAX_UTC);
    }
    value.and_then(|s| DateTime::parse_from_rfc2822(s).ok()).map(|t| t.with_timezone(&Utc))
        .filter(|t| *t > now).unwrap_or(now + chrono::Duration::seconds(60))
}
fn timestamp(value: &str) -> Result<DateTime<Utc>> {
    if value.len() != 14 || !value.bytes().all(|b| b.is_ascii_digit()) {
        bail!("archive_invalid_request: use an exact UTC timestamp YYYYMMDDhhmmss");
    }
    let parsed = NaiveDateTime::parse_from_str(value, "%Y%m%d%H%M%S")
        .map_err(|_| anyhow!("archive_invalid_request: invalid calendar timestamp"))?;
    if parsed.format("%Y%m%d%H%M%S").to_string() != value {
        bail!("archive_invalid_request: invalid calendar timestamp");
    }
    Ok(parsed.and_utc())
}
fn original(value: &str) -> Result<Url> {
    if value.len() > 4096 || value.contains('*') || value.chars().any(char::is_control) {
        bail!("archive_invalid_request: use one HTTP(S) URL without wildcards or control characters");
    }
    let url = fetch::validated_url(value)
        .map_err(|_| anyhow!("archive_invalid_request: use an absolute HTTP(S) URL without credentials"))?;
    if matches!(url.host_str(), Some("web.archive.org")) {
        bail!("archive_invalid_request: supply the original URL, not a replay URL");
    }
    Ok(url)
}
fn replay_url(original: &str, timestamp: &str) -> String {
    format!("https://web.archive.org/web/{timestamp}id_/{original}")
}
fn index_url(url: &Url, from: &str, to: &str, count: usize) -> Result<Url> {
    let mut endpoint = Url::parse(INDEX_URL)?;
    endpoint.query_pairs_mut().extend_pairs([
        ("url", url.as_str()), ("matchType", "exact"), ("from", from), ("to", to),
        ("output", "json"), ("fl", "timestamp,original,statuscode,mimetype,digest"),
        ("limit", &format!("-{count}")),
    ]);
    Ok(endpoint)
}
fn captures(bytes: &[u8], requested: &Url, from: &str, to: &str, count: usize) -> Result<(Vec<ArchiveCapture>, bool, usize)> {
    let mut rows: Vec<Vec<String>> = serde_json::from_slice(bytes)
        .map_err(|_| anyhow!("archive_upstream_failed: invalid capture index JSON"))?;
    if rows.is_empty() { return Ok((vec![], false, 0)); }
    if rows.remove(0) != ["timestamp", "original", "statuscode", "mimetype", "digest"] {
        bail!("archive_upstream_failed: unexpected capture index columns");
    }
    if rows.len() > count + 1 {
        bail!("archive_upstream_failed: capture index ignored the result bound");
    }
    let more = rows.len() > count;
    let mut found = Vec::new();
    let mut excluded = 0;
    for row in rows {
        if row.len() != 5 { bail!("archive_upstream_failed: malformed capture index row"); }
        timestamp(&row[0]).map_err(|_| anyhow!("archive_upstream_failed: invalid capture timestamp"))?;
        let url = original(&row[1]).map_err(|_| anyhow!("archive_upstream_failed: invalid capture URL"))?;
        // CDX canonicalization can combine http/https or www variants. Do not
        // silently broaden the caller's URL or temporal policy to include them.
        if url != *requested || row[0].as_str() < from || row[0].as_str() > to {
            excluded += 1; continue;
        }
        let status = if row[2] == "-" { None } else {
            Some(row[2].parse::<u16>().ok().filter(|n| (100..=599).contains(n))
                .ok_or_else(|| anyhow!("archive_upstream_failed: invalid capture status"))?)
        };
        found.push(ArchiveCapture {
            original_url: url.to_string(), timestamp: row[0].clone(), replay_url: replay_url(url.as_str(), &row[0]),
            capture_status: status, media_type: (row[3] != "-").then(|| row[3].clone()),
            cdx_digest: (row[4] != "-").then(|| row[4].clone()),
        });
    }
    found.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    found.dedup_by(|a, b| a.timestamp == b.timestamp && a.original_url == b.original_url);
    found.truncate(count);
    Ok((found, more, excluded))
}
fn verify_replay(headers: &header::HeaderMap, capture: &ArchiveCapture) -> Result<String> {
    let memento = headers.get("memento-datetime").and_then(|v| v.to_str().ok())
        .and_then(|s| DateTime::parse_from_rfc2822(s).ok())
        .ok_or_else(|| anyhow!("archive_identity_mismatch: missing or invalid Memento-Datetime"))?;
    if memento.with_timezone(&Utc).format("%Y%m%d%H%M%S").to_string() != capture.timestamp {
        bail!("archive_identity_mismatch: replay capture differs from the selected timestamp");
    }
    // Validate the original link if supplied. A missing link does not establish
    // it independently; the exact CDX original and requested replay URL remain evidence.
    for value in headers.get_all(header::LINK) {
        let text = value.to_str().map_err(|_| anyhow!("archive_identity_mismatch: invalid replay Link header"))?;
        for link in text.split('<').skip(1) {
            if let Some((target, attrs)) = link.split_once('>') {
                if attrs.split(';').filter_map(|part| part.split_once('=')).any(|(key, value)|
                    key.trim().eq_ignore_ascii_case("rel") && value.trim().trim_end_matches(',').trim()
                        .trim_matches('"').split_ascii_whitespace().any(|rel| rel.eq_ignore_ascii_case("original"))) {
                    let target = target.trim().strip_prefix('<').unwrap_or(target.trim());
                    if original(target).ok().as_ref() != original(&capture.original_url).ok().as_ref() {
                        bail!("archive_identity_mismatch: replay original differs from the selected URL");
                    }
                }
            }
        }
    }
    Ok(memento.to_rfc3339())
}

impl Engine {
    pub async fn archive_lookup(&self, request: ArchiveLookupRequest) -> Result<ArchiveLookupResponse> {
        timeout(Duration::from_secs(self.config.request_timeout_seconds.min(30)), async {
            let _operation = self.operation_slots.acquire().await?;
            self.archive_lookup_inner(request).await
        }).await.map_err(|_| anyhow!("archive_timeout: archive lookup deadline reached"))?
    }
    async fn archive_lookup_inner(&self, request: ArchiveLookupRequest) -> Result<ArchiveLookupResponse> {
        let url = original(&request.url)?;
        let at = timestamp(&request.at)?;
        if request.within_days > 3660 || !(1..=3).contains(&request.limit) {
            bail!("archive_invalid_request: within_days must be 0 to 3660 and limit 1 to 3");
        }
        let earliest = at.checked_sub_signed(chrono::Duration::days(i64::from(request.within_days)))
            .filter(|d| d.format("%Y%m%d%H%M%S").to_string().len() == 14)
            .context("archive_invalid_request: lookback date is out of range")?
            .format("%Y%m%d%H%M%S").to_string();
        let endpoint = index_url(&url, &earliest, &request.at, request.limit + 1)?;
        let (received, cached) = self.archive.get(endpoint, INDEX_LIMIT.min(self.config.max_bytes), true, request.refresh).await?;
        let artifact = self.store.put_bytes(&received.bytes, "application/json", "wayback_cdx_index").await?;
        let (captures, has_more, excluded) = captures(&received.bytes, &url, &earliest, &request.at, request.limit)?;
        let mut warnings = vec![Warning::new("archive_index_only", "Capture index metadata is not fetched page evidence. This is a bounded, at-or-before selection, not a complete timeline or a publication date.")];
        if captures.is_empty() { warnings.push(Warning::new("archive_no_capture", "No matching capture was returned in this time window. Absence does not prove that the page was never archived.")); }
        if has_more { warnings.push(Warning::new("archive_candidates_limited", "The index contains more candidates. No extra request or broad capture crawl was made.")); }
        if excluded > 0 { warnings.push(Warning::new("archive_candidates_excluded", format!("Excluded {excluded} rows outside the exact URL or time policy. Scheme and www variants are not substituted."))); }
        Ok(ArchiveLookupResponse { provider: "wayback_cdx".into(), requested_url: url.to_string(), requested_at: request.at,
            earliest_at: earliest, retrieved_at: received.retrieved_at, captures, has_more, cached, index_artifact: artifact, warnings })
    }

    pub async fn archive_read(&self, request: ArchiveReadRequest) -> Result<ReadResponse> {
        timeout(Duration::from_secs(self.config.request_timeout_seconds.min(30)), async {
            let _operation = self.operation_slots.acquire().await?;
            self.archive_read_inner(request).await
        }).await.map_err(|_| anyhow!("archive_timeout: archive read deadline reached"))?
    }
    async fn archive_read_inner(&self, request: ArchiveReadRequest) -> Result<ReadResponse> {
        let url = original(&request.url)?;
        timestamp(&request.timestamp)?;
        if let Some(name) = &request.library { self.store.require_library(name).await?; }
        let key = hex::encode(Sha256::digest(serde_json::to_vec(&json!({"archive":VERSION,"url":url.as_str(),
            "timestamp":request.timestamp,"html_parser":readers::html::PARSER,"extraction":EXTRACTION_VERSION,"max_bytes":self.config.max_bytes}))?));
        // Shared archive serialization prevents parallel provider requests. The store
        // keeps accepted exact captures; lookup cache is separate and short lived.
        if !request.refresh {
            if let Some(document) = self.store.cached(&key, 86400).await? {
                if let Some(name) = &request.library { self.store.add(name, &document.id, request.actor).await?; }
                return Ok(ReadResponse { document, cached: true });
            }
        }
        let lookup = self.archive_lookup_inner(ArchiveLookupRequest { url: url.to_string(), at: request.timestamp.clone(),
            within_days: 0, limit: 1, refresh: request.refresh }).await?;
        let capture = lookup.captures.into_iter().find(|c| c.timestamp == request.timestamp)
            .context("archive_unavailable: the exact selected capture is absent from the bounded index response")?;
        if capture.capture_status.is_some_and(|status| status != 200) {
            bail!("archive_unavailable: the index reports an unsuccessful or redirect capture; no other capture was substituted");
        }
        let replay = Url::parse(&capture.replay_url)?;
        let (received, _) = self.archive.get(replay, self.config.max_bytes, false, true).await?;
        let memento = verify_replay(&received.headers, &capture)?;
        let content_type = received.headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(str::to_owned);
        let mime = readers::detect(url.as_str(), content_type.as_deref(), &received.bytes);
        let artifact = self.store.put_bytes(&received.bytes, &mime, "wayback_replay").await?;
        let mut parsed = match mime.as_str() {
            "text/html" | "application/xhtml+xml" => {
                let decoded = self.decode_html(received.bytes, mime.clone(), content_type, false).await?;
                let evidence = read_recovery::inspect(&decoded.text);
                if evidence.blocked.is_some() { bail!("archive_content_unavailable: captured content is an access or challenge page"); }
                let parsed = self.parse_html(decoded, url.to_string(), None).await?;
                if !read_recovery::usable(&parsed) || evidence.shell.is_some() {
                    bail!("archive_content_unavailable: no usable captured HTTP text; no browser or live fallback attempted");
                }
                parsed
            }
            "text/plain" | "text/markdown" => self.parse(received.bytes, url.to_string(), mime, None).await?,
            _ => bail!("archive_format_unsupported: this archive route supports HTML, XHTML, plain text, and Markdown only"),
        };
        if parsed.blocks.is_empty() { bail!("archive_content_unavailable: capture has no readable blocks"); }
        parsed.parser = format!("{}+{VERSION}", parsed.parser);
        parsed.metadata["archive"] = json!({"provider":"wayback","resolver":VERSION,"original_url":url.as_str(),
            "requested_capture":request.timestamp,"actual_capture":capture.timestamp,"capture":capture,
            "memento_datetime":memento,"replay_status":received.status,"index_retrieved_at":lookup.retrieved_at,
            "index_artifact":lookup.index_artifact,"replay_artifact":artifact,"mode":"id_","redirects_followed":0,
            "rights":"unknown; archive availability does not grant redistribution permission"});
        let source = Source { requested: capture.replay_url.clone(), resolved: capture.replay_url,
            retrieved_at: received.retrieved_at, status: Some(received.status), version: Some(format!("wayback:{}", request.timestamp)), original: artifact };
        let capture_status = capture.capture_status.map(|s| s.to_string()).unwrap_or_else(|| "unknown".into());
        let mut warnings = vec![Warning::new("historical_capture", format!("Historical Wayback capture {} of {}. Capture index HTTP {} and replay HTTP {} are separate. Not a live read or proof of publication date.", request.timestamp, url, capture_status, received.status)),
            Warning::new("archive_links_unverified", "Links resolve against the captured original URL. Linked pages and subresources were not fetched and are not verified historical evidence. An ordinary read of those links is a live request."),
            Warning::new("archive_replay_bytes", "The original export retains the received replay body after HTTP transfer decoding, not a WARC record or a guaranteed byte-identical publisher response. CDX digests and local SHA-256 have different meanings.")];
        if capture.capture_status.is_none() {
            warnings.push(Warning::new("archive_capture_status_unknown", "The capture index did not report original HTTP status, as can occur for revisit records. Replay HTTP 200 does not establish original status. Exact capture identity was checked separately."));
        }
        let document = self.finish(parsed, source, warnings).await?;
        self.store.cache(key, document.id.clone()).await?;
        if let Some(name) = request.library { self.store.add(&name, &document.id, request.actor).await?; }
        Ok(ReadResponse { document, cached: false })
    }
}
