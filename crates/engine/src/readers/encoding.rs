//! Bounded HTML encoding selection. This is not the complete browser sniffing algorithm.
use crate::error::ErrorKind;
use anyhow::{bail, Context, Result};
use encoding_rs::{CoderResult, Encoding, REPLACEMENT, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252, X_USER_DEFINED};
use scraper::{ElementRef, Html, Selector};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use webtool_protocol::{Locator, Warning};

pub const VERSION: &str = "html-encoding/1";
pub const DEFAULT_LIMIT: usize = 25 * 1024 * 1024;
const META_LIMIT: usize = 1024;
const DECLARATION_LIMIT: usize = 16;

pub struct Decoded {
    pub text: String,
    pub metadata: Value,
    pub warnings: Vec<Warning>,
    pub had_errors: bool,
}
impl Decoded {
    pub fn attach(self, parsed: &mut super::Parsed) {
        if self.had_errors {
            // A selector still identifies an element, but replaced characters
            // cannot be described as exact source text.
            for (index, block) in parsed.blocks.iter_mut().enumerate() {
                block.locator = Locator::Derived { index: index + 1 };
            }
        }
        parsed.metadata["html_encoding"] = self.metadata;
        parsed.warnings.extend(self.warnings);
    }
}

struct Declaration {
    source: &'static str,
    label: String,
    encoding: Option<&'static Encoding>,
}
fn declaration(label: &str, source: &'static str, warnings: &mut Vec<Warning>) -> Declaration {
    let mut encoding = Encoding::for_label(label.as_bytes());
    if source == "html_meta" {
        let adjusted = match encoding {
            Some(e) if e == UTF_16LE || e == UTF_16BE => Some(UTF_8),
            Some(e) if e == X_USER_DEFINED => Some(WINDOWS_1252),
            e => e,
        };
        if adjusted != encoding {
            warnings.push(Warning::new("html_encoding_label_adjusted", "An HTML meta UTF-16 or x-user-defined label uses the HTML UTF-8 or windows-1252 mapping."));
            encoding = adjusted;
        }
    }
    if encoding.is_none() {
        warnings.push(Warning::new("html_encoding_unknown", format!("Unrecognized or empty {source} charset declaration; it was not used.")));
    }
    // Labels are untrusted. Bound retained diagnostic values, not just the scan.
    Declaration { source, label: label.chars().take(80).collect(), encoding }
}
fn transport(value: &str, source: &'static str, warnings: &mut Vec<Warning>) -> Vec<Declaration> {
    let parsed = if value.len() <= 8192 { value.parse::<mime::Mime>().ok() } else { None };
    let Some(parsed) = parsed else {
        warnings.push(Warning::new("html_encoding_malformed", format!("Malformed or oversized {source} media type; its charset was not used.")));
        return vec![];
    };
    let mut out = Vec::new();
    for (_, value) in parsed.params().filter(|(name, _)| *name == mime::CHARSET) {
        if out.len() == DECLARATION_LIMIT {
            warnings.push(Warning::new("html_encoding_declaration_limit", "Charset inspection stopped after 16 declarations."));
            break;
        }
        out.push(declaration(value.as_str(), source, warnings));
    }
    out
}
fn meta(bytes: &[u8], warnings: &mut Vec<Warning>) -> Vec<Declaration> {
    // Only ASCII-compatible declarations in the first 1024 source bytes are
    // eligible. Replacement here is only a bounded markup prescan, not content.
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(META_LIMIT)]);
    let dom = Html::parse_document(&prefix);
    let selector = Selector::parse("meta").expect("constant selector");
    let mut out = Vec::new();
    for element in dom.select(&selector) {
        if element.ancestors().filter_map(ElementRef::wrap)
            .any(|a| matches!(a.value().name(), "template" | "noscript" | "svg")) { continue; }
        let attributes = element.value();
        let declarations = if let Some(label) = attributes.attr("charset") {
            vec![declaration(label, "html_meta", warnings)]
        } else if attributes.attr("http-equiv").is_some_and(|v| v.trim().eq_ignore_ascii_case("content-type")) {
            match attributes.attr("content") {
                Some(value) => transport(value, "html_meta", warnings),
                None => {
                    warnings.push(Warning::new("html_encoding_malformed", "HTML content-type meta has no content attribute."));
                    vec![]
                }
            }
        } else { vec![] };
        out.extend(declarations);
        if out.len() >= DECLARATION_LIMIT {
            out.truncate(DECLARATION_LIMIT);
            warnings.push(Warning::new("html_encoding_declaration_limit", "HTML meta inspection stopped after 16 declarations."));
            break;
        }
    }
    out
}

