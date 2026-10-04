use std::time::Instant;
use crate::error::ErrorKind;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::Value;
use webtool_protocol::*;
use crate::{config::Config, fetch};
use super::{canonical, command, helper_error, helper_warnings, metadata, selectable_tracks, video_id_valid};

pub(super) fn validate_search(request: &VideoSearchRequest) -> Result<()> {
    if request.query.trim().is_empty() || request.query.len() > 4096 || request.query.contains('\0') {
        bail!(ErrorKind::VideoInvalidQuery.context(format!("video_invalid_query: supply a nonempty literal query of at most 4096 bytes without NUL")));
    }
    if !(1..=20).contains(&request.limit) {
        bail!(ErrorKind::VideoInvalidLimit.context(format!("video_invalid_limit: video search limit must be between 1 and 20")));
    }
    Ok(())
}
fn text(value: &Value, key: &str) -> Option<String> {
    value[key].as_str().map(str::to_owned)
}
pub(super) fn video(value: &Value) -> Option<VideoMetadata> {
    let id = value["id"].as_str().filter(|id| video_id_valid(id))?;
    if value.get("entries").is_some()
        || value["ie_key"].as_str().is_some_and(|key| key != "Youtube") {
        return None;
    }
    let url = format!("https://www.youtube.com/watch?v={id}");
    // Never replace a mismatching supplied identity with a guessed watch URL.
    if let Some(supplied) = value["webpage_url"].as_str().or(value["url"].as_str()) {
        if supplied != id && canonical(supplied).ok().as_deref() != Some(url.as_str()) { return None; }
    }
    Some(VideoMetadata {
        video_id: id.into(), url, title: text(value, "title"), channel: text(value, "channel"),
        channel_id: text(value, "channel_id"),
        duration_seconds: value["duration"].as_f64().filter(|n| n.is_finite() && *n >= 0.0),
        view_count: value["view_count"].as_u64(), upload_date: text(value, "upload_date"),
    })
}
fn results(metadata: &Value, limit: usize) -> Result<(Vec<VideoSearchResult>, Vec<Warning>)> {
    let entries = metadata["entries"].as_array()
        .context(ErrorKind::MediaHelperFailed.context("media_helper_failed: video search did not return a flat entries array"))?;
    let mut results = Vec::new();
    let mut omitted = 0;
    for (index, entry) in entries.iter().take(limit).enumerate() {
        if let Some(video) = video(entry) {
            results.push(VideoSearchResult { provider_rank: index + 1, video, description: text(entry, "description") });
        } else { omitted += 1; }
    }
    let mut warnings = Vec::new();
    if omitted > 0 {
        warnings.push(Warning::new("video_results_omitted", format!("Omitted {omitted} unsupported or malformed entries. No replacement results were fetched.")));
    }
    if entries.len() > limit {
        warnings.push(Warning::new("video_results_truncated", "Helper returned more entries than requested; only the requested prefix was inspected."));
    }
    if results.is_empty() && !entries.is_empty() {
        bail!(ErrorKind::MediaHelperFailed.context(format!("media_helper_failed: video search returned no valid video identities")));
    }
    Ok((results, warnings))
}
pub(super) async fn search(request: VideoSearchRequest, config: &Config) -> Result<VideoSearchResponse> {
    let started = Instant::now();
    let temp = tempfile::tempdir().context("create video search temporary directory")?;
    let mut cmd = command(config)?;
    // The prefix and count are service-owned. The entire literal query is one argv item.
    cmd.current_dir(temp.path()).args(["--flat-playlist", "--dump-single-json", "--playlist-end"])
        .arg(request.limit.to_string()).arg("--").arg(format!("ytsearch{}:{}", request.limit, request.query));
    let output = fetch::helper_output(cmd, config.helper_timeout_seconds, config.max_bytes).await.map_err(helper_error)?;
    let metadata: Value = serde_json::from_slice(&output.stdout)
        .context(ErrorKind::MediaHelperFailed.context("media_helper_failed: video search did not return metadata JSON"))?;
    let (results, mut warnings) = results(&metadata, request.limit)?;
    warnings.extend(helper_warnings(&output.stderr));
    Ok(VideoSearchResponse {
        query: request.query, provider: "youtube".into(), helper_version: helper_version(&metadata),
        retrieved_at: Utc::now().to_rfc3339(), results, warnings, elapsed_ms: started.elapsed().as_millis() as u64,
    })
}
fn helper_version(metadata: &Value) -> Option<String> {
    metadata.pointer("/_version/version").and_then(Value::as_str).map(str::to_owned)
}
pub(super) async fn tracks(url: &str, config: &Config) -> Result<CaptionTracksResponse> {
    let temp = tempfile::tempdir().context("create caption inventory temporary directory")?;
    let (metadata, mut warnings) = metadata(url, config, temp.path()).await?;
    let video = video(&metadata).context(ErrorKind::MediaIdentityMismatch.context("media_identity_mismatch: helper returned inconsistent video metadata"))?;
    let tracks: Vec<_> = selectable_tracks(&metadata).into_iter().map(|(_, origin, language, track)| CaptionTrack {
        language: language.into(), origin, name: text(track, "name"), format: "vtt".into(),
    }).collect();
    if tracks.is_empty() {
        warnings.push(Warning::new("media_captions_unavailable", "No selectable untranslated VTT tracks were supplied. No transcription or translation was attempted."));
    }
    Ok(CaptionTracksResponse {
        video, provider: "youtube".into(), helper_version: helper_version(&metadata),
        retrieved_at: Utc::now().to_rfc3339(), tracks, warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn flat_results_keep_order_identity_and_unknown_values() {
        let metadata = json!({"entries":[
            {"id":"aaaaaaaaaaa","title":"first","duration":0,"view_count":0},
            {"id":"invalid"},
            {"id":"bbbbbbbbbbb","url":"https://youtu.be/bbbbbbbbbbb?list=ignored"},
            {"id":"ccccccccccc","url":"https://www.youtube.com/watch?v=ddddddddddd"}
        ]});
        let (items, warnings) = results(&metadata, 4).unwrap();
        assert_eq!(items.iter().map(|item| item.provider_rank).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(items[0].video.duration_seconds, Some(0.0));
        assert_eq!(items[0].video.view_count, Some(0));
        assert_eq!(items[1].video.title, None);
        assert_eq!(items[1].video.duration_seconds, None);
        assert_eq!(items[1].video.url, "https://www.youtube.com/watch?v=bbbbbbbbbbb");
        assert_eq!(warnings[0].code, "video_results_omitted");
        let request = VideoSearchRequest { query: "  literal: -- query  ".into(), limit: 5 };
        validate_search(&request).unwrap();
        assert_eq!(request.query, "  literal: -- query  ");
    }
}
