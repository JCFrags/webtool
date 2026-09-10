use std::{net::SocketAddr, path::{Path, PathBuf}};
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
        for e in &self.search_engines {
            if !["duckduckgo", "brave", "startpage", "yahoo"].contains(&e.as_str()) {
                bail!("unsupported search engine: {e}");
            }
        }
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
}