/// Caller-sized chunks prevent an unbounded decoder allocation. The retained
/// UTF-8 result never exceeds `limit`; one fixed scratch buffer is additional.
fn bounded(bytes: &[u8], encoding: &'static Encoding, limit: usize) -> Result<(String, bool)> {
    let mut decoder = encoding.new_decoder_without_bom_handling();
    let mut output = String::with_capacity(bytes.len().min(limit));
    let mut scratch = [0u8; 8192];
    let mut read = 0;
    let mut had_errors = false;
    loop {
        // encoding_rs requires at least four output bytes, even at the cap.
        let size = (limit - output.len()).clamp(4, scratch.len());
        let (result, consumed, written, errors) = decoder.decode_to_utf8(&bytes[read..], &mut scratch[..size], true);
        if written > limit - output.len() { bail!(ErrorKind::HtmlInputSizeLimit.context(format!("html_decoded_size_limit: decoded HTML exceeds {limit} UTF-8 bytes"))); }
        if output.capacity() - output.len() < written { output.reserve_exact(written); }
        output.push_str(std::str::from_utf8(&scratch[..written]).expect("decoder returns UTF-8"));
        read += consumed;
        had_errors |= errors;
        if result == CoderResult::InputEmpty { return Ok((output, had_errors)); }
    }
}
fn xml_declaration(bytes: &[u8]) -> Result<Option<String>> {
    if !bytes.starts_with(b"<?xml") || !bytes.get(5).is_some_and(u8::is_ascii_whitespace) { return Ok(None); }
    let prefix = &bytes[..bytes.len().min(META_LIMIT)];
    let end = prefix.windows(2).position(|pair| pair == b"?>")
        .context(ErrorKind::HtmlEncodingUnsupported.context("xhtml_encoding_unsupported: XML declaration exceeds 1024 bytes or is incomplete"))?;
    let declaration = std::str::from_utf8(&prefix[..end + 2]).context(ErrorKind::HtmlEncodingUnsupported.context("xhtml_encoding_unsupported: XML declaration is not UTF-8"))?;
    let probe = format!("{declaration}<probe/>");
    roxmltree::Document::parse(&probe).context(ErrorKind::HtmlEncodingUnsupported.context("xhtml_encoding_unsupported: malformed XML declaration"))?;
    let pattern = regex::Regex::new(r#"encoding\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("constant pattern");
    Ok(pattern.captures(declaration).and_then(|c| c.get(1).or_else(|| c.get(2))).map(|v| v.as_str().to_owned()))
}

pub fn decode(bytes: &[u8], mime: &str, content_type: Option<&str>, rendered: bool, limit: usize) -> Result<Decoded> {
    if bytes.len() > limit { bail!(ErrorKind::HtmlInputSizeLimit.context(format!("html_input_size_limit: HTML input exceeds {limit} bytes"))); }
    if bytes.starts_with(b"\xff\xfe\0\0") || bytes.starts_with(b"\0\0\xfe\xff") {
        bail!(ErrorKind::HtmlEncodingUnsupported.context(format!("html_encoding_unsupported: UTF-32 is not supported")));
    }
    let bom = Encoding::for_bom(bytes);
    let mut warnings = Vec::new();
    let mut declarations = content_type.map(|v| transport(v, "http_charset", &mut warnings)).unwrap_or_default();
    let xhtml = mime == "application/xhtml+xml" && !rendered;
    if xhtml {
        // XHTML is XML, not HTML encoding sniffing. Deliberately support only
        // strict UTF-8 until XML encodings have a separate implementation.
        if !warnings.is_empty() || declarations.iter().any(|d| d.encoding != Some(UTF_8))
            || bom.is_some_and(|(e, _)| e != UTF_8) {
            bail!(ErrorKind::HtmlEncodingUnsupported.context(format!("xhtml_encoding_unsupported: XHTML requires an absent or UTF-8 transport charset and UTF-8 BOM")));
        }
        if let Some(label) = xml_declaration(&bytes[bom.map_or(0, |(_, n)| n)..])? {
            if !label.eq_ignore_ascii_case("utf-8") { bail!(ErrorKind::HtmlEncodingUnsupported.context(format!("xhtml_encoding_unsupported: only UTF-8 XML declarations are supported"))); }
            declarations.push(declaration(&label, "xml_declaration", &mut warnings));
        }
    } else {
        declarations.extend(meta(bytes, &mut warnings));
    }
    let (encoding, selected_by) = if rendered {
        if bom.is_some_and(|(e, _)| e != UTF_8) { bail!(ErrorKind::HtmlEncodingInvalid.context(format!("rendered_encoding_invalid: captured DOM is not UTF-8"))); }
        (UTF_8, "rendered_utf8")
    } else if xhtml {
        (UTF_8, "xhtml_utf8")
    } else if let Some((encoding, _)) = bom {
        (encoding, "bom")
    } else if let Some(d) = declarations.iter().find(|d| d.encoding.is_some()) {
        (d.encoding.unwrap(), d.source)
    } else if std::str::from_utf8(bytes).is_ok() {
        (UTF_8, "utf8_default")
    } else {
        warnings.push(Warning::new("html_encoding_fallback", "No recognized encoding declaration. Assumed windows-1252 because the HTML is not valid UTF-8; verify the retained original."));
        (WINDOWS_1252, "windows1252_fallback")
    };
    if encoding == REPLACEMENT { bail!(ErrorKind::HtmlEncodingUnsupported.context(format!("html_encoding_unsupported: selected charset maps to the replacement encoding; no fallback was attempted"))); }
    if declarations.iter().any(|d| d.encoding.is_some_and(|e| e != encoding)) {
        warnings.push(Warning::new("html_encoding_conflict", format!("Charset declarations disagree with selected {} ({selected_by}); lower-priority declarations were not used.", encoding.name())));
    }
    let removed = bom.map_or(0, |(_, n)| n);
    let (text, had_errors) = bounded(&bytes[removed..], encoding, limit)?;
    if had_errors && (rendered || xhtml) { bail!(ErrorKind::HtmlEncodingInvalid.context(format!("html_encoding_invalid: captured DOM and XHTML must be valid UTF-8; no replacement decoding was accepted"))); }
    if text.contains('\0') { bail!(ErrorKind::HtmlEncodingInvalid.context(format!("html_encoding_invalid: decoded HTML contains NUL characters; unsupported encoding or binary input"))); }
    if had_errors {
        warnings.push(Warning::new("html_encoding_replacements", "Malformed source sequences were replaced with U+FFFD. Text locations are derived, and automatic browser recovery is disabled for this input."));
    }
    let metadata = json!({
        "version": VERSION, "encoding": encoding.name(), "selected_by": selected_by,
        "bom": bom.map(|(e, _)| e.name()), "bom_bytes_removed": removed,
        "declarations": declarations.iter().map(|d| json!({"source": d.source, "label": d.label, "encoding": d.encoding.map(Encoding::name)})).collect::<Vec<_>>(),
        "meta_prescan_bytes": if xhtml { 0 } else { bytes.len().min(META_LIMIT) },
        "input_bytes": bytes.len(), "decoded_bytes": text.len(), "decoded_sha256": hex::encode(Sha256::digest(text.as_bytes())),
        "had_errors": had_errors, "decoded_byte_limit": limit,
        "locator_basis": "CSS selectors address the decoded retained artifact, not raw-byte offsets"
    });
    Ok(Decoded { text, metadata, warnings, had_errors })
}

/// Saved CSS operations replay the stored encoding, never today's header/meta
/// rules. Legacy snapshots without a decision retain their strict UTF-8 view.
pub fn restore(bytes: &[u8], record: Option<&Value>, limit: usize) -> Result<String> {
    if bytes.len() > limit { bail!(ErrorKind::HtmlInputSizeLimit.context(format!("html_input_size_limit: retained HTML exceeds {limit} bytes"))); }
    let Some(record) = record else { return Ok(std::str::from_utf8(bytes).context("legacy HTML original is not UTF-8")?.to_owned()); };
    if record["version"] != VERSION { bail!("unsupported saved HTML encoding version"); }
    let encoding = record["encoding"].as_str().and_then(|label| Encoding::for_label(label.as_bytes())).context("invalid saved HTML encoding")?;
    let removed = record["bom_bytes_removed"].as_u64().context("missing saved BOM length")? as usize;
    let input = bytes.get(removed..).context("invalid saved BOM length")?;
    let (text, _) = bounded(input, encoding, limit)?;
    if record["decoded_sha256"] != hex::encode(Sha256::digest(text.as_bytes())) { bail!("saved HTML decoding checksum does not match retained original"); }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn html(bytes: &[u8], header: Option<&str>) -> Decoded { decode(bytes, "text/html", header, false, 4096).unwrap() }
    #[test]
    fn declarations_and_bom_have_explicit_precedence() {
        let bytes = b"<meta charset=utf-8><p>caf\xe9 \x80</p>";
        let decoded = html(bytes, Some("text/html; charset=windows-1252"));
        assert!(decoded.text.contains("café €"));
        assert_eq!(decoded.metadata["selected_by"], "http_charset");
        assert!(decoded.warnings.iter().any(|w| w.code == "html_encoding_conflict"));
        let meta = html(b"<meta content='text/html; charset=iso-8859-1' http-equiv=Content-Type><p>\x80</p>", None);
        assert!(meta.text.ends_with("<p>€</p>"));
        assert_eq!(meta.metadata["encoding"], "windows-1252");
        let bom = html(b"\xef\xbb\xbf<meta charset=windows-1252><p>caf\xc3\xa9</p>", Some("text/html; charset=windows-1252"));
        assert_eq!(bom.metadata["selected_by"], "bom");
        assert!(bom.text.ends_with("<p>café</p>"));
        assert!(bom.warnings.iter().any(|w| w.code == "html_encoding_conflict"));
    }
    #[test]
    fn unknown_malformed_and_replacement_inputs_are_visible() {
        let unknown = html(b"<meta charset=nonsense><p>\xe9</p>", Some("text/html; charset=\"unterminated"));
        for code in ["html_encoding_malformed", "html_encoding_unknown", "html_encoding_fallback"] {
            assert!(unknown.warnings.iter().any(|w| w.code == code));
        }
        let broken = html(b"<p>\xff</p>", Some("text/html; charset=UTF-8"));
        assert!(broken.had_errors && broken.text.contains('\u{fffd}'));
        assert!(broken.warnings.iter().any(|w| w.code == "html_encoding_replacements"));
        assert!(decode(b"<p>x</p>", "text/html", Some("text/html; charset=iso-2022-cn"), false, 4096).is_err());
        let bounded_meta = html(&[vec![b' '; 1024], "<meta charset=windows-1252>é".as_bytes().to_vec()].concat(), None);
        assert_eq!(bounded_meta.metadata["selected_by"], "utf8_default");
    }
    #[test]
    fn output_is_bounded_and_saved_decoding_is_reproducible() {
        assert!(bounded(&[0x80; 100], WINDOWS_1252, 299).is_err());
        assert_eq!(bounded(&[0x80; 100], WINDOWS_1252, 300).unwrap().0.len(), 300);
        let bytes = b"<p>\x80</p>";
        let decoded = html(bytes, Some("text/html; charset=windows-1252"));
        assert_eq!(restore(bytes, Some(&decoded.metadata), 4096).unwrap(), decoded.text);
        assert!(restore(b"changed", Some(&decoded.metadata), 4096).is_err());
    }
    #[test]
    fn rendered_utf8_ignores_stale_meta_and_xhtml_does_not_sniff_html() {
        let source = "<meta charset=windows-1252><p>café 漢字</p>";
        let decoded = decode(source.as_bytes(), "text/html", None, true, 4096).unwrap();
        assert_eq!(decoded.text, source);
        assert_eq!(decoded.metadata["selected_by"], "rendered_utf8");
        assert!(decoded.warnings.iter().any(|w| w.code == "html_encoding_conflict"));
        assert!(decode(b"<p>\xe9</p>", "text/html", None, true, 4096).is_err());
        assert_eq!(decode(source.as_bytes(), "application/xhtml+xml", None, false, 4096).unwrap().text, source);
        assert!(decode(b"<?xml version='1.0' encoding='ISO-8859-1'?><p>x</p>", "application/xhtml+xml", None, false, 4096).is_err());
    }
}
