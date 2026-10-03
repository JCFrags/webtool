//! Explicit thin index commands. The CLI never accepts or resolves provider credentials.
use clap::{Subcommand, ValueEnum};
use webtool_protocol::*;
use crate::{document, human, output, stdout, warnings, Client, Output, Result};

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Report local provider configuration and shared cooldowns. No live probe.
    Status,
    /// Search an explicitly configured Sourcegraph instance. Snippets are not source evidence.
    Sourcegraph { #[command(subcommand)] action: SourcegraphCommand },
    /// Query the explicitly configured Context7 index with fast=true. Queries leave the server.
    Context7 { #[command(subcommand)] action: Context7Command },
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Mode { Literal, Path, Regexp, Symbol }
impl From<Mode> for SourcegraphMode {
    fn from(value: Mode) -> Self { match value { Mode::Literal => Self::Literal, Mode::Path => Self::Path, Mode::Regexp => Self::Regexp, Mode::Symbol => Self::Symbol } }
}
#[derive(Subcommand)]
pub(crate) enum SourcegraphCommand {
    Search {
        query: String,
        #[arg(long, value_enum, default_value="literal")] mode: Mode,
        #[arg(long="repo")] repository: Option<String>,
        #[arg(long="ref", requires="repository")] reference: Option<String>,
        #[arg(long)] path: Option<String>,
        #[arg(long)] language: Option<String>,
        #[arg(long, default_value_t=5)] limit: usize,
    },
    /// Verify saved hit lines against an ordinary GitHub code file and map at the same commit/blob.
    Verify { search_id: String, hit: usize, map_id: String, file_id: String },
}
#[derive(Subcommand)]
pub(crate) enum Context7Command {
    Libraries { library_name: String, query: String, #[arg(long, default_value_t=5)] limit: usize },
    Context {
        discovery_id: String, library_id: String, query: String,
        #[arg(long, conflicts_with="tracked", required_unless_present="tracked")] version: Option<String>,
        /// Explicit tracked index, not an installed/latest-release claim.
        #[arg(long, conflicts_with="version")] tracked: bool,
        #[arg(long, default_value_t=5)] limit: usize,
        #[arg(long)] details: bool,
    },
}
fn coverage(c: &ExternalIndexCoverage) {
    eprintln!("Index coverage: {} request(s), {} bytes, {} omitted result(s), incomplete: {}.", c.requests, c.response_bytes, c.omitted_results, c.incomplete);
    warnings(&c.warnings, true);
    if let Some(p) = &c.stopped { eprintln!("{}", render::terminal_safe(&format!("Stopped [{}]: {}", p.code, p.message))); }
}
pub(crate) async fn run(client: &Client, command: Command, format: Output) -> Result<()> {
    match command {
        Command::Status => {
            let result: ExternalProvidersResponse = client.get("/v1/external/providers").await?;
            if !human(format) { return output(&result, format); }
            for p in &result.providers { stdout(&render::terminal_safe(&format!("{:?}: endpoint configured {}, credential reference {}, locally ready {}, cooldown {} s, {}/{} requests in the shared minute.\n{}\n",
                p.provider, p.endpoint_configured, p.credential_configured, p.locally_ready, p.cooldown_seconds, p.requests_in_window, p.max_requests_per_minute, p.detail)))?; }
            Ok(())
        },
        Command::Sourcegraph { action: SourcegraphCommand::Search { query, mode, repository, reference, path, language, limit } } => {
            let result: SourcegraphSearchResponse = client.post("/v1/external/sourcegraph/search", &SourcegraphSearchRequest {
                query, mode: mode.into(), repository, reference, path, language, limit }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("Index search ID: {}\nStream done: {}. All hits remain unverified index snippets.\n", result.document_id, result.stream_done)))?;
            for (i, h) in result.hits.iter().enumerate() {
                stdout(&render::terminal_safe(&format!("Hit {}: {} {} | commit {}\n  {}\n  Index fetched: {}\n", i, h.repository, h.path, h.commit.as_deref().unwrap_or("unknown"), h.indexed_url, h.native["repoLastFetched"])))?;
                for line in &h.lines { stdout(&render::terminal_safe(&format!("  {}: {}\n", line.line_number_zero_based.saturating_add(1), line.text)))?; }
            }
            if !result.progress.is_empty() || !result.alerts.is_empty() {
                stdout(&render::terminal_safe(&format!("Provider progress/skips: {}\nProvider alerts: {}\n", serde_json::to_string(&result.progress)?, serde_json::to_string(&result.alerts)?)))?;
            }
            Ok(())
        },
        Command::Sourcegraph { action: SourcegraphCommand::Verify { search_id, hit, map_id, file_id } } => {
            let result: SourcegraphVerifyResponse = client.post("/v1/external/sourcegraph/verify", &SourcegraphVerifyRequest { search_id, hit, map_id, file_id }).await?;
            warnings(&result.warnings, true);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("Verified retained file: {} | {} | commit {} | blob {}\n", result.file_id, result.path, result.commit, result.blob_sha)))?;
            for line in &result.lines { stdout(&render::terminal_safe(&format!("{}: {} [file bytes {}..{}]\n", line.line, line.text, line.byte_range[0], line.byte_range[1])))?; }
            Ok(())
        },
        Command::Context7 { action: Context7Command::Libraries { library_name, query, limit } } => {
            let result: Context7LibrariesResponse = client.post("/v1/external/context7/libraries", &Context7LibrariesRequest { library_name, query, limit }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("Library discovery ID: {}\nProvider search filter applied: {}\n", result.document_id, result.search_filter_applied)))?;
            for library in &result.libraries { stdout(&render::terminal_safe(&format!("{} ({})\n  Branch: {} | index update: {} | state: {}\n  Versions: {}\n",
                library.title, library.id, library.native["branch"], library.native["lastUpdateDate"], library.native["state"], library.native["versions"])))?; }
            Ok(())
        },
        Command::Context7 { action: Context7Command::Context { discovery_id, library_id, query, version, tracked: _, limit, details } } => {
            let selection = match version { Some(version) => Context7VersionSelection::Listed { version }, None => Context7VersionSelection::Tracked };
            let result: Context7ContextResponse = client.post("/v1/external/context7/context", &Context7ContextRequest { discovery_id, library_id, selection, query, limit }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("Context7 requested index: {}\nThis is third-party index context, not revision-exact publisher text.\n", result.requested_library_id)))?;
            document(&result.document, format, details, false)
        },
    }
}
