use std::{net::SocketAddr,path::PathBuf};
use anyhow::Result;
use clap::Parser;
use webtool_engine::{Engine,config::Config};

#[derive(Parser)]
#[command(name="webtoold",version,about="Shared web reading service for a trusted group")]
struct Args{
    #[arg(long)]config:Option<PathBuf>,
    #[arg(long)]bind:Option<SocketAddr>,
    #[arg(long)]data_dir:Option<PathBuf>,
}
#[tokio::main]
async fn main()->Result<()>{
    tracing_subscriber::fmt().with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_|"webtool=info,tower_http=info".into())).init();
    let args=Args::parse();let mut config=Config::load(args.config.as_deref())?;
    if let Some(bind)=args.bind{config.bind=bind;}
    if let Some(dir)=args.data_dir{config.data_dir=dir;}
    config.validate()?;
    if !config.bind.ip().is_loopback(){eprintln!("Warning: all connected users can read shared libraries. This server has no authentication and is intended for a trusted network.");}
    if config.browser_no_sandbox { eprintln!("Warning: browser_no_sandbox disables Chromium sandboxing. Do not use it for untrusted pages on an unisolated host."); }
    let engine=Engine::new(config.clone()).await?;
    let listener=tokio::net::TcpListener::bind(config.bind).await?;
    engine.recover_jobs().await?;
    eprintln!("webtoold {} listening on http://{}",env!("CARGO_PKG_VERSION"),listener.local_addr()?);
    let shutdown=engine.clone();
    axum::serve(listener,webtool_server::router(engine))
        .with_graceful_shutdown(async move{shutdown_signal().await;shutdown.stop_jobs().await;}).await?;
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());
        if let Ok(mut signal) = terminate {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = signal.recv() => {} }
        } else {
            let _ = tokio::signal::ctrl_c().await;
        }
    }
    #[cfg(not(unix))]
    { let _ = tokio::signal::ctrl_c().await; }
}
