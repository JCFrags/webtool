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
    /// Exact nearby lines in an already saved file. No provider request.
    Context { map_id: String, file_id: String, line: usize, #[arg(long, default_value_t=3)] before: usize, #[arg(long, default_value_t=8)] after: usize },
    /// Compare admitted saved maps. Provider patches require explicit opt-in.
    Compare { base_map_id: String, head_map_id: String, #[arg(long)] provider: bool, #[arg(long, default_value_t=1)] page: usize, #[arg(long, default_value_t=5)] limit: usize },
    /// Public issue, PR, and release snapshots. No credentials or asset downloads.
    Github { #[command(subcommand)] action: GithubCommand },
}
#[derive(Subcommand)]
pub(crate) enum GithubCommand {
    /// One discovery page. The GitHub issues list also includes PR metadata.
    List { repository: String, #[arg(long, value_enum)] kind: GithubKind, #[arg(long, value_enum, default_value="all")] state: State,
        #[arg(long, default_value_t=1)] page: usize, #[arg(long, default_value_t=5)] limit: usize },
    Issue { repository: String, number: u64, #[command(flatten)] comments: Comments, #[arg(long)] details: bool },
    Pr { repository: String, number: u64, #[command(flatten)] comments: Comments, #[arg(long)] details: bool },
    /// Read notes and asset metadata at one exact tag, not latest or asset bytes.
    Release { repository: String, tag: String, #[arg(long)] details: bool },
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum GithubKind { Issue, PullRequest, Release }
impl From<GithubKind> for GitHubKind {
    fn from(v: GithubKind) -> Self { match v { GithubKind::Issue => Self::Issue, GithubKind::PullRequest => Self::PullRequest, GithubKind::Release => Self::Release } }
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum State { Open, Closed, All }
impl From<State> for GitHubState { fn from(v: State) -> Self { match v { State::Open => Self::Open, State::Closed => Self::Closed, State::All => Self::All } } }
#[derive(Args)]
pub(crate) struct Comments {
    /// Fetch one issue-conversation comment page. PR reviews are not included.
    #[arg(long)] comments: bool,
    #[arg(long, default_value_t=1, requires="comments")] comment_page: usize,
    #[arg(long, default_value_t=5, requires="comments")] comment_limit: usize,
}
impl Comments { fn page(self) -> Option<GitHubPage> { self.comments.then_some(GitHubPage { page: self.comment_page, limit: self.comment_limit }) } }
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Mode { Paths, Literal, PathGlob, Regex, Symbols, GithubCode }
impl From<Mode> for CodeSearchMode {
    fn from(value: Mode) -> Self { match value { Mode::Paths => Self::Paths, Mode::Literal => Self::Literal, Mode::PathGlob => Self::PathGlob, Mode::Regex => Self::Regex, Mode::Symbols => Self::Symbols, Mode::GithubCode => Self::GithubCode } }
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
        CodeCommand::Context { map_id, file_id, line, before, after } => {
            let result: CodeContextResponse = client.post("/v1/code/context", &CodeContextRequest { map_id, file_id, line, before, after }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("{}:{}-{} | commit {} | blob {}\n{}\n{}", result.path, result.start_line, result.end_line,
                result.resolved_commit, result.blob_sha, result.url, result.text)))
        },
        CodeCommand::Compare { base_map_id, head_map_id, provider, page, limit } => {
            let result: CodeCompareResponse = client.post("/v1/code/compare", &CodeCompareRequest { base_map_id, head_map_id, provider, pagination: GitHubPage { page, limit } }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&render::terminal_safe(&format!("Comparison ID: {}\n{}: {} ({}) to {} ({})\n", result.document_id, result.repository,
                result.base_commit, result.base_requested_ref, result.head_commit, result.head_requested_ref)))?;
            for change in &result.changes { stdout(&render::terminal_safe(&format!("{} {}\n", change.status, change.path)))?; }
            if let Some(next) = result.next_page { eprintln!("Next provider commit page: {next}. No page was followed."); }
            if let Some(doc) = &result.provider_document { document(doc, format, false, false)?; }
            Ok(())
        },
        CodeCommand::Github { action } => github(client, action, format).await,
    }
}
async fn github(client: &Client, action: GithubCommand, format: Output) -> Result<()> {
    let (request, details) = match action {
        GithubCommand::List { repository, kind, state, page, limit } => {
            let result: GitHubListResponse = client.post("/v1/code/github/list", &GitHubListRequest { repository, kind: kind.into(), state: state.into(), pagination: GitHubPage { page, limit } }).await?;
            coverage(&result.coverage);
            if !human(format) { return output(&result, format); }
            stdout(&format!("Discovery ID: {}\nObserved: {} | page {}\n", result.document_id, result.observed_at, result.pagination.page))?;
            for item in &result.items { stdout(&render::terminal_safe(&format!("{} {}\n  {}\n", item["kind"].as_str().unwrap_or("unknown"),
                item["title"].as_str().or_else(|| item["name"].as_str()).or_else(|| item["tag_name"].as_str()).unwrap_or(""), item["html_url"].as_str().unwrap_or(""))))?; }
            if let Some(next) = result.next_page { eprintln!("Next discovery page: {next}. No page was followed."); }
            return Ok(());
        },
        GithubCommand::Issue { repository, number, comments, details } => (GitHubReadRequest { repository, kind: GitHubKind::Issue, number: Some(number), tag: None, comments: comments.page() }, details),
        GithubCommand::Pr { repository, number, comments, details } => (GitHubReadRequest { repository, kind: GitHubKind::PullRequest, number: Some(number), tag: None, comments: comments.page() }, details),
        GithubCommand::Release { repository, tag, details } => (GitHubReadRequest { repository, kind: GitHubKind::Release, number: None, tag: Some(tag), comments: None }, details),
    };
    let result: GitHubReadResponse = client.post("/v1/code/github/read", &request).await?;
    coverage(&result.coverage);
    if !human(format) { return output(&result, format); }
    document(&result.document, format, details, false)?;
    if let Some(comments) = &result.comments { document(comments, format, details, false)?; }
    if let Some(next) = result.next_comment_page { eprintln!("Next conversation comment page: {next}. No page was followed."); }
    Ok(())
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
