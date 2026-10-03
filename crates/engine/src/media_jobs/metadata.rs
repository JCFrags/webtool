use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::process::Command;
use webtool_protocol::*;
use crate::config::Config;
use super::runner;

pub fn helper(config: &Config, max_bytes: u64) -> Result<Command> {
    super::limits(config)?;
    let mut cmd=runner::command(config.ytdlp_path.as_ref().expect("readiness checked"),max_bytes);
    cmd.args(["--ignore-config","--no-plugin-dirs","--no-remote-components","--no-cache-dir",
        "--no-playlist","--color","never","--proxy","","--retries","0","--extractor-retries","0",
        "--fragment-retries","0","--file-access-retries","0","--abort-on-error","--abort-on-unavailable-fragments",
        "--xff","never","--no-continue","--concurrent-fragments","1","--no-write-info-json","--no-write-subs",
        "--no-write-auto-subs","--no-write-thumbnail","--fixup","never"]);
    cmd.args(["--socket-timeout",&config.request_timeout_seconds.to_string(),"--max-filesize",&max_bytes.to_string()]);
    if let Some(runtime)=&config.ytdlp_js_runtime { cmd.args(["--no-js-runtimes","--js-runtimes",runtime]); }
    Ok(cmd)
}
pub fn token(s: &str) -> bool { !s.is_empty() && s.len()<=64 && s.bytes().all(|b|b.is_ascii_alphanumeric() || b"._-".contains(&b)) }
fn literal(value: &Value) -> Option<String> { value.as_str().filter(|s|token(s)).map(str::to_owned) }
pub fn codec(value: &Value) -> Option<String> { literal(value).filter(|s|s!="none") }
pub fn probe_codec(s: &str) -> &str {
    if s.starts_with("avc1") { "h264" } else if s.starts_with("av01") { "av1" }
    else if s.starts_with("vp09") { "vp9" } else if s.starts_with("mp4a") { "aac" }
    else if s.starts_with("hev1") || s.starts_with("hvc1") { "hevc" } else { s }
}
fn number(value: &Value) -> Option<u32> { value.as_u64().and_then(|n|u32::try_from(n).ok()).filter(|n|*n>0) }
pub fn normalize(raw: &Value, url: &str) -> Result<MediaFormatsResponse> {
    let id=url.rsplit('=').next().unwrap_or("");
    if raw["id"].as_str()!=Some(id) || raw["entries"].is_array()
        || raw["_type"].as_str().is_some_and(|s|s!="video") {
        bail!("media_identity_mismatch: expected one selected video");
    }
    // An explicit live/upcoming state must not be masked by is_live=false.
    let not_live=match raw["live_status"].as_str() {
        Some("not_live"|"was_live")=>raw["is_live"].as_bool()!=Some(true),
        None=>raw["is_live"].as_bool()==Some(false),
        _=>false,
    };
    if !not_live {
        bail!("media_format_unavailable: live, upcoming, and unknown live state are unsupported");
    }
    if let Some(candidate)=raw["webpage_url"].as_str() {
        if crate::media::youtube_url(candidate)?.as_deref()!=Some(url) { bail!("media_identity_mismatch: inconsistent selected URL"); }
    }
    let items=raw["formats"].as_array().context("media_format_unavailable: no supplied formats")?;
    if items.len()>512 { bail!("media_output_limit: format inventory exceeds 512 entries"); }
    let mut formats=Vec::new(); let mut seen=std::collections::HashSet::new();
    for item in items {
        let Some(id)=literal(&item["format_id"]) else { continue; };
        if !seen.insert(id.clone()) { bail!("media_identity_mismatch: duplicate format identities"); }
        let container=literal(&item["ext"]).unwrap_or_default();
        let protocol=literal(&item["protocol"]).unwrap_or_default();
        let mut f=MediaFormat {id,identity:String::new(),container,protocol,video_codec:codec(&item["vcodec"]),audio_codec:codec(&item["acodec"]),
            width:number(&item["width"]),height:number(&item["height"]),audio_language:literal(&item["language"]),
            bytes:item["filesize"].as_u64(),estimated_bytes:item["filesize_approx"].as_u64(),selectable:false};
        f.selectable=matches!(f.protocol.as_str(),"http"|"https") && matches!(f.container.as_str(),"mp4"|"m4a"|"webm"|"ogg"|"opus"|"mp3"|"flac"|"wav")
            && item["has_drm"].as_bool()!=Some(true)
            && (f.video_codec.is_some() || f.audio_codec.is_some())
            && f.video_codec.as_ref().is_none_or(|s|matches!(probe_codec(s),"h264"|"hevc"|"av1"|"vp8"|"vp9"))
            && f.audio_codec.as_ref().is_none_or(|s|matches!(probe_codec(s),"aac"|"opus"|"vorbis"|"mp3"|"flac"|"pcm_s16le"));
        f.identity=hex::encode(Sha256::digest(serde_json::to_vec(&json!({"id":f.id,"container":f.container,"protocol":f.protocol,
            "vcodec":f.video_codec,"acodec":f.audio_codec,"width":f.width,"height":f.height,"language":f.audio_language}))?));
        formats.push(f);
    }
    Ok(MediaFormatsResponse {video_id:id.into(),url:url.into(),title:raw["title"].as_str().map(|s|s.chars().take(512).collect()),
        duration_seconds:raw["duration"].as_f64().filter(|n|n.is_finite() && *n>0.0),observed_at:chrono::Utc::now().to_rfc3339(),
        helper_version:raw.pointer("/_version/version").and_then(Value::as_str).filter(|s|token(s)).map(str::to_owned),formats,
        warnings:vec![Warning::new("media_preview_observation","Formats are a source observation, not a guarantee of availability, access-method permission, or content rights. Only supported direct HTTP(S) native streams are selectable.")]})
}
pub fn selected(preview: &MediaFormatsResponse, request: &MediaDownloadRequest, config: &Config) -> Result<Vec<MediaFormat>> {
    let cap=super::limits(config)?;
    if preview.video_id!=request.video_id { bail!("media_identity_mismatch: selected video changed"); }
    if preview.duration_seconds.is_none_or(|n|n>request.max_duration_seconds as f64) { bail!("media_format_unavailable: unknown or excessive duration"); }
    let find=|s:&FormatSelection|->Result<MediaFormat> {
        preview.formats.iter().find(|f|f.id==s.id && f.identity==s.identity && f.selectable).cloned()
            .context("media_format_unavailable: selected format is absent, changed, or unsupported")
    };
    let formats=match &request.selection {
        MediaSelection::NativeAudio {audio}=>{
            let f=find(audio)?;
            if f.video_codec.is_some() || f.audio_codec.is_none() { bail!("media_format_unavailable: select one native audio-only stream"); }
            vec![f]
        },
        MediaSelection::Video {video,audio}=>{
            let f=find(video)?;
            if f.video_codec.is_none() || f.width.is_none_or(|n|n>request.max_width.unwrap_or(cap.max_width))
                || f.height.is_none_or(|n|n>request.max_height.unwrap_or(cap.max_height)) { bail!("media_format_unavailable: missing or excessive video dimensions"); }
            if let Some(audio)=audio {
                let a=find(audio)?;
                if f.audio_codec.is_some() || a.video_codec.is_some() || a.audio_codec.is_none() { bail!("media_format_unavailable: merge requires separate video-only and audio-only streams"); }
                vec![f,a]
            } else {
                if f.audio_codec.is_none() { bail!("media_format_unavailable: video requires supplied audio or an explicit audio stream"); }
                vec![f]
            }
        },
    };
    let known=formats.iter().try_fold(0u64,|sum,f|sum.checked_add(f.bytes.or(f.estimated_bytes).unwrap_or(0))).context("media_budget_exceeded: source sizes overflow")?;
    let retained=if formats.len()==2 { known.saturating_mul(2).saturating_add(65536) } else { known };
    if retained>request.max_bytes { bail!("media_budget_exceeded: supplied source sizes exceed total retained-byte budget"); }
    Ok(formats)
}
