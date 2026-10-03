//! Explicit single-media transfers. Preview metadata is not a rights or availability guarantee.
use serde::{Deserialize, Deserializer, Serialize};
use crate::{Artifact, CrawlRequest, Warning};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum JobRequest { Crawl(CrawlRequest), Media(MediaDownloadRequest) }
impl<'de> Deserialize<'de> for JobRequest {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        // Never try legacy crawl decoding after a present, invalid discriminator.
        if value.get("kind").is_some() {
            serde_json::from_value(value).map(Self::Media).map_err(serde::de::Error::custom)
        } else {
            serde_json::from_value(value).map(Self::Crawl).map_err(serde::de::Error::custom)
        }
    }
}
impl JobRequest {
    pub fn crawl(&self) -> Option<&CrawlRequest> { if let Self::Crawl(r) = self { Some(r) } else { None } }
    pub fn crawl_mut(&mut self) -> Option<&mut CrawlRequest> { if let Self::Crawl(r) = self { Some(r) } else { None } }
    pub fn url(&self) -> &str { match self { Self::Crawl(r) => &r.url, Self::Media(r) => &r.url } }
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaJobKind { Media }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaFormatsRequest { pub url: String }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFormat {
    pub id: String,
    /// SHA-256 of stable selected identity fields, not a signed URL or authorization token.
    pub identity: String,
    pub container: String, pub protocol: String,
    pub video_codec: Option<String>, pub audio_codec: Option<String>,
    pub width: Option<u32>, pub height: Option<u32>, pub audio_language: Option<String>,
    pub bytes: Option<u64>, pub estimated_bytes: Option<u64>, pub selectable: bool,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFormatsResponse {
    pub video_id: String, pub url: String, pub title: Option<String>, pub duration_seconds: Option<f64>,
    pub observed_at: String, pub helper_version: Option<String>, pub formats: Vec<MediaFormat>, pub warnings: Vec<Warning>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatSelection { pub id: String, pub identity: String }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum MediaSelection {
    Video { video: FormatSelection, audio: Option<FormatSelection> },
    NativeAudio { audio: FormatSelection },
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaDownloadRequest {
    pub kind: MediaJobKind, pub url: String, pub video_id: String, pub selection: MediaSelection,
    /// Total retained source inputs plus final output, not a per-file limit.
    pub max_bytes: u64, pub max_duration_seconds: u64,
    pub max_width: Option<u32>, pub max_height: Option<u32>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaStage { Queued, Metadata, Transfer, Merge, Validation, Publication, Complete }
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaProgress {
    pub stage: MediaStage, pub transferred_bytes: u64, pub total_bytes: Option<u64>,
    pub speed_bytes_per_second: Option<f64>, pub eta_seconds: Option<f64>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaStream {
    pub kind: String, pub codec: String, pub width: Option<u32>, pub height: Option<u32>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaArtifact {
    /// Job-scoped identifier. The export route verifies membership, not only a hash.
    pub id: String, pub artifact: Artifact, pub container: String, pub duration_seconds: f64,
    pub streams: Vec<MediaStream>, pub derived_from: Vec<String>,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaResult {
    pub video_id: String, pub observed_at: String, pub helper_version: Option<String>,
    pub ffmpeg_version: String, pub ffprobe_version: String, pub selected_formats: Vec<MediaFormat>,
    pub artifacts: Vec<MediaArtifact>, pub output_id: String,
}
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaJob { pub progress: MediaProgress, pub result: Option<MediaResult> }

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn legacy_and_discriminator_are_not_ambiguous() {
        let old = serde_json::json!({"url":"https://example.invalid","max_pages":2});
        let job: JobRequest = serde_json::from_value(old).unwrap();
        assert_eq!(job.crawl().unwrap().max_pages, 2);
        assert!(serde_json::to_value(&job).unwrap().get("kind").is_none());
        for kind in [serde_json::json!("media"), serde_json::json!("crawl"), serde_json::Value::Null] {
            assert!(serde_json::from_value::<JobRequest>(serde_json::json!({"kind":kind,"url":"https://example.invalid"})).is_err());
        }
    }
}
