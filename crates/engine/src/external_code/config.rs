//! Credentials stay in the server environment, never in serializable configuration.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExternalCodeConfig {
    /// Whole operation, including provider queue and pacing. Also capped by request_timeout_seconds.
    pub timeout_ms: u64,
    /// Decoded response bytes per operation. Also capped by max_bytes.
    pub max_bytes: usize,
    pub sourcegraph: ExternalProviderConfig,
    pub context7: ExternalProviderConfig,
}
impl Default for ExternalCodeConfig {
    fn default() -> Self { Self { timeout_ms: 10_000, max_bytes: 1024 * 1024,
        sourcegraph: ExternalProviderConfig::default(), context7: ExternalProviderConfig::default() } }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExternalProviderConfig {
    /// Operator-selected base URL. No provider is selected by default.
    pub endpoint: Option<String>,
    /// Server environment reference only. Context7 requires it; Sourcegraph may be anonymous.
    pub credential_env: Option<String>,
    pub timeout_ms: u64,
    pub interval_ms: u64,
    /// Shared local rolling-minute ceiling, not a claim about the provider's allowance.
    pub max_requests_per_minute: usize,
}
impl Default for ExternalProviderConfig {
    fn default() -> Self { Self { endpoint: None, credential_env: None,
        timeout_ms: 6000, interval_ms: 1000, max_requests_per_minute: 20 } }
}
impl ExternalCodeConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        if !(500..=30_000).contains(&self.timeout_ms) || !(1024..=4*1024*1024).contains(&self.max_bytes) {
            bail!("external_code timeout_ms must be 500 to 30000 and max_bytes must be 1024 to 4194304");
        }
        for provider in [&self.sourcegraph, &self.context7] {
            if !(100..=20_000).contains(&provider.timeout_ms) || !(100..=60_000).contains(&provider.interval_ms)
                || !(1..=60).contains(&provider.max_requests_per_minute) {
                bail!("external_code provider timeout_ms must be 100 to 20000, interval_ms 100 to 60000, and max_requests_per_minute 1 to 60");
            }
            if let Some(value) = &provider.endpoint {
                endpoint(value).map_err(|_| anyhow::anyhow!("external_code endpoint must be HTTPS, or literal loopback HTTP, without credentials, query, or fragment"))?;
            }
            if let Some(name) = &provider.credential_env {
                if name.is_empty() || name.len() > 128 || !name.bytes().enumerate().all(|(i, b)|
                    b.is_ascii_alphabetic() || b == b'_' || (i > 0 && b.is_ascii_digit())) {
                    bail!("external_code credential_env must be an environment variable name, not a credential");
                }
            }
        }
        Ok(())
    }
}
pub(super) fn endpoint(value: &str) -> Result<Url, ()> {
    let url = Url::parse(value).map_err(|_| ())?;
    let loopback = match url.host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(), _ => false,
    };
    if value.len() > 2048 || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some()
        || url.query().is_some() || url.fragment().is_some()
        || !(url.scheme() == "https" || (url.scheme() == "http" && loopback)) { return Err(()); }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_defaults_and_secret_free_configuration() {
        let c = ExternalCodeConfig::default(); c.validate().unwrap();
        assert!(c.sourcegraph.endpoint.is_none() && c.context7.credential_env.is_none());
        assert!(endpoint("https://sourcegraph.example.org").is_ok());
        assert!(endpoint("http://127.0.0.1:4444/api").is_ok());
        assert!(endpoint("http://[::1]:4444").is_ok());
        for invalid in ["http://example.org", "http://localhost", "https://u:secret@example.org", "https://example.org/?key=secret", "https://example.org/#x", "ftp://example.org"] {
            assert!(endpoint(invalid).is_err());
        }
        assert!(toml::from_str::<ExternalCodeConfig>("[context7]\ncredential = 'synthetic'").is_err());
        let mut c = c; c.context7.credential_env = Some("1NOT_A_NAME".into()); assert!(c.validate().is_err());
    }
}
