//! Explicit opt-in single-media jobs, using the common scheduler and object store.
mod metadata;
mod runner;
mod workspace;
use std::{path::Path, time::Duration};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use webtool_protocol::*;
use crate::{config::{Config,MediaDownloadConfig}, Engine};
use workspace::{Workspace,META_BYTES};

pub fn limits(config: &Config) -> Result<&MediaDownloadConfig> {
    if !cfg!(target_os="linux") { bail!("media_platform_unsupported: Linux supervision is required"); }
    let c=config.media_download.as_ref().context("media_download_disabled: operator has not configured media budgets")?;
    for path in [config.ytdlp_path.as_deref(),Some(c.ffmpeg_path.as_path()),Some(c.ffprobe_path.as_path())] {
        if !path.is_some_and(|p|p.is_absolute() && crate::media::executable(p)) { bail!("media_download_helpers_missing: configure existing absolute yt-dlp, ffmpeg, and ffprobe executables"); }
    }
    Ok(c)
}
fn canonical(url: &str) -> Result<String> {
    if url.len()>8192 { bail!("media_download_invalid: oversized URL"); }
    crate::media::youtube_url(url)?.context("media_download_invalid: select one watch or youtu.be video URL")
}
fn validate(request: &MediaDownloadRequest, config: &Config) -> Result<()> {
    let c=limits(config)?; let url=canonical(&request.url)?;
    if url.rsplit('=').next()!=Some(request.video_id.as_str()) || request.max_bytes==0 || request.max_bytes>c.max_job_bytes
        || request.max_duration_seconds==0 || request.max_duration_seconds>c.max_duration_seconds
        || request.max_width.is_some_and(|n|n==0 || n>c.max_width) || request.max_height.is_some_and(|n|n==0 || n>c.max_height) {
        bail!("media_download_invalid: supply matching video identity and finite bounds within operator ceilings");
    }
    let ids=match &request.selection { MediaSelection::NativeAudio {audio}=>vec![audio],MediaSelection::Video {video,audio}=>std::iter::once(video).chain(audio.iter()).collect() };
    for f in ids {
        if !metadata::token(&f.id) || f.identity.len()!=64 || !f.identity.bytes().all(|b|b.is_ascii_hexdigit()) { bail!("media_download_invalid: select exact format ID and identity from a preview"); }
    }
    Ok(())
}
async fn budget(engine: &Engine, excluding: Option<String>, staging: u64, objects: u64) -> Result<()> {
    let c=limits(&engine.config)?;
    let (reserved_stage,reserved_objects)=engine.store.media_reservations(excluding).await?;
    let existing=workspace::directory_bytes(&engine.store.root().join("objects"),usize::MAX)?;
    let stage=reserved_stage.saturating_add(staging).saturating_add(workspace::staging_bytes(engine.store.root())?);
    let total=existing.saturating_add(stage).saturating_add(reserved_objects).saturating_add(objects);
    if stage>c.staging_bytes || total>c.storage_bytes
        || workspace::free_bytes(engine.store.root())?<c.free_space_reserve_bytes.saturating_add(stage).saturating_add(reserved_objects).saturating_add(objects) {
        bail!("media_budget_exceeded: storage reservation or free-space admission failed");
    }
    Ok(())
}
async fn inventory(engine: &Engine, url: &str, ws: &mut Workspace, deadline: tokio::time::Instant, token: &CancellationToken, max: u64) -> Result<(Value,MediaFormatsResponse)> {
    let mut cmd=metadata::helper(&engine.config,max)?;
    cmd.args(["--skip-download","--no-progress","--dump-single-json","--",url]);
    let output=runner::run(cmd,ws,limits(&engine.config)?,max,deadline,token,None,true).await?;
    let raw:Value=serde_json::from_slice(&output).context("media_helper_failed: invalid format metadata")?;
    let preview=metadata::normalize(&raw,url)?;
    Ok((raw,preview))
}
impl Engine {
    pub async fn media_formats(&self, request: MediaFormatsRequest) -> Result<MediaFormatsResponse> {
        let c=limits(&self.config)?; let url=canonical(&request.url)?;
        let deadline=tokio::time::Instant::now()+Duration::from_secs(c.timeout_seconds.min(self.config.helper_timeout_seconds));
        let _permit=tokio::time::timeout_at(deadline,self.media_job_slot.acquire()).await.context("media_deadline: preview admission deadline reached")??;
        let id=uuid::Uuid::new_v4().to_string(); let mut ws=Workspace::create(self.store.root(),&id)?;
        let result=inventory(self,&url,&mut ws,deadline,&CancellationToken::new(),c.max_job_bytes).await.map(|(_,p)|p);
        ws.clean()?;
        result
    }
    pub async fn submit_media(&self, mut request: MediaDownloadRequest) -> Result<Job> {
        validate(&request,&self.config)?; request.url=canonical(&request.url)?;
        let _submission=self.submission_lock.lock().await;
        self.require_job_capacity().await?;
        budget(self,None,request.max_bytes.saturating_add(META_BYTES),request.max_bytes).await?;
        let now=Utc::now().to_rfc3339();
        let job=Job {id:uuid::Uuid::new_v4().to_string(),state:JobState::Queued,request:JobRequest::Media(request),created_at:now.clone(),updated_at:now,
            document_ids:vec![],visited:0,failed:0,progress:None,media:Some(MediaJob {progress:MediaProgress {stage:MediaStage::Queued,transferred_bytes:0,total_bytes:None,speed_bytes_per_second:None,eta_seconds:None},result:None}),
            warnings:vec![Warning::new("media_access_and_rights","Source visibility and this request do not establish content rights or permission for the access method. No login, cookies, token plugins, denial bypass, ASR, conversion, playlist, or compatible resume is supported."),
                Warning::new("media_sampled_budget","Bytes count all retained inputs and output. Reservations cover staging and object copies. Per-file limits and 100 ms sampling can overshoot aggregate staging limits. This is not a hard filesystem quota.")],error:None};
        self.store.put_job(job.clone()).await?;
        self.schedule(job.clone()).await;
        Ok(job)
    }
    pub async fn media_artifact(&self, id: &str, artifact_id: &str) -> Result<MediaArtifact> {
        let job=self.store.job(id).await?;
        job.media.and_then(|m|m.result).and_then(|r|r.artifacts.into_iter().find(|a|a.id==artifact_id))
            .context("media_artifact_not_found: artifact is not retained by this job")
    }
    pub(crate) async fn recover_media(&self, job: &mut Job) -> Result<()> {
        if let Err(_e)=workspace::recover(self.store.root(),&job.id).await {
            job.warnings.push(Warning::new("media_staging_retained","Owned staging could not be safely verified or its writers have not stopped. It is retained for operator inspection, not compatible resume."));
        }
        job.error=Some("Media work was interrupted. No startup network request was made. Compatible resume is unsupported; submit a new preview-checked job. Accepted objects remain available.".into());
        job.updated_at=Utc::now().to_rfc3339(); self.store.put_job(job.clone()).await
    }
    pub(crate) async fn execute_media_job(&self, mut job: Job, token: CancellationToken) -> Result<()> {
        let JobRequest::Media(request)=job.request.clone() else { bail!("media_download_invalid: not a media request"); };
        let c=limits(&self.config)?;
        let elapsed=(Utc::now()-chrono::DateTime::parse_from_rfc3339(&job.created_at)?.with_timezone(&Utc)).num_seconds().max(0) as u64;
        let deadline=tokio::time::Instant::now()+Duration::from_secs(c.timeout_seconds.saturating_sub(elapsed));
        let permit=tokio::select! {biased; _=token.cancelled()=>None,_=tokio::time::sleep_until(deadline)=>None,p=self.media_job_slot.acquire()=>Some(p?)};
        let Some(_permit)=permit else { return finish_failure(self,&mut job,token.is_cancelled(),"media_deadline: admission expired").await; };
        job.state=JobState::Running; stage(self,&mut job,MediaStage::Metadata).await?;
        let mut ws=Workspace::create(self.store.root(),&job.id)?;
        let result=execute(self,&request,&mut job,&mut ws,deadline,&token).await;
        // The runner always kills/reaps/joins first. Never clean accepted objects.
        let cleanup=ws.clean();
        if cleanup.is_err() { job.warnings.push(Warning::new("media_staging_retained","Staging ownership or stopped-writer checks failed. Retained for operator inspection.")); }
        match result {
            Ok(())=>{ if cleanup.is_err() { self.store.put_job(job).await?; } Ok(()) },
            Err(e)=>finish_failure(self,&mut job,token.is_cancelled(),safe_error(&e)).await,
        }
    }
}
fn safe_error(error: &anyhow::Error) -> &str {
    for cause in error.chain() {
        let text=cause.to_string();
        for (prefix,message) in [("media_cancelled:","media_cancelled: cancellation requested"),("media_deadline:","media_deadline: operation deadline reached"),
            ("media_budget_exceeded:","media_budget_exceeded: byte or storage budget reached"),("media_format_unavailable:","media_format_unavailable: selection changed, is unsupported, or exceeds constraints"),
            ("media_identity_mismatch:","media_identity_mismatch: selected source identity changed"),("media_validation_failed:","media_validation_failed: actual output does not match selection"),
            ("media_output_limit:","media_output_limit: helper output bound reached"),("media_staging_unsafe:","media_staging_unsafe: ownership, file, or process checks failed")] {
            if text.starts_with(prefix) { return message; }
        }
    }
    "media_helper_failed: operation failed; no retry or bypass was attempted"
}
async fn finish_failure(engine: &Engine, job: &mut Job, cancelled: bool, message: &str) -> Result<()> {
    let _lock=engine.submission_lock.lock().await;
    job.state=if cancelled {JobState::Cancelled} else {JobState::Failed}; job.error=Some(message.into());job.updated_at=Utc::now().to_rfc3339();
    engine.store.put_job(job.clone()).await
}
async fn stage(engine: &Engine, job: &mut Job, stage: MediaStage) -> Result<()> {
    let p=&mut job.media.as_mut().context("media_download_invalid: no progress record")?.progress;
    p.stage=stage;p.speed_bytes_per_second=None;p.eta_seconds=None;
    job.updated_at=Utc::now().to_rfc3339();engine.store.put_job(job.clone()).await
}
pub(super) async fn update_transfer(engine: &Engine, job: &mut Job, line: &[u8], base: u64) -> Result<()> {
    let text=std::str::from_utf8(line).unwrap_or("");let values:Vec<_>=text.trim().split('\t').collect();
    if values.len()!=5 { return Ok(()); }
    let p=&mut job.media.as_mut().context("media_download_invalid: no progress record")?.progress;
    if let Ok(bytes)=values[1].parse::<u64>() { p.transferred_bytes=base.saturating_add(bytes); }
    // total_bytes describes all selected inputs, not one current stream.
    // It remains unknown if any selected stream lacks an exact source size.
    let number=|s:&str|s.parse::<f64>().ok().filter(|n|n.is_finite() && *n>=0.0);
    p.speed_bytes_per_second=number(values[3]);p.eta_seconds=number(values[4]);
    job.updated_at=Utc::now().to_rfc3339();engine.store.put_job(job.clone()).await
}

