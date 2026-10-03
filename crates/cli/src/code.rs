//! Explicit thin CLI commands. All retrieval and matching stay on the server.
use clap::{Args, Subcommand, ValueEnum};
use webtool_protocol::*;
use crate::{document, human, output, stdout, warnings, Client, Output, Result};

#[derive(Subcommand)]
pub(crate) enum CodeCommand {
    /// Discover public GitHub repositories. Descriptions are not fetched code.
    Discover { #[arg(required=true, num_args=1..)] query: Vec<String>, #[arg(long, default_value_t=5)] limit: usize },
    /// Save a bounded repository map at one explicit branch, tag, or commit.
    Map {
        repository: String,
        #[arg(long="ref")] reference: String,
        #[arg(long, default_value="")] path: String,
        #[arg(long, default_value_t=2)] depth: usize,
        #[arg(long, default_value_t=200)] max_entries: usize,
        #[arg(long, default_value_t=8)] max_requests: usize,
        #[arg(long, default_value_t=2097152)] max_bytes: usize,
    },
    /// Search saved map paths, or explicitly selected admitted files. No hidden fallback.
    Search {
        map_id: String, query: String,
        #[arg(long, value_enum)] mode: Mode,
        #[arg(long="path")] paths: Vec<String>,
        #[arg(long, default_value_t=5)] limit: usize,
        #[command(flatten)] limits: Limits,
    },
    /// Fetch one admitted regular UTF-8 file at the map's exact commit.
    File { map_id: String, path: String, #[command(flatten)] limits: Limits, #[arg(long)] details: bool },
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Mode { Paths, Literal, GithubCode }
impl From<Mode> for CodeSearchMode {
    fn from(value: Mode) -> Self { match value { Mode::Paths => Self::Paths, Mode::Literal => Self::Literal, Mode::GithubCode => Self::GithubCode } }
}
#[derive(Args)]
pub(crate) struct Limits {
    #[arg(long, default_value_t=2)] max_files: usize,
    #[arg(long, default_value_t=262144)] max_file_bytes: usize,
    #[arg(long, default_value_t=1048576)] max_bytes: usize,
    #[arg(long, default_value_t=2)] max_requests: usize,
}
impl From<Limits> for FileLimits {
    fn from(v: Limits) -> Self { Self { max_files: v.max_files, max_file_bytes: v.max_file_bytes, max_bytes: v.max_bytes, max_requests: v.max_requests } }
}
#[derive(Args)]
pub(crate) struct DocsArgs {
    crate_name: String,
    /// Exact release, not latest or a range.
    version: String,
    /// Page path after crate/version, or source file path when --source is selected.
    path: String,
    /// Read the published source HTML route, not a rustdoc page.
    #[arg(long)] source: bool,
    #[arg(long)] details: bool,
}
fn coverage(value: &CodeCoverage) {
    eprintln!("Coverage: {} requests, {} response bytes, {} files, {} file bytes; {}.", value.requests, value.response_bytes,
        value.files_read, value.file_bytes, if value.incomplete { "incomplete" } else { "complete within selected scope" });
    warnings(&value.warnings, true);
    if let Some(error) = &value.stopped { eprintln!("{}", render::terminal_safe(&format!("Stopped [{}]: {}", error.code, error.message))); }
}
pub(crate) async fn run(client: &Client, action: CodeCommand, format: Output) -> Result<()> {
    match action {
        CodeCommand::Discover { query, limit } => {
            let result: RepositoryDiscoverResponse = client.post("/v1/code/discover", &RepositoryDiscoverRequest { query: query.join(" "), limit }).await?;
            warnings(&result.warnings, true);
            if !human(format) { return output(&result, format); }
            stdout(&format!("Discovery ID: {}\nObserved: {} | incomplete: {}\n", result.document_id, result.observed_at, result.incomplete))?;
            for hit in &result.results { stdout(&render::terminal_safe(&format!("{}\n  {}\n  {}\n  Default branch: {}\n", hit.repository, hit.url, hit.description.as_deref().unwrap_or(""), hit.default_branch)))?; }
            Ok(())
        },
        CodeCommand::Map { repository, reference, path, depth, max_entries, max_requests, max_bytes } => {
            let result: RepositoryMapResponse = client.post("/v1/code/map", &RepositoryMapRequest { repository, reference, path, limits: MapLimits { depth, max_entries, max_requests, max_bytes } }).await?;
            coverage(&result.map.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("Map ID: {}\n{} | requested {} | commit {}\nScope: {} | depth {} | {} admitted entries\n",
                result.document_id, result.map.repository, result.map.requested_ref, result.map.resolved_commit,
                if result.map.path.is_empty() { "/" } else { &result.map.path }, result.map.limits.depth, result.map.entries.len())))?;
            for entry in &result.map.entries { stdout(&render::terminal_safe(&format!("{} {} {}\n", entry.mode, entry.kind, entry.path)))?; }
            Ok(())
        },
        CodeCommand::Search { map_id, query, mode, paths, limit, limits } => {
            let result: CodeSearchResponse = client.post("/v1/code/search", &CodeSearchRequest { map_id, query, mode: mode.into(), paths, limit, limits: limits.into() }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("{} | requested {} | commit {}\n", result.repository, result.requested_ref, result.resolved_commit)))?;
            for entry in &result.paths { stdout(&render::terminal_safe(&format!("{} {}\n  {}\n", entry.kind, entry.path, entry.url)))?; }
            for found in &result.matches { stdout(&render::terminal_safe(&format!("{}:{} | blob {} | document {}\n  {}\n{}\n\n", found.path, found.line, found.blob_sha, found.document_id, found.url, found.excerpt)))?; }
            for file in &result.files { if let Some(error) = &file.error { eprintln!("{}", render::terminal_safe(&format!("{} [{}]: {}", file.path, error.code, error.message))); } }
            if result.paths.is_empty() && result.matches.is_empty() { stdout("No matches in the reported scope.\n")?; }
            Ok(())
        },
        CodeCommand::File { map_id, path, limits, details } => {
            let result: CodeFileResponse = client.post("/v1/code/file", &CodeFileRequest { map_id, path, limits: limits.into() }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            let identity = &result.document.metadata["code"];
            stdout(&render::terminal_safe(&format!("Commit: {} | blob: {}\n", identity["resolved_commit"].as_str().unwrap_or("unknown"), identity["blob_sha"].as_str().unwrap_or("unknown"))))?;
            document(&result.document, format, details, false)
        },
    }
}
pub(crate) async fn docs(client: &Client, args: DocsArgs, format: Output) -> Result<()> {
    let result: DocumentationResponse = client.post("/v1/docs/read", &DocumentationRequest { crate_name: args.crate_name,
        version: args.version, path: args.path, kind: if args.source { DocumentationKind::Source } else { DocumentationKind::Page } }).await?;
    coverage(&result.coverage);
    if !human(format) { return output(&result, format); }
    let identity = &result.document.metadata["documentation"];
    stdout(&render::terminal_safe(&format!("docs.rs {} {} | {}\n", identity["crate_name"].as_str().unwrap_or("unknown"),
        identity["resolved_version"].as_str().unwrap_or("unknown"), result.document.source.resolved)))?;
    document(&result.document, format, args.details, false)
}
