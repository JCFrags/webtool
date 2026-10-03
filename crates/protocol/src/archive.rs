//! Explicit historical lookup and reads. Ordinary reads never fall back to archives.
use serde::{Deserialize, Serialize};
use crate::{Artifact, Warning};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveLookupRequest {
    pub url: String,
    /// UTC timestamp, exactly YYYYMMDDhhmmss. Only captures at or before this time.
    pub at: String,
    /// Inclusive lookback window, 0 to 3660 days. Zero selects the exact second.
    #[serde(default = "thirty")]
    pub within_days: u16,
    /// One to three candidates, newest first. No automatic pagination.
    #[serde(default = "three")]
    pub limit: usize,
    #[serde(default)]
    pub refresh: bool,
}
fn thirty() -> u16 { 30 }
fn three() -> usize { 3 }

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveCapture {
    pub original_url: String,
    pub timestamp: String,
    pub replay_url: String,
    /// Status reported by the capture index, not the replay HTTP response.
    pub capture_status: Option<u16>,
    pub media_type: Option<String>,
    /// Provider digest, not a hash of the locally retained replay bytes.
    pub cdx_digest: Option<String>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveLookupResponse {
    pub provider: String,
    pub requested_url: String,
    pub requested_at: String,
    pub earliest_at: String,
    pub retrieved_at: String,
    pub captures: Vec<ArchiveCapture>,
    /// The bounded index response contained more rows. Not a complete timeline.
    pub has_more: bool,
    pub cached: bool,
    pub index_artifact: Artifact,
    pub warnings: Vec<Warning>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchiveReadRequest {
    pub url: String,
    /// One explicitly selected UTC capture timestamp, exactly YYYYMMDDhhmmss.
    pub timestamp: String,
    #[serde(default)]
    pub refresh: bool,
    #[serde(default)]
    pub library: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
}
