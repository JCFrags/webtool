//! Offline citation of the saved selected PMC source, never a DOI re-fetch.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use webtool_protocol::{Document, ScholarlySnapshot};
use super::metadata::Metadata;
fn bib(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() { match c {
        '\\' => out.push_str("\\textbackslash{}"), '{' => out.push_str("\\{"), '}' => out.push_str("\\}"),
        '$' | '&' | '#' | '%' | '_' => { out.push('\\'); out.push(c); },
        '~' => out.push_str("\\textasciitilde{}"), '^' => out.push_str("\\textasciicircum{}"),
        c if c.is_whitespace() => out.push(' '), c => out.push(c),
    } } out
}
pub fn citation(document: &Document, format: &str) -> Result<Value> {
    let snapshot: ScholarlySnapshot = serde_json::from_value(document.metadata["scholarly"].clone()).context("citation_metadata_unavailable: invalid saved scholarly metadata")?;
    let meta: Metadata = serde_json::from_value(document.metadata["pmc"]["selection"].clone()).context("citation_metadata_unavailable: invalid saved PMC metadata")?;
    let record = snapshot.records.first().context("citation_metadata_unavailable: no selected PMC record")?;
    let version = meta.versioned_pmcid.as_ref().unwrap_or(&meta.pmcid);
    let landing = format!("https://pmc.ncbi.nlm.nih.gov/articles/{version}/");
    let issued = meta.citation_date_key.as_ref().and_then(|k| record.dates.get(k));
    let parts = issued.filter(|d| matches!(d.precision.as_str(), "year" | "month" | "day")).and_then(|d|
        d.value.split('-').map(str::parse::<u32>).collect::<std::result::Result<Vec<_>, _>>().ok());
    let note = format!("Saved PMC source {version}, instance {}, OAI datestamp {}. This route does not retrieve arbitrary historical versions or substitute a DOI record. Content state: {:?}.", meta.article_instance.as_deref().unwrap_or("not supplied"), meta.oai_datestamp, record.content_state);
    let text = match format {
        "csl" => {
            let authors = record.authors.iter().map(|a| {
                if let Some(literal) = &a.literal { json!({"literal":literal}) } else {
                    let mut v = json!({}); if let Some(given) = &a.given { v["given"] = json!(given); } if let Some(family) = &a.family { v["family"] = json!(family); } v
                }
            }).collect::<Vec<_>>();
            let mut v = json!({"id":record.id,"type":"article-journal","title":record.title,"author":authors,"URL":landing,"archive":"PubMed Central","archive_location":version,
                "version":meta.pmc_version,"note":note});
            if let Some(journal) = &meta.journal { v["container-title"] = json!(journal); }
            if let Some(volume) = &meta.volume { v["volume"] = json!(volume); }
            if let Some(issue) = &meta.issue { v["issue"] = json!(issue); }
            if let Some(pages) = &meta.pages { v["page"] = json!(pages); }
            if let Some(doi) = record.identifiers.get("doi") { v["DOI"] = json!(doi); }
            if let Some(parts) = parts { v["issued"] = json!({"date-parts":[parts]}); }
            serde_json::to_string_pretty(&v)?
        },
        "bibtex" => {
            let authors = record.authors.iter().map(|a| a.literal.clone().map(|n| format!("{{{}}}", bib(&n))).unwrap_or_else(|| {
                format!("{}, {}", bib(a.family.as_deref().unwrap_or("")), bib(a.given.as_deref().unwrap_or("")))
            })).collect::<Vec<_>>().join(" and ");
            let mut fields = vec![format!("title = {{{}}}", bib(&record.title)), format!("author = {{{authors}}}"), format!("url = {{{}}}", bib(&landing)), format!("note = {{{}}}", bib(&note))];
            for (key, value) in [("journal", meta.journal.as_ref()), ("volume", meta.volume.as_ref()), ("number", meta.issue.as_ref()), ("pages", meta.pages.as_ref()), ("version", meta.pmc_version.as_ref()), ("doi", record.identifiers.get("doi"))] {
                if let Some(value) = value { fields.push(format!("{key} = {{{}}}", bib(value))); }
            }
            if let Some(parts) = parts { if let Some(year) = parts.first() { fields.push(format!("year = {{{year}}}")); } }
            format!("@article{{pmc_{},\n  {}\n}}", version.replace('.', "_"), fields.join(",\n  "))
        },
        _ => bail!("citation_format_unsupported: saved PMC records support bibtex or csl"),
    };
    Ok(json!({"document_id":document.id,"format":format,"source":landing,"version":version,"metadata_source":"retained PMC OAI front matter","oai_datestamp":meta.oai_datestamp,"article_instance":meta.article_instance,"text":text}))
}
