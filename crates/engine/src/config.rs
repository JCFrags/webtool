use std::{collections::{BTreeMap, HashSet}, net::SocketAddr, path::{Path, PathBuf}};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub bind: SocketAddr,
    pub data_dir: PathBuf,
    pub max_bytes: usize,
    pub network_concurrency: usize,
    pub parse_concurrency: usize,
    pub job_concurrency: usize,
    pub crawl_concurrency: usize,
    pub browser_concurrency: usize,
    pub request_timeout_seconds: u64,
    pub helper_timeout_seconds: u64,
    pub cache_seconds: u64,
    pub user_agent: String,
    pub search_engines: Vec<String>,
    pub search: SearchConfig,
    pub lightpanda_path: Option<PathBuf>,
    pub chromium_path: Option<PathBuf>,
    pub ytdlp_path: Option<PathBuf>,
    /// yt-dlp runtime spec, e.g. node:/usr/bin/node. None uses yt-dlp defaults.
    pub ytdlp_js_runtime: Option<String>,
    pub browser_no_sandbox: bool,
    pub browser_wait_ms: u64,
    /// Used only by the optional fastCRW adapter. No fallback ladder is implicit.
    pub crw_renderer: Option<serde_json::Value>,
    /// Validated by Xberg when the feature is enabled.
    pub document_config: Option<serde_json::Value>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8420".parse().expect("constant socket address"),
            data_dir: "data".into(), max_bytes: 25 * 1024 * 1024,
            network_concurrency: 12, parse_concurrency: 2, job_concurrency: 2,
            crawl_concurrency: 4, browser_concurrency: 2,
            request_timeout_seconds: 30, helper_timeout_seconds: 90,
            cache_seconds: 3600, user_agent: "webtool/0.1 (+shared research reader)".into(),
            search_engines: vec!["duckduckgo".into(), "brave".into()],
            search: SearchConfig::default(),
            lightpanda_path: None, chromium_path: None, ytdlp_path: None, ytdlp_js_runtime: None,
            browser_no_sandbox: false, browser_wait_ms: 2000,
            crw_renderer: None, document_config: None,
        }
    }
}
impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config = match path {
            Some(p) => toml::from_str(&std::fs::read_to_string(p)
                .with_context(|| format!("read config {}", p.display()))?)?,
            None => Self::default(),
        };
        Self::validate(&config)?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("network_concurrency", self.network_concurrency),
            ("parse_concurrency", self.parse_concurrency),
            ("job_concurrency", self.job_concurrency),
            ("crawl_concurrency", self.crawl_concurrency),
            ("browser_concurrency", self.browser_concurrency),
        ] {
            if !(1..=64).contains(&value) { bail!("{name} must be between 1 and 64"); }
        }
        if !(1024..=128 * 1024 * 1024).contains(&self.max_bytes) {
            bail!("max_bytes must be between 1024 and 134217728");
        }
        if self.request_timeout_seconds == 0 || self.helper_timeout_seconds == 0 {
            bail!("timeouts must be positive");
        }
        if !self.user_agent.to_ascii_lowercase().starts_with("webtool/") {
            bail!("user_agent must start with webtool/ so crawler robots matching uses the advertised product token");
        }
        if self.search_engines.is_empty() { bail!("configure at least one search engine"); }
        let mut seen = HashSet::new();
        for e in &self.search_engines {
            if !SEARCH_ENGINES.contains(&e.as_str()) {
                bail!("unsupported search engine: {e}");
            }
            if !seen.insert(e) { bail!("duplicate search engine: {e}"); }
        }
        self.search.validate()?;
        Ok(())
    }
}

const SEARCH_ENGINES: &[&str] = &["duckduckgo", "brave", "startpage", "yahoo"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SearchConfig {
    /// Whole-search budget, including shared admission and pacing waits.
    pub timeout_ms: u64,
    /// Active provider requests across all web searches in this service.
    pub concurrency: usize,
    /// Also capped by the general max_bytes setting.
    pub max_bytes: usize,
    pub providers: BTreeMap<String, SearchProviderConfig>,
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self { timeout_ms: 8000, concurrency: 4, max_bytes: 4 * 1024 * 1024, providers: BTreeMap::new() }
    }
}
impl SearchConfig {
    fn validate(&self) -> Result<()> {
        if !(100..=60_000).contains(&self.timeout_ms) {
            bail!("search.timeout_ms must be between 100 and 60000");
        }
        if !(1..=16).contains(&self.concurrency) {
            bail!("search.concurrency must be between 1 and 16");
        }
        if !(1024..=32 * 1024 * 1024).contains(&self.max_bytes) {
            bail!("search.max_bytes must be between 1024 and 33554432");
        }
        for (name, provider) in &self.providers {
            if !SEARCH_ENGINES.contains(&name.as_str()) {
                bail!("unsupported search provider budget: {name}");
            }
            provider.validate().with_context(|| format!("search.providers.{name}"))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SearchProviderConfig {
    /// Request/body budget after admission. The whole-search budget still applies.
    pub timeout_ms: u64,
    pub concurrency: usize,
    /// Minimum time between request starts. Zero explicitly disables pacing.
    pub interval_ms: u64,
}
impl Default for SearchProviderConfig {
    fn default() -> Self {
        Self { timeout_ms: 6000, concurrency: 1, interval_ms: 1000 }
    }
}
impl SearchProviderConfig {
    fn validate(&self) -> Result<()> {
        // The pinned search client also has a 20-second socket ceiling.
        if !(100..=20_000).contains(&self.timeout_ms) {
            bail!("timeout_ms must be between 100 and 20000");
        }
        if !(1..=4).contains(&self.concurrency) {
            bail!("concurrency must be between 1 and 4");
        }
        if self.interval_ms > 60_000 { bail!("interval_ms must not exceed 60000"); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn default_is_valid() { Config::default().validate().unwrap(); }
    #[test] fn rejects_zero_workers() {
        let mut c = Config::default(); c.parse_concurrency = 0;
        assert!(c.validate().is_err());
    }
    #[test] fn rejects_unknown_configuration() {
        assert!(toml::from_str::<Config>("made_up = true").is_err());
    }
    #[test] fn search_defaults_support_existing_and_example_configurations() {
        let old: Config = toml::from_str("search_engines = ['duckduckgo', 'brave']").unwrap();
        old.validate().unwrap();
        assert_eq!(old.search.timeout_ms, 8000);
        assert!(old.search.providers.is_empty());
        let example: Config = toml::from_str(include_str!("../../../config.example.toml")).unwrap();
        example.validate().unwrap();
        let partial: Config = toml::from_str("[search.providers.brave]\ntimeout_ms = 2000").unwrap();
        partial.validate().unwrap();
        assert_eq!(partial.search.providers["brave"].interval_ms, 1000);
    }
    #[test] fn search_rejects_invalid_budgets_and_duplicate_providers() {
        for value in ["search_engines = ['brave', 'brave']", "[search]\ntimeout_ms = 0",
            "[search]\nconcurrency = 0", "[search]\nmax_bytes = 1",
            "[search.providers.unknown]", "[search.providers.brave]\ntimeout_ms = 20001",
            "[search.providers.brave]\nconcurrency = 0", "[search.providers.brave]\ninterval_ms = 60001"] {
            let config: Config = toml::from_str(value).unwrap();
            assert!(config.validate().is_err(), "{value}");
        }
        assert!(toml::from_str::<Config>("[search]\nunknown = true").is_err());
        assert!(toml::from_str::<Config>("[search.providers.brave]\nunknown = true").is_err());
    }
}
