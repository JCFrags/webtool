use std::path::PathBuf;
use anyhow::{bail,Context,Result};
use clap::Args;
use tokio::io::AsyncWriteExt;
use webtool_protocol::*;
use crate::{Client,Output,human,output,stdout,warnings,job_output};

#[derive(Args)]
pub struct DownloadArgs {
    pub url:String,
    #[arg(long)]pub video_id:String,
    #[arg(long)]pub video_format:Option<String>,#[arg(long)]pub video_identity:Option<String>,
    #[arg(long)]pub audio_format:Option<String>,#[arg(long)]pub audio_identity:Option<String>,
    /// Retain one native source audio stream. No MP3 conversion or transcription.
    #[arg(long)]pub audio_only:bool,
    /// Total bytes for retained source inputs and final output, not bytes per file.
    #[arg(long)]pub max_bytes:u64,#[arg(long)]pub max_duration_seconds:u64,
    #[arg(long)]pub max_width:Option<u32>,#[arg(long)]pub max_height:Option<u32>,
    #[arg(long)]pub wait:bool,
}
pub async fn formats(client:&Client,url:String,format:Output)->Result<()> {
    let r:MediaFormatsResponse=client.post("/v1/video/formats",&MediaFormatsRequest {url}).await?;
    warnings(&r.warnings,true);
    if !human(format) {return output(&r,format);}
    let mut text=format!("Video {} | {}\nObserved: {} | duration: {} seconds\n",r.video_id,r.title.as_deref().unwrap_or("unknown title"),r.observed_at,r.duration_seconds.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()));
    for f in r.formats {
        text.push_str(&format!("Format {} | selectable={} | {} | video={} audio={} | {}x{} | bytes={} estimated={}\nIdentity: {}\n",f.id,f.selectable,f.container,
            f.video_codec.as_deref().unwrap_or("none/unknown"),f.audio_codec.as_deref().unwrap_or("none/unknown"),f.width.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()),f.height.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()),
            f.bytes.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()),f.estimated_bytes.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()),f.identity));
    }
    stdout(&render::terminal_safe(&text))
}
pub async fn download(client:&Client,args:DownloadArgs,format:Output)->Result<()> {
    let selection=|id:Option<String>,identity:Option<String>|->Result<FormatSelection> {Ok(FormatSelection {
        id:id.context("supply format ID from video formats")?,identity:identity.context("supply identity from video formats")?})};
    let audio=match (args.audio_format,args.audio_identity) {(None,None)=>None,(id,identity)=>Some(selection(id,identity)?)};
    let selected=if args.audio_only {
        if args.video_format.is_some() || args.video_identity.is_some() {bail!("--audio-only cannot select a video format");}
        MediaSelection::NativeAudio {audio:audio.context("--audio-only requires --audio-format and --audio-identity")?}
    } else {MediaSelection::Video {video:selection(args.video_format,args.video_identity)?,audio}};
    let job:Job=client.post("/v1/video/download",&MediaDownloadRequest {kind:MediaJobKind::Media,url:args.url,video_id:args.video_id,selection:selected,
        max_bytes:args.max_bytes,max_duration_seconds:args.max_duration_seconds,max_width:args.max_width,max_height:args.max_height}).await?;
    let job=if args.wait {client.wait(&job.id).await?} else {job};
    job_output(&job,format)?;
    if args.wait && job.state!=JobState::Complete {bail!("media job did not complete; inspect its saved error and warnings");}
    Ok(())
}
#[derive(Args)]
pub struct ExportArgs {
    pub job:String, pub artifact:String,
    /// Client-local destination. An existing file requires --force.
    #[arg(short,long)]pub output:PathBuf,#[arg(long)]pub force:bool,
}
pub async fn export(client:&Client,args:ExportArgs)->Result<()> {
    for value in [&args.job,&args.artifact] {
        if value.is_empty() || !value.bytes().all(|b|b.is_ascii_hexdigit() || b==b'-') {bail!("use an exact job and artifact identifier");}
    }
    let job:Job=client.get(&format!("/v1/jobs/{}",args.job)).await?;
    let a=job.media.and_then(|m|m.result).and_then(|r|r.artifacts.into_iter().find(|a|a.id==args.artifact)).context("artifact is not retained by this job")?;
    let response=client.http.get(format!("{}/v1/jobs/{}/artifacts/{}",client.base,args.job,args.artifact)).send().await.with_context(||client.connection_error())?;
    if !response.status().is_success() {let _:serde_json::Value=Client::decode(response).await?;bail!("artifact export failed");}
    let parent=args.output.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    if !args.force && args.output.exists() {bail!("destination exists; use --force only for intended replacement");}
    let temp=tempfile::NamedTempFile::new_in(parent)?;
    let mut file=tokio::fs::OpenOptions::new().write(true).open(temp.path()).await?;
    let mut response=response;let mut size=0u64;
    while let Some(chunk)=response.chunk().await? {
        size=size.checked_add(chunk.len() as u64).context("export size overflow")?;
        if size>a.artifact.size {bail!("export exceeds declared artifact size");}
        file.write_all(&chunk).await?;
    }
    if size!=a.artifact.size {bail!("export is incomplete");}
    file.sync_all().await?;drop(file);
    if args.force {temp.persist(&args.output)?;} else {temp.persist_noclobber(&args.output)?;}
    eprintln!("{}",render::terminal_safe(&format!("Exported {size} bytes to {}. Retained SHA-256: {}",args.output.display(),a.artifact.sha256)));
    Ok(())
}