async fn version(engine: &Engine, ws: &mut Workspace, path: &Path, deadline: tokio::time::Instant, token: &CancellationToken, max: u64) -> Result<String> {
    let mut cmd=runner::command(path,max);cmd.arg("-version");
    let bytes=runner::run(cmd,ws,limits(&engine.config)?,max,deadline,token,None,true).await?;
    let line=std::str::from_utf8(&bytes)?.lines().next().unwrap_or("");
    let version=line.split_whitespace().nth(2).filter(|s|metadata::token(s)).context("media_helper_failed: helper version unavailable")?;
    Ok(version.into())
}
async fn execute(engine: &Engine, request: &MediaDownloadRequest, job: &mut Job, ws: &mut Workspace, deadline: tokio::time::Instant, token: &CancellationToken) -> Result<()> {
    validate(request,&engine.config)?;
    let c=limits(&engine.config)?;
    let ffmpeg_version=version(engine,ws,&c.ffmpeg_path,deadline,token,request.max_bytes).await?;
    let ffprobe_version=version(engine,ws,&c.ffprobe_path,deadline,token,request.max_bytes).await?;
    let (mut raw,preview)=inventory(engine,&request.url,ws,deadline,token,request.max_bytes).await?;
    let selected=metadata::selected(&preview,request,&engine.config)?;
    // Only source identity, headers and direct native-stream fields reach load-info-json.
    // Do not pass helper-produced filenames, postprocessors, or downloader options.
    let fields=["id","title","webpage_url","extractor","extractor_key","duration","is_live","live_status","http_headers"];
    let mut pruned=serde_json::Map::new();
    for key in fields {if let Some(value)=raw.get(key) {pruned.insert(key.into(),value.clone());}}
    let mut formats=Vec::new();
    for f in raw["formats"].as_array().context("media_format_unavailable: no formats")? {
        if !selected.iter().any(|s|Some(s.id.as_str())==f["format_id"].as_str()) {continue;}
        let source=f["url"].as_str().context("media_format_unavailable: direct stream URL missing")?;
        crate::fetch::validated_url(source)?;
        let mut object=serde_json::Map::new();
        for key in ["format_id","url","ext","protocol","vcodec","acodec","filesize","filesize_approx","width","height","tbr","abr","vbr","asr","audio_channels","language","http_headers"] {
            if let Some(value)=f.get(key) {object.insert(key.into(),value.clone());}
        }
        formats.push(Value::Object(object));
    }
    pruned.insert("formats".into(),Value::Array(formats));raw=Value::Object(pruned);
    let bytes=serde_json::to_vec(&raw)?;
    if bytes.len() as u64>META_BYTES { bail!("media_output_limit: selected private metadata exceeds limit"); }
    tokio::fs::write(ws.path.join("source.json"),bytes).await?;
    {let _lock=engine.submission_lock.lock().await;
     budget(engine,Some(job.id.clone()),request.max_bytes.saturating_add(META_BYTES),request.max_bytes).await?;}
    stage(engine,job,MediaStage::Transfer).await?;
    let mut names=Vec::new();let mut transferred=0u64;
    job.media.as_mut().expect("media dispatch checked").progress.total_bytes=selected.iter().try_fold(0u64,|n,f|n.checked_add(f.bytes?));
    for (i,f) in selected.iter().enumerate() {
        let name=format!("input-{i}.{}",f.container);
        let mut cmd=metadata::helper(&engine.config,request.max_bytes)?;
        cmd.args(["--load-info-json"]).arg(ws.path.join("source.json"));
        cmd.args(["--no-simulate","--format",&f.id,"--output",&name,"--newline","--progress","--progress-delta","1",
            "--progress-template","download:WEBTOOL\t%(progress.downloaded_bytes)s\t%(progress.total_bytes)s\t%(progress.speed)s\t%(progress.eta)s"]);
        runner::run(cmd,ws,c,request.max_bytes,deadline,token,Some((engine,job,transferred)),false).await?;
        transferred=transferred.saturating_add(std::fs::metadata(ws.regular(&name)?)?.len());
        job.media.as_mut().expect("media dispatch checked").progress.transferred_bytes=transferred;
        names.push(name);
    }
    tokio::fs::remove_file(ws.path.join("source.json")).await?;
    let output=if selected.len()==2 {
        stage(engine,job,MediaStage::Merge).await?;
        let name="output.mkv".to_owned();let mut cmd=runner::command(&c.ffmpeg_path,request.max_bytes);
        cmd.args(["-nostdin","-hide_banner","-loglevel","error","-n","-threads","1","-protocol_whitelist","file","-format_whitelist","matroska,webm,mov",
            "-i",&names[0],"-protocol_whitelist","file","-format_whitelist","matroska,webm,mov,ogg,mp3,flac,wav","-i",&names[1],
            "-map","0:v:0","-map","1:a:0","-c","copy","-map_metadata","-1","-f","matroska",&name]);
        runner::run(cmd,ws,c,request.max_bytes,deadline,token,None,false).await?;
        ws.regular(&name)?;name
    } else {names[0].clone()};
    stage(engine,job,MediaStage::Validation).await?;
    let mut properties=Vec::new();
    for (i,name) in names.iter().enumerate() {properties.push(probe(engine,ws,name,&selected[i..i+1],request,deadline,token).await?);}
    if selected.len()==2 {properties.push(probe(engine,ws,&output,&selected,request,deadline,token).await?);names.push(output.clone());}
    let actual=names.iter().try_fold(0u64,|n,name|->Result<u64>{Ok(n.checked_add(std::fs::metadata(ws.regular(name)?)?.len()).context("media_budget_exceeded: output size overflow")?)} )?;
    if actual>request.max_bytes {bail!("media_budget_exceeded: actual total retained inputs and output exceeds limit");}
    stage(engine,job,MediaStage::Publication).await?;
    // Cancellation and publication share submission_lock. A cancellation accepted
    // before this lock cannot produce a later complete result. Copies are streamed.
    let _lock=engine.submission_lock.lock().await;
    if token.is_cancelled(){bail!("media_cancelled: cancellation requested");}
    if tokio::time::Instant::now()>=deadline{bail!("media_deadline: publication deadline reached");}
    budget(engine,Some(job.id.clone()),actual,actual).await?;
    let mut artifacts=Vec::new();let mut inputs=Vec::new();
    for (i,name) in names.iter().enumerate() {
        let derived=i>=selected.len();
        let (container,duration,streams)=properties[i].clone();
        let artifact=engine.store.put_file(&ws.regular(name)?,mime(name),if derived{"media_derived_merge"}else{"media_fetched_stream"},request.max_bytes,deadline).await?;
        let id=artifact.sha256.clone();
        artifacts.push(MediaArtifact {id:id.clone(),artifact,container,duration_seconds:duration,streams,derived_from:if derived{inputs.clone()}else{vec![]}});
        if !derived {inputs.push(id);}
    }
    if tokio::time::Instant::now()>=deadline {bail!("media_deadline: publication deadline reached");}
    let output_id=artifacts.last().context("media_validation_failed: no output")?.id.clone();
    let media=job.media.as_mut().context("media_download_invalid: no media progress")?;
    media.result=Some(MediaResult {video_id:preview.video_id,observed_at:preview.observed_at,helper_version:preview.helper_version,
        ffmpeg_version,ffprobe_version,selected_formats:selected,artifacts,output_id});
    media.progress.stage=MediaStage::Complete;media.progress.speed_bytes_per_second=None;media.progress.eta_seconds=None;
    job.state=JobState::Complete;job.updated_at=Utc::now().to_rfc3339();engine.store.put_job(job.clone()).await
}
fn mime(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {"mkv"=>"video/x-matroska","webm"=>"video/webm","mp4"=>"video/mp4","m4a"=>"audio/mp4","opus"|"ogg"=>"audio/ogg","mp3"=>"audio/mpeg","flac"=>"audio/flac","wav"=>"audio/wav",_=>"application/octet-stream"}
}
async fn probe(engine: &Engine, ws: &mut Workspace, name: &str, selected: &[MediaFormat], request: &MediaDownloadRequest, deadline: tokio::time::Instant, token: &CancellationToken) -> Result<(String,f64,Vec<MediaStream>)> {
    let c=limits(&engine.config)?;ws.regular(name)?;
    let mut cmd=runner::command(&c.ffprobe_path,request.max_bytes);
    cmd.args(["-hide_banner","-v","error","-threads","1","-protocol_whitelist","file","-format_whitelist","matroska,webm,mov,ogg,mp3,flac,wav",
        "-probesize","4194304","-analyzeduration","5000000","-show_entries","format=format_name,duration:stream=codec_type,codec_name,width,height","-of","json",name]);
    let bytes=runner::run(cmd,ws,c,request.max_bytes,deadline,token,None,true).await?;
    let value:Value=serde_json::from_slice(&bytes)?;
    let duration=value["format"]["duration"].as_str().and_then(|s|s.parse::<f64>().ok()).filter(|n|n.is_finite() && *n>0.0 && *n<=request.max_duration_seconds as f64)
        .context("media_validation_failed: actual duration is unknown or excessive")?;
    let container=value["format"]["format_name"].as_str().filter(|s|s.len()<128).context("media_validation_failed: container unavailable")?.to_owned();
    let expected=if selected.len()==2 {"matroska"} else {match selected[0].container.as_str() {"mp4"|"m4a"=>"mov","webm"=>"matroska","opus"=>"ogg",s=>s}};
    if !container.split(',').any(|s|s==expected) {bail!("media_validation_failed: actual container differs from selection");}
    let mut streams=Vec::new();
    for s in value["streams"].as_array().context("media_validation_failed: streams unavailable")? {
        let kind=s["codec_type"].as_str().context("media_validation_failed: unknown stream")?;
        let codec=s["codec_name"].as_str().filter(|s|metadata::token(s)).context("media_validation_failed: unknown codec")?;
        if !matches!(kind,"video"|"audio") {bail!("media_validation_failed: unexpected stream kind");}
        let width=s["width"].as_u64().and_then(|n|u32::try_from(n).ok());let height=s["height"].as_u64().and_then(|n|u32::try_from(n).ok());
        let matches=selected.iter().any(|f|if kind=="audio" {f.audio_codec.as_ref().is_some_and(|c|metadata::probe_codec(c)==codec)}
            else {f.video_codec.as_ref().is_some_and(|c|metadata::probe_codec(c)==codec) && f.width==width && f.height==height
                && width.is_some_and(|n|n<=request.max_width.unwrap_or(c.max_width)) && height.is_some_and(|n|n<=request.max_height.unwrap_or(c.max_height))});
        if !matches {bail!("media_validation_failed: actual codec or dimensions differ from selection");}
        streams.push(MediaStream {kind:kind.into(),codec:codec.into(),width,height});
    }
    let audio=selected.iter().any(|s|s.audio_codec.is_some());let video=selected.iter().any(|s|s.video_codec.is_some());
    if streams.len()!=usize::from(audio)+usize::from(video) || streams.iter().any(|s|s.kind=="audio")!=audio || streams.iter().any(|s|s.kind=="video")!=video {
        bail!("media_validation_failed: required audio or video stream missing");
    }
    Ok((container,duration,streams))
}
