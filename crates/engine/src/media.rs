//! Media helpers never download a video or synthesize missing captions implicitly.
use anyhow::{bail,Context,Result};
use serde_json::Value;
use tokio::process::Command;
use webtool_protocol::Warning;
use crate::{config::Config,fetch,readers::{self,Parsed}};

pub async fn read(url:&str,language:&str,config:&Config,client:&reqwest::Client)->Result<(Parsed,Vec<u8>,String)>{
    fetch::validated_url(url)?;
    let executable=config.ytdlp_path.as_ref().context("yt-dlp is not configured. Set ytdlp_path in the server config.")?;
    let mut cmd=Command::new(executable);
    cmd.args(["--ignore-config","--no-playlist","--skip-download","--dump-single-json","--no-warnings","--",url]);
    let metadata_bytes=fetch::helper(cmd,config.helper_timeout_seconds,config.max_bytes).await?;
    let metadata:Value=serde_json::from_slice(&metadata_bytes).context("yt-dlp did not return metadata JSON")?;
    let title=metadata["title"].as_str().unwrap_or(url);
    let mut choice=None;
    for (key,origin) in [("subtitles","provided"),("automatic_captions","automatic")]{
        if let Some(tracks)=metadata[key][language].as_array(){
            if let Some(track)=tracks.iter().find(|v|v["ext"].as_str()==Some("vtt")){
                choice=Some((track.clone(),origin));break;
            }
        }
    }
    let Some((track,origin))=choice else{bail!("no VTT caption track for language {language}; no video was downloaded or transcript generated");};
    let track_url=track["url"].as_str().context("caption track has no URL")?;
    let fetched=fetch::http(client,track_url,config.max_bytes).await?;
    let content=std::str::from_utf8(&fetched.bytes).context("captions are not UTF-8")?;
    let mut parsed=readers::captions::parse(content,"captions.vtt")?;
    parsed.title=title.into();parsed.parser="yt-dlp+native-captions/1".into();
    // Track URLs often contain expiring tokens. Preserve stable media metadata, not those tokens.
    parsed.metadata=serde_json::json!({"video_id":metadata["id"],"media_url":url,"language":language,"track_origin":origin,
        "description":metadata["description"],"duration":metadata["duration"],"chapters":metadata["chapters"],"thumbnail":metadata["thumbnail"]});
    if origin=="automatic"{parsed.warnings.push(Warning::new("automatic_captions","This track was generated automatically by the source platform."));}
    Ok((parsed,fetched.bytes,"text/vtt".into()))
}
