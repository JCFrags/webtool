//! Explicit video discovery and supplied-caption operations. No media transfer.
use serde::{Deserialize, Serialize};
use crate::Warning;

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoSearchRequest {
    /// Literal provider query. No rewriting or per-result enrichment.
    pub query: String,
    #[serde(default = "five")]
    #[cfg_attr(feature = "openapi", schema(default = 5, minimum = 1, maximum = 20))]
    pub limit: usize,
}
fn five() -> usize { 5 }

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub video_id: String,
    pub url: String,
    /// Optional values are supplied by the helper. Null means unavailable, not zero.
    pub title: Option<String>,
    pub channel: Option<String>,
    pub channel_id: Option<String>,
    pub duration_seconds: Option<f64>,
    pub view_count: Option<u64>,
    /// Supplied YYYYMMDD value, if present. Not inferred from relative dates.
    pub upload_date: Option<String>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSearchResult {
    /// One-based position in the helper's flat results, before unsupported entries are omitted.
    pub provider_rank: usize,
    pub video: VideoMetadata,
    /// Discovery text, not fetched transcript evidence.
    pub description: Option<String>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoSearchResponse {
    pub query: String,
    pub provider: String,
    pub helper_version: Option<String>,
    pub retrieved_at: String,
    pub results: Vec<VideoSearchResult>,
    pub warnings: Vec<Warning>,
    pub elapsed_ms: u64,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptionOrigin { Provided, Automatic }
impl CaptionOrigin {
    pub fn as_str(self) -> &'static str {
        match self { Self::Provided => "provided", Self::Automatic => "automatic" }
    }
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptionChoice { #[default] ProvidedFirst, Provided, Automatic }

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionTracksRequest { pub url: String }

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionTrack {
    pub language: String,
    /// Provided does not prove human authorship. Automatic means provider-generated.
    pub origin: CaptionOrigin,
    pub name: Option<String>,
    pub format: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptionTracksResponse {
    pub video: VideoMetadata,
    pub provider: String,
    pub helper_version: Option<String>,
    pub retrieved_at: String,
    /// One selectable untranslated VTT track per exact language and origin. No signed URLs.
    pub tracks: Vec<CaptionTrack>,
    pub warnings: Vec<Warning>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionReadRequest {
    pub url: String,
    #[serde(default = "crate::default_language")]
    pub language: String,
    #[serde(default)]
    pub choice: CaptionChoice,
    #[serde(default)]
    pub refresh: bool,
    #[serde(default)]
    pub library: Option<String>,
}
