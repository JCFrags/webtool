//! One document engine. Normalization never invents page coordinates.
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use webtool_protocol::{Cell, Content, Locator, Warning};
use super::Parsed;
use crate::config::Config;

pub async fn parse(bytes: &[u8], name: &str, config: &Config) -> Result<Parsed> {
    #[cfg(feature = "documents")]
    {
        let mut cfg = if let Some(value) = &config.document_config {
            serde_json::from_value::<xberg::ExtractionConfig>(value.clone())
                .context("invalid Xberg document_config")?
        } else {
            xberg::ExtractionConfig {
                pages: Some(xberg::PageConfig {
                    extract_pages: true,
                    ..Default::default()
                }),
                output_format: xberg::OutputFormat::Plain,
                enable_quality_processing: false,
                ..Default::default()
            }
        };
        // The application owns caching. Avoid an independent extractor cache.
        cfg.use_cache = false;
        #[cfg(not(feature = "ocr"))]
        {
            if cfg.ocr.is_some() || cfg.force_ocr || cfg.force_ocr_pages.as_ref().is_some_and(|p|!p.is_empty()) {
                bail!("OCR is unavailable in this server build; document_config requires OCR");
            }
            cfg.disable_ocr = true;
        }
        let mime = super::detect(name, None, bytes);
        let input = xberg::ExtractInput::from_bytes(bytes.to_vec(), mime.as_str(), Some(name.into()));
        let output = xberg::extract(input, &cfg).await?;
        let errors = serde_json::to_value(&output.errors)?;
        let result = output.results.first().with_context(|| format!("document engine returned no results: {errors}"))?;
        let mut parsed = normalize(serde_json::to_value(result)?, name)
            .with_context(|| format!("normalize Xberg output; engine errors: {errors}"))?;
        // Keep every result and envelope diagnostic, not only the first normalized result.
        parsed.metadata["upstream_output"] = serde_json::to_value(&output)?;
        if errors.as_array().is_some_and(|items| !items.is_empty()) {
            parsed.warnings.push(Warning::new("document_engine_errors", errors.to_string()));
        }
        if output.results.len() > 1 {
            parsed.warnings.push(Warning::new("additional_document_results", "The engine returned multiple documents. Only the first result was normalized."));
        }
        Ok(parsed)
    }
    #[cfg(not(feature = "documents"))]
    {
        let _ = (bytes, name, config);
        bail!("binary documents require a server built with --features documents; scanned images additionally require OCR models")
    }
}

