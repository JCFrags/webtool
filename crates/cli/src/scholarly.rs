//! Thin scholarly client. The server owns provider clients, artifacts, and caches.
use clap::{Subcommand, ValueEnum};
use webtool_protocol::*;
use crate::{bail, human, output, render, stdout, warnings, Client, Output, Result};

#[derive(Clone, Copy, ValueEnum)]
pub enum Provider { Arxiv, Openalex }
impl From<Provider> for ScholarlyProvider {
    fn from(value: Provider) -> Self { match value { Provider::Arxiv => Self::Arxiv, Provider::Openalex => Self::Openalex } }
}
#[derive(Subcommand)]
pub enum Command {
    /// Discover metadata through one explicit provider. No destination paper is fetched.
    Search {
        #[arg(value_enum)] provider: Provider,
        #[arg(required = true, num_args = 1..)] query: Vec<String>,
        #[arg(long, default_value_t = 5)] limit: usize,
        #[arg(long)] refresh: bool,
    },
    /// Inspect one Crossref DOI record and its attributed update relations.
    Doi { doi: String, #[arg(long)] refresh: bool },
    /// Inspect one exact arXiv version. The default is metadata and license evidence only.
    Arxiv {
        id: String,
        /// Request the pinned PDF only when the selected version has supported reuse terms.
        #[arg(long)] full_text: bool,
        #[arg(long)] refresh: bool,
    },
    /// Inspect PMC OAI metadata. A .N suffix asserts the returned version, not historical retrieval.
    Pmc {
        /// Namespaced ID, such as pmc:PMC3974642.1.
        id: String,
        /// Request reusable JATS only after item-specific identity, currency, and rights checks.
        #[arg(long)] full_text: bool,
        #[arg(long)] refresh: bool,
        /// Assert the exact source OAI datestamp, not a publication or observation date.
        #[arg(long)] expected_datestamp: Option<String>,
    },
}
pub async fn run(client: &Client, command: Command, format: Output) -> Result<()> {
    let result: ScholarlyResponse = match command {
        Command::Search { provider, query, limit, refresh } => client.post("/v1/scholarly/search", &ScholarlySearchRequest { provider: provider.into(), query: query.join(" "), limit, refresh }).await?,
        Command::Doi { doi, refresh } => client.post("/v1/scholarly/doi", &ScholarlyDoiRequest { doi, refresh }).await?,
        Command::Arxiv { id, full_text, refresh } => client.post("/v1/scholarly/arxiv", &ScholarlyArxivRequest { id, full_text, refresh }).await?,
        Command::Pmc { id, full_text, refresh, expected_datestamp } => client.post("/v1/scholarly/pmc", &ScholarlyPmcRequest { id, full_text, refresh, expected_datestamp }).await?,
    };
    warnings(&result.warnings, true);
    if human(format) { stdout(&render::terminal_safe(&text(&result)))?; } else { output(&result, format)?; }
    if let Some(error) = &result.full_text_error { bail!("{}: {} Saved metadata ID: {}", error.code, error.message, result.document_id); }
    Ok(())
}
fn text(result: &ScholarlyResponse) -> String {
    let s = &result.snapshot;
    let mut out = format!("Provider: {}\nSaved ID: {}\nObserved: {} | age {} seconds{}\n{}\n\n", s.provider.name(), result.document_id, s.observed_at, result.age_seconds,
        if result.cached { " | cached" } else { "" }, if s.partial { "Partial result. See warnings and retained original." } else { "Metadata only unless a record explicitly says full_text_read." });
    for (index, record) in s.records.iter().enumerate() {
        out.push_str(&format!("{}. {}\n   Identity: {}\n   Content state: {:?}\n", index+1, record.title, record.id, record.content_state));
        for (name, value) in &record.identifiers { out.push_str(&format!("   {name}: {value}\n")); }
        let authors = record.authors.iter().map(|a| a.literal.clone().unwrap_or_else(|| [a.given.as_deref().unwrap_or(""), a.family.as_deref().unwrap_or("")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" "))).collect::<Vec<_>>().join("; ");
        out.push_str(&format!("   Authors (provider order): {authors}\n"));
        for (name, date) in &record.dates { out.push_str(&format!("   {name}: {} ({})\n", date.value, date.precision)); }
        for loc in &record.locations {
            out.push_str(&format!("   Location{}: {} | version {}\n   Rights: {:?} | license {}\n   {}\n", if loc.primary { " (primary)" } else { "" }, loc.landing_url.as_deref().unwrap_or("not supplied"), loc.version.as_deref().unwrap_or("not supplied"), loc.rights.decision, loc.rights.license.as_deref().unwrap_or("not supplied"), loc.rights.basis));
            if let Some(url) = &loc.pdf_url { out.push_str(&format!("   PDF link (not itself proof of permission): {url}\n")); }
        }
        out.push_str(&format!("   Status coverage: {}\n", record.status.coverage));
        for claim in &record.status.claims { out.push_str(&format!("   Status claim: {} {} | related {} | source {}{}\n", claim.field, claim.label,
            claim.related_id.as_deref().unwrap_or("not supplied"), claim.source, claim.date.as_ref().map(|d| format!(" | date {} ({})", d.value, d.precision)).unwrap_or_default())); }
        if let Some(abstract_text) = &record.abstract_text { out.push_str(&format!("\n   Provider abstract (not full text):\n{}\n", abstract_text)); }
        if matches!(record.content_state, ScholarlyContentState::FullTextRead) { out.push_str(&format!("\n   Read the saved paper body with: webtool read {}\n", result.document_id)); }
        out.push('\n');
    }
    if s.records.is_empty() { out.push_str("No results returned.\n"); }
    out
}
