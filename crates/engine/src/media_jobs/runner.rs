//! Bounded streaming runner. No diagnostics or signed metadata enter saved jobs.
use std::{path::Path, process::Stdio, time::Duration};
use anyhow::{bail, Context, Result};
use tokio::{io::{AsyncRead, AsyncReadExt}, process::Command, sync::mpsc};
use tokio_util::sync::CancellationToken;
use webtool_protocol::Job;
use crate::{config::MediaDownloadConfig, Engine};
use super::workspace::{self, Workspace, META_BYTES};

struct Group(Option<u32>);
impl Drop for Group { fn drop(&mut self) { if let Some(pid)=self.0 {workspace::kill_group(pid);} } }
async fn pump(mut input: impl AsyncRead + Unpin, tx: mpsc::Sender<Result<Vec<u8>>>, max: usize, line_max: usize, emit: bool) -> Result<()> {
    let mut buffer=[0u8;4096]; let mut total=0usize; let mut line=Vec::new();
    loop {
        let n=input.read(&mut buffer).await?;
        if n==0 { if emit && !line.is_empty() { let _=tx.send(Ok(line)).await; } return Ok(()); }
        total=total.saturating_add(n);
        if total>max { bail!("media_output_limit: helper output exceeds its bound"); }
        for byte in &buffer[..n] {
            if *byte==b'\n' {
                if emit && tx.send(Ok(std::mem::take(&mut line))).await.is_err() { return Ok(()); }
                line.clear();
            } else {
                if line.len()>=line_max { bail!("media_output_limit: helper line exceeds its bound"); }
                line.push(*byte);
            }
        }
    }
}
pub fn command(path: &Path, max_bytes: u64) -> Command {
    let mut command=Command::new(path);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(unix)] {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
        unsafe { command.as_std_mut().pre_exec(move || {
            nix::sys::resource::setrlimit(nix::sys::resource::Resource::RLIMIT_FSIZE,max_bytes,max_bytes).map_err(std::io::Error::from)
        }); }
    }
    command
}
/// The caller must not wrap this future in a dropping timeout: this runner owns
/// group termination, direct-child wait/reap, and pipe-task joins on every return.
pub async fn run(mut command: Command, workspace: &mut Workspace, limits: &MediaDownloadConfig, max_bytes: u64,
    deadline: tokio::time::Instant, token: &CancellationToken, mut progress: Option<(&Engine, &mut Job, u64)>, collect: bool) -> Result<Vec<u8>> {
    if token.is_cancelled() { bail!("media_cancelled: cancellation requested"); }
    if tokio::time::Instant::now()>=deadline { bail!("media_deadline: job deadline reached"); }
    command.current_dir(&workspace.path);
    let mut child=command.spawn().context("media_helper_failed: cannot start configured helper")?;
    let pid=child.id().context("media_helper_failed: helper has no process identity")?;
    let mut group=Group(Some(pid));
    let (tx,mut rx)=mpsc::channel(8);
    let stdout=child.stdout.take().context("media_helper_failed: stdout unavailable")?;
    let stderr=child.stderr.take().context("media_helper_failed: stderr unavailable")?;
    let max=if collect { META_BYTES as usize } else { 512*1024 };
    let line_max=if collect { max } else { 4096 };
    let out=tokio::spawn({let tx=tx.clone(); async move {
        if pump(stdout,tx.clone(),max,line_max,true).await.is_err() { let _=tx.send(Err(anyhow::anyhow!("media_output_limit: stdout bound reached"))).await; }
    }});
    let err=tokio::spawn(async move {
        if pump(stderr,tx.clone(),1024*1024,8192,false).await.is_err() { let _=tx.send(Err(anyhow::anyhow!("media_output_limit: stderr bound reached"))).await; }
    });
    let result=async {
        workspace.child(pid)?;
        let mut value=Vec::new(); let mut status=None; let mut pipes=true;
        let mut interval=tokio::time::interval(Duration::from_millis(100));
        let mut last=tokio::time::Instant::now()-Duration::from_secs(1);
        let mut pending:Option<Vec<u8>>=None;
        while status.is_none() || pipes {
            tokio::select! {
                biased;
                _=token.cancelled()=>bail!("media_cancelled: cancellation requested"),
                _=tokio::time::sleep_until(deadline)=>bail!("media_deadline: job deadline reached"),
                _=interval.tick()=>{
                    if workspace.bytes()?>max_bytes.saturating_add(META_BYTES) || workspace::free_bytes(&workspace.path)?<limits.free_space_reserve_bytes {
                        bail!("media_budget_exceeded: sampled staging or free-space limit reached");
                    }
                    if last.elapsed()>=Duration::from_secs(1) {
                        if let (Some((engine,job,base)),Some(line))=(progress.as_mut(),pending.take()) {
                            super::update_transfer(engine,job,&line,*base).await?;
                        }
                        last=tokio::time::Instant::now();
                    }
                },
                result=child.wait(), if status.is_none()=>{
                    status=Some(result?);
                    // No helper may leave a background writer behind, even on success.
                    workspace::kill_group(pid);
                },
                item=rx.recv(), if pipes=>{
                    match item {
                        Some(line)=>{ let line=line?;
                            if collect { value.extend_from_slice(&line); value.push(b'\n'); }
                            else if line.starts_with(b"WEBTOOL\t") { pending=Some(line); }
                        }, None=>pipes=false,
                    }
                },
            }
        }
        if let (Some((engine,job,base)),Some(line))=(progress.as_mut(),pending) { super::update_transfer(engine,job,&line,*base).await?; }
        if !status.context("media_helper_failed: missing helper status")?.success() { bail!("media_helper_failed: configured helper failed; no retry or bypass"); }
        Ok(value)
    }.await;
    workspace::kill_group(pid);
    let reaped=child.wait().await;
    out.abort(); err.abort(); let _=out.await; let _=err.await;
    let stopped=workspace::stopped(pid).await;
    if stopped.is_ok() {group.0=None;}
    drop(group);
    reaped.context("media_helper_failed: cannot reap helper")?;
    stopped?;
    result
}
