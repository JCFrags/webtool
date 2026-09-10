use std::{collections::HashMap,process::Stdio,time::Duration};
use anyhow::{anyhow,bail,Context,Result};
use futures_util::StreamExt;
use tokio::{io::{AsyncRead,AsyncReadExt},process::Command};
use url::Url;
use webtool_protocol::{Renderer,Warning};
use crate::config::Config;

pub struct Fetched {
    pub bytes:Vec<u8>,pub resolved:String,pub content_type:Option<String>,
    pub status:Option<u16>,pub version:Option<String>,pub role:String,
    pub warnings:Vec<Warning>,
}
pub fn validated_url(value:&str)->Result<Url>{
    let mut u=Url::parse(value).context("invalid URL")?;
    if !matches!(u.scheme(),"http"|"https")||u.host_str().is_none(){bail!("only absolute HTTP and HTTPS URLs are accepted");}
    if !u.username().is_empty()||u.password().is_some(){bail!("credentials embedded in URLs are not supported");}
    u.set_fragment(None);Ok(u)
}
pub fn client(config:&Config)->Result<reqwest::Client>{
    Ok(reqwest::Client::builder().user_agent(&config.user_agent)
        .connect_timeout(Duration::from_secs(10)).timeout(Duration::from_secs(config.request_timeout_seconds))
        .redirect(reqwest::redirect::Policy::limited(10)).pool_max_idle_per_host(8).build()?)
}
pub async fn http(client:&reqwest::Client,url:&str,max:usize)->Result<Fetched>{
    let url=validated_url(url)?;
    let response=client.get(url).send().await.context("fetch source")?;
    let status=response.status().as_u16();
    if !response.status().is_success(){bail!("source returned HTTP {status}");}
    if response.content_length().is_some_and(|n|n>max as u64){bail!("source exceeds the {max}-byte limit");}
    let resolved=response.url().to_string();
    let content_type=response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).map(str::to_owned);
    let version=response.headers().get(reqwest::header::ETAG).and_then(|v|v.to_str().ok()).map(str::to_owned);
    let mut stream=response.bytes_stream();let mut bytes=Vec::new();
    while let Some(chunk)=stream.next().await{
        let chunk=chunk?;
        if bytes.len().saturating_add(chunk.len())>max{bail!("source exceeds the {max}-byte limit after decompression");}
        bytes.extend_from_slice(&chunk);
    }
    Ok(Fetched{bytes,resolved,content_type,status:Some(status),version,role:"http_response".into(),warnings:vec![]})
}

/// Child stdout, stderr, elapsed time, and process groups are all bounded.
pub async fn helper(mut command:Command,timeout:u64,max:usize)->Result<Vec<u8>>{
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(unix)]{
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    let mut child=command.spawn().context("launch configured helper")?;
    #[cfg(unix)]let _group=ProcessGroup(child.id());
    let stdout=child.stdout.take().context("helper stdout unavailable")?;
    let stderr=child.stderr.take().context("helper stderr unavailable")?;
    let outcome=tokio::time::timeout(Duration::from_secs(timeout),async{
        tokio::try_join!(read_limited(stdout,max),read_limited(stderr,1024*1024),async{Ok::<_,anyhow::Error>(child.wait().await?)})
    }).await;
    match outcome{
        Ok(Ok((out,err,status)))=>{
            if !status.success(){bail!("helper exited with {status}: {}",String::from_utf8_lossy(&err).chars().take(2000).collect::<String>());}
            Ok(out)
        },
        Ok(Err(e))=>{let _=child.kill().await;let _=child.wait().await;Err(e)},
        Err(_)=>{let _=child.kill().await;let _=child.wait().await;bail!("helper exceeded its {timeout}-second deadline")},
    }
}
async fn read_limited<R:AsyncRead+Unpin>(reader:R,max:usize)->Result<Vec<u8>>{
    let mut bytes=Vec::new();reader.take(max as u64+1).read_to_end(&mut bytes).await?;
    if bytes.len()>max{bail!("helper output exceeded {max} bytes");}Ok(bytes)
}
#[cfg(unix)]struct ProcessGroup(Option<u32>);
#[cfg(unix)]impl Drop for ProcessGroup{
    fn drop(&mut self){
        if let Some(id)=self.0{let _=nix::sys::signal::killpg(nix::unistd::Pid::from_raw(id as i32),nix::sys::signal::Signal::SIGKILL);}
    }
}