// This normalization layer can be tested without loading document models.
fn normalize(payload: Value, name: &str) -> Result<Parsed> {
    let mime = payload.get("mime_type").and_then(Value::as_str).unwrap_or("");
    let title = payload.pointer("/metadata/title").and_then(Value::as_str).unwrap_or(name);
    let mut parsed = Parsed::new(title, "xberg/1.1.1+source-blocks/2");
    let mut had_pages = false;
    if let Some(pages) = payload.get("pages").and_then(Value::as_array) {
        for page in pages {
            let text = page.get("content").and_then(Value::as_str).unwrap_or("");
            let number = positive_number(page.get("page_number"));
            if text.trim().is_empty() {
                let location = number.map(|n|format!("Page {n}")).unwrap_or_else(||"An unnumbered page".into());
                let status = if page.get("is_blank").and_then(Value::as_bool)==Some(true) {
                    "The engine marks it blank."
                } else { "This does not establish that it is blank or scanned." };
                parsed.warnings.push(Warning::new("document_page_no_text", format!("{location} has no extracted text. {status} {}", ocr_limit())));
                continue;
            }
            let locator = source_location(mime, number, page.get("sheet_name").and_then(Value::as_str), parsed.blocks.len());
            parsed.push(Content::Paragraph { text: text.into() }, locator);
            had_pages = true;
        }
    }
    if !had_pages {
        let text = payload.get("content").and_then(Value::as_str).unwrap_or("");
        if !text.trim().is_empty() {
            parsed.push(Content::Paragraph { text: text.into() }, Locator::Derived { index: 1 });
            parsed.warnings.push(Warning::new("document_location_unavailable", "The document engine supplied combined content without source pages. Generated line numbers are not source locations."));
        }
    }
    let mut supplemental_tables = Vec::new();
    if let Some(tables) = payload.get("tables").and_then(Value::as_array) {
        for table in tables {
            let Some(raw_rows) = table.get("cells").and_then(Value::as_array) else { continue; };
            let mut rows = Vec::new();
            for raw_row in raw_rows {
                let Some(raw_cells) = raw_row.as_array() else { bail!("document table row has an unexpected shape"); };
                let mut row = Vec::new();
                for cell in raw_cells {
                    let text = cell.as_str().context("document table cell is not text")?;
                    row.push(Cell { text: text.into(), row_span: 1, col_span: 1, header: false });
                }
                rows.push(row);
            }
            if !rows.iter().flatten().any(|cell| !cell.text.trim().is_empty()) { continue; }
            let locator = source_location(mime, positive_number(table.get("page_number")), None, parsed.blocks.len());
            parsed.push(Content::Table { rows }, locator);
            supplemental_tables.push(format!("b{}", parsed.blocks.len()));
        }
        if !supplemental_tables.is_empty() {
            parsed.warnings.push(Warning::new("document_tables_supplement", "Structured tables supplement the engine's text and may repeat table text. Cell spans and header roles are not inferred."));
        }
    }
    if let Some(items) = payload.get("processing_warnings").and_then(Value::as_array) {
        for item in items {
            parsed.warnings.push(Warning::new("document_engine_warning", item.to_string()));
        }
    }
    if parsed.blocks.is_empty() {
        let warnings = parsed.warnings.iter().map(|w|w.message.as_str()).collect::<Vec<_>>().join("; ");
        bail!("document contains no extracted text or nonempty table cells. Empty page objects are not readable content. {} Blank pages, scans, or extraction failures are possible; the cause is not established. {warnings}", ocr_limit());
    }
    parsed.warnings.push(Warning::new("document_structure_partial", "Page text and tables are normalized. Full upstream structures remain in metadata. Figure export and fine-grained document element mapping are not implemented."));
    if let Some(total) = positive_number(payload.pointer("/metadata/pages/total_count")) {
        if let Some(pages) = payload.get("pages").and_then(Value::as_array) {
            if pages.len() < total {
                parsed.warnings.push(Warning::new("document_pages_partial", format!("The engine reports {total} pages but returned {} page objects. See upstream metadata for extraction scope and warnings.", pages.len())));
            }
        }
    }
    parsed.metadata = json!({"upstream": payload, "supplemental_table_blocks": supplemental_tables});
    Ok(parsed)
}
fn ocr_limit() -> &'static str {
    if cfg!(feature="ocr") { "OCR support is compiled, but backend and model readiness are not verified." }
    else { "OCR is unavailable in this server build; image-only scans cannot be read." }
}
fn positive_number(value: Option<&Value>) -> Option<usize> {
    value.and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).filter(|n| *n > 0)
}
fn source_location(mime: &str, page: Option<usize>, sheet: Option<&str>, index: usize) -> Locator {
    if let Some(name) = sheet { return Locator::Sheet { name: name.into() }; }
    if let Some(number) = page {
        if mime == "application/pdf" { return Locator::Page { number, bbox: None }; }
        if mime.contains("presentation") || mime.contains("powerpoint") {
            return Locator::Slide { number };
        }
    }
    Locator::Derived { index: index + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn pdf_pages_keep_reported_numbers() {
        let p = normalize(json!({"mime_type":"application/pdf","pages":[{"page_number":4,"content":"Exact text"}]}), "paper.pdf").unwrap();
        assert_eq!(p.blocks[0].locator, Locator::Page { number:4, bbox:None });
    }
    #[test] fn generated_text_does_not_get_fake_source_lines() {
        let p = normalize(json!({"content":"Some text"}), "x.docx").unwrap();
        assert!(matches!(p.blocks[0].locator, Locator::Derived { .. }));
    }
    #[test] fn table_zero_and_empty_values_survive() {
        let p = normalize(json!({"mime_type":"application/pdf","tables":[{"page_number":2,"cells":[["0",""]]}]}), "x.pdf").unwrap();
        let Content::Table { rows } = &p.blocks[0].content else { panic!("expected table") };
        assert_eq!(rows[0][0].text,"0"); assert_eq!(rows[0][1].text,"");
        assert!(!rows[0][0].header);
    }
    #[test] fn sheets_are_not_pdf_pages() {
        let p = normalize(json!({"pages":[{"page_number":1,"sheet_name":"Actuals","content":"0"}]}), "x.xlsx").unwrap();
        assert_eq!(p.blocks[0].locator, Locator::Sheet { name:"Actuals".into() });
    }
    #[test] fn blank_result_is_explicit() { assert!(normalize(json!({"content":""}),"x.pdf").is_err()); }
}
