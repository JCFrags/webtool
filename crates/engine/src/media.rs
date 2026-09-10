//! Only existing YouTube captions: no video, audio, translation, or transcription.
use std::{collections::BTreeSet, path::Path, time::Duration};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::process::Command;
use webtool_protocol::Warning;
use crate::{config::Config, fetch, readers::{self, Parsed}};

pub const PARSER: &str = "yt-dlp+native-captions/2";

/// Canonicalize only the supported single-video URLs. Ignore playlist/tracking parameters.
pub fn youtube_url(value: &str) -> Result<Option<String>> {
    let url = fetch::validated_url(value)?;
    let id = match url.host_str().unwrap_or("") {
        "youtu.be" | "www.youtu.be" => Some(url.path().trim_start_matches('/').to_owned()),
        "youtube.com" | "www.youtube.com" | "m.youtube.com" if url.path()=="/watch" =>
            Some(url.query_pairs().find(|(k,_)|k=="v").map(|(_,v)|v.into_owned()).unwrap_or_default()),
        _ => None,
    };
    let Some(id) = id else { return Ok(None); };
    if id.len()!=11 || !id.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-' || b==b'_') {
        bail!("unsupported YouTube video ID; use a watch or youtu.be URL for one video");
    }
    Ok(Some(format!("https://www.youtube.com/watch?v={id}")))
}
pub fn validate_language(language: &str) -> Result<()> {
    if language.is_empty() || language.len()>64 || !language.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'-' || b==b'_') {
        bail!("caption language must be a literal language code, such as en or en-US");
    }
    Ok(())
}
pub fn executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else { return false; };
    if !meta.is_file() { return false; }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o111 == 0 { return false; }
    }
    true
}
fn command(config: &Config) -> Result<Command> {
    let path = config.ytdlp_path.as_ref().filter(|p|executable(p))
        .context("media_helper_missing: yt-dlp is not configured as an executable; set ytdlp_path")?;
    let mut cmd = Command::new(path);
    cmd.args(["--ignore-config", "--no-plugin-dirs", "--no-remote-components", "--no-cache-dir",
        "--no-playlist", "--skip-download", "--ignore-no-formats-error", "--no-progress",
        "--color", "never", "--proxy", "", "--retries", "1", "--extractor-retries", "1",
        "--fragment-retries", "1", "--extractor-args", "youtube:skip=translated_subs"]);
    cmd.args(["--socket-timeout", &config.request_timeout_seconds.to_string(),
        "--max-filesize", &config.max_bytes.to_string()]);
    if let Some(runtime) = &config.ytdlp_js_runtime {
        cmd.args(["--no-js-runtimes", "--js-runtimes", runtime]);
    }
    // Also bound subtitle files when a source omits Content-Length. This limit is
    // inherited by helper children, independently of stdout/stderr limits.
    #[cfg(unix)] {
        use std::os::unix::process::CommandExt;
        let max = config.max_bytes as nix::sys::resource::rlim_t;
        unsafe {
            cmd.as_std_mut().pre_exec(move || {
                nix::sys::resource::setrlimit(nix::sys::resource::Resource::RLIMIT_FSIZE, max, max)
                    .map_err(std::io::Error::from)
            });
        }
    }
    Ok(cmd)
}
// Diagnostics must not persist signed subtitle URLs. Never print the metadata JSON.
fn diagnostic(value: &str) -> String {
    let urls = regex::Regex::new(r"https?://[^\s]+" ).expect("constant regex");
    urls.replace_all(value, "[source URL]").chars().take(4000).collect()
}
fn helper_error(error: anyhow::Error) -> anyhow::Error {
    let message = diagnostic(&format!("{error:#}"));
    let lower = message.to_ascii_lowercase();
    let code = if lower.contains("confirm you") || lower.contains("sign in") || lower.contains("403")
        || lower.contains("429") || lower.contains("not available in your country") {
        "media_source_blocked"
    } else if lower.contains("deadline") { "media_helper_timeout" }
    else { "media_helper_failed" };
    anyhow::anyhow!("{code}: {message}")
}
fn untranslated_vtt(track: &Value) -> bool {
    if track["ext"].as_str()!=Some("vtt") { return false; }
    track["url"].as_str().and_then(|s|url::Url::parse(s).ok())
        .is_some_and(|u|matches!(u.scheme(),"http"|"https") && !u.query_pairs().any(|(k,_)|k=="tlang"))
}
pub async fn read(url: &str, language: &str, config: &Config) -> Result<(Parsed,Vec<u8>,String)> {
    validate_language(language)?;
    let canonical = youtube_url(url)?.context("unsupported media URL; use a YouTube watch or youtu.be URL")?;
    // One deadline covers metadata, selection, track download, and parsing. On
    // cancellation helper process groups die and TempDir removes signed metadata/files.
    tokio::time::timeout(Duration::from_secs(config.helper_timeout_seconds), read_inner(&canonical,language,config))
        .await.context("media_helper_timeout: caption retrieval exceeded its deadline")?
}
async fn read_inner(url: &str, language: &str, config: &Config) -> Result<(Parsed,Vec<u8>,String)> {
    let temp = tempfile::tempdir().context("create caption temporary directory")?;
    let mut cmd = command(config)?;
    cmd.current_dir(temp.path()).args(["--dump-single-json", "--", url]);
    let output = fetch::helper_output(cmd, config.helper_timeout_seconds, config.max_bytes).await.map_err(helper_error)?;
    let mut metadata: Value = serde_json::from_slice(&output.stdout)
        .context("media_helper_failed: yt-dlp did not return metadata JSON")?;
    let mut warnings = Vec::new();
    if !output.stderr.is_empty() {
        warnings.push(Warning::new("yt_dlp_diagnostic", diagnostic(&String::from_utf8_lossy(&output.stderr))));
    }
    let mut available = BTreeSet::new();
    let mut choice = None;
    for (key,origin) in [("subtitles","provided"),("automatic_captions","automatic")] {
        if let Some(languages) = metadata[key].as_object() {
            for (lang,tracks) in languages {
                if let Some(track) = tracks.as_array().and_then(|v|v.iter().find(|v|untranslated_vtt(v))) {
                    available.insert(lang.clone());
                    if lang==language && choice.is_none() { choice=Some((key,origin,track.clone())); }
                }
            }
        }
    }
    let Some((key,origin,track)) = choice else {
        let diagnostics = warnings.iter().map(|w|w.message.as_str()).collect::<Vec<_>>().join("; ");
        bail!("media_captions_unavailable: no untranslated VTT track for {language}. Available untranslated VTT languages: {}. {diagnostics}",
            if available.is_empty(){"none".into()}else{available.into_iter().collect::<Vec<_>>().join(", ")});
    };
    let stable = json!({"video_id":metadata["id"], "title":metadata["title"], "media_url":url,
        "language":language, "track_origin":origin, "track_format":"vtt", "track_name":track["name"],
        "description":metadata["description"], "duration":metadata["duration"], "channel_id":metadata["channel_id"],
        "channel":metadata["channel"], "upload_date":metadata["upload_date"], "chapters":metadata["chapters"],
        "helper_version":metadata.pointer("/_version/version")});
    // Pass the existing source context (including per-track/top-level headers) to
    // yt-dlp's own downloader. Prune to exactly one track; never reselect a translated
    // language or run the generic HTTP client on the expiring subtitle URL.
    metadata["subtitles"] = json!({});
    metadata["automatic_captions"] = json!({});
    metadata[key] = json!({language:[track]});
    if let Some(object)=metadata.as_object_mut() { object.remove("requested_subtitles"); }
    let info = temp.path().join("source.json");
    tokio::fs::write(&info, serde_json::to_vec(&metadata)?).await?;
    let mut cmd = command(config)?;
    cmd.current_dir(temp.path()).args(["--load-info-json"]).arg(&info)
        .args(["--no-simulate", "--sub-format", "vtt", "--sub-langs", &format!("^{}$",regex::escape(language)),
            "--output", "caption.%(ext)s"])
        .arg(if origin=="provided" {"--write-subs"} else {"--write-auto-subs"});
    let downloaded = fetch::helper_output(cmd, config.helper_timeout_seconds, config.max_bytes).await.map_err(helper_error)?;
    if !downloaded.stderr.is_empty() {
        warnings.push(Warning::new("yt_dlp_diagnostic", diagnostic(&String::from_utf8_lossy(&downloaded.stderr))));
    }
    let file = temp.path().join(format!("caption.{language}.vtt"));
    let size = tokio::fs::metadata(&file).await
        .context("media_track_download_failed: yt-dlp did not create the selected caption file")?.len();
    if size > config.max_bytes as u64 { bail!("caption file exceeds configured byte limit"); }
    let bytes = tokio::fs::read(file).await.context("read downloaded caption file")?;
    let content = std::str::from_utf8(&bytes).context("media_captions_malformed: captions are not UTF-8")?;
    let mut parsed = readers::captions::parse(content,"captions.vtt")
        .context("media_captions_malformed: selected track is not valid timestamped captions")?;
    parsed.title=stable["title"].as_str().unwrap_or(url).into();
    parsed.parser=PARSER.into();
    parsed.metadata=stable;
    parsed.warnings.extend(warnings);
    if origin=="automatic" {
        parsed.warnings.push(Warning::new("automatic_captions","This track was generated automatically by YouTube, not by webtool."));
    }
    Ok((parsed,bytes,"text/vtt".into()))
}