pub async fn browser(url:&str,renderer:&Renderer,config:&Config)->Result<Fetched>{
    validated_url(url)?;
    if matches!(renderer, Renderer::Crw) {
        #[cfg(feature="crw-browser")]
        { return crw(url, config).await; }
        #[cfg(not(feature="crw-browser"))]
        { bail!("fastCRW requires a server built with --features crw-browser"); }
    }
    let mut cmd=match renderer{
        Renderer::Lightpanda=>{
            let path=config.lightpanda_path.as_ref().context("Lightpanda is not configured. Set lightpanda_path in the server config.")?;
            let mut c=Command::new(path);
            c.env("LIGHTPANDA_DISABLE_TELEMETRY","true")
                .args(["fetch","--obey-robots","--dump","html","--wait-ms",&config.browser_wait_ms.to_string(),url]);c
        },
        Renderer::Chromium=>{
            let path=config.chromium_path.as_ref().context("Chromium is not configured. Set chromium_path in the server config.")?;
            let mut c=Command::new(path);
            c.args(["--headless=new","--disable-gpu","--no-first-run","--no-default-browser-check","--disable-background-networking","--dump-dom"]);
            if config.browser_no_sandbox{c.arg("--no-sandbox");}
            c.arg(format!("--virtual-time-budget={}",config.browser_wait_ms));c
        },
        Renderer::Http=>bail!("HTTP does not use a browser helper"),
        Renderer::Crw=>bail!("fastCRW adapter was not selected"),
    };
    let profile=tempfile::tempdir()?;
    if matches!(renderer,Renderer::Chromium){cmd.arg(format!("--user-data-dir={}",profile.path().display())).arg(url);}
    let bytes=helper(cmd,config.helper_timeout_seconds,config.max_bytes).await?;
    if bytes.is_empty(){bail!("browser produced an empty DOM");}
    Ok(Fetched{bytes,resolved:url.into(),content_type:Some("text/html".into()),status:None,version:None,role:"rendered_dom".into(),
        warnings:vec![Warning::new("browser_status_unavailable","CLI DOM capture does not report navigation status or final redirect URL. The saved URL is the requested URL."),
        Warning::new("browser_wait_budget","DOM capture used a fixed readiness budget. Late-loading content may be absent.")]})
}

#[cfg(feature="crw-browser")]
async fn crw(url:&str,config:&Config)->Result<Fetched>{
    use crw_core::{config::{RendererConfig,StealthConfig},Deadline};
    let value=config.crw_renderer.clone().context("missing crw_renderer config")?;
    let renderer_config:RendererConfig=serde_json::from_value(value)?;
    let r=crw_renderer::FallbackRenderer::new(&renderer_config,&config.user_agent,None,&StealthConfig::default())?;
    let deadline=Deadline::from_request_ms(config.helper_timeout_seconds*1000);
    let result=r.fetch(url,&HashMap::new(),Some(config.browser_wait_ms),None,None,deadline).await?;
    if result.html.len()>config.max_bytes{bail!("rendered page exceeds configured size limit");}
    if !(200..300).contains(&result.status_code){bail!("rendered source returned HTTP {}",result.status_code);}
    Ok(Fetched{bytes:result.html.into_bytes(),resolved:result.final_url.unwrap_or_else(||url.into()),
        content_type:Some("text/html".into()),status:Some(result.status_code),version:None,role:"rendered_dom".into(),
        warnings:vec![Warning::new("experimental_crw_adapter","This optional adapter has not been compiled or integration-tested in the delivery environment.")]})
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn rejects_local_files(){assert!(validated_url("file:///etc/passwd").is_err());}
    #[test]fn preserves_query_semantics(){assert_eq!(validated_url("https://example.com/a?x=2&x=1#part").unwrap().query(),Some("x=2&x=1"));}
    #[test]fn rejects_embedded_credentials(){assert!(validated_url("https://user:pass@example.com").is_err());}
    #[tokio::test]async fn output_limit_is_enforced(){assert!(read_limited(&b"123456"[..],3).await.is_err());}
    #[cfg(unix)]#[tokio::test]async fn helper_output_is_captured(){let mut c=Command::new("printf");c.arg("hello");assert_eq!(helper(c,2,100).await.unwrap(),b"hello");}
}
