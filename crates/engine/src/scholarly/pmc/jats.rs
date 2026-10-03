//! A narrow retained-JATS reader, not a schema validator or publisher renderer.
use std::collections::{HashMap, HashSet};
use roxmltree::Node;
use serde_json::{json, Map, Value};
use webtool_protocol::*;
use crate::readers::Parsed;
use super::{metadata::{self, child, children, href, text, Selected}, PmcError};
pub(super) const PARSER: &str = "pmc-jats/1";
fn range(node: Node<'_, '_>) -> Value { let r = node.range(); json!([r.start, r.end]) }
fn raw_text(node: Node<'_, '_>) -> String { node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect() }
fn normalized(input: &str) -> String {
    // Retain explicit cell/paragraph breaks while collapsing XML layout whitespace.
    input.split('\n').map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n").trim().to_owned()
}
fn safe_link(input: &str) -> bool {
    url::Url::parse(input).is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some() && u.username().is_empty() && u.password().is_none())
}
struct Reader<'a, 'i> {
    parsed: Parsed,
    ids: HashMap<String, Vec<Node<'a, 'i>>>,
    blocks: Map<String, Value>,
    xrefs: Vec<Value>, references: Vec<Value>, formulas: Vec<Value>, objects: Vec<Value>, unsupported: Vec<Value>,
    rendered_refs: HashSet<usize>,
    partial: bool,
}
impl Reader<'_, '_> {
    fn warn(&mut self, code: &str, message: &str) {
        if !self.parsed.warnings.iter().any(|w| w.code == code) { self.parsed.warnings.push(Warning::new(code, message)); }
    }
    fn push(&mut self, content: Content, node: Node<'_, '_>) -> Result<(), PmcError> {
        if self.parsed.blocks.len() >= 10_000 { return Err(PmcError::SizeLimit); }
        let index = self.parsed.blocks.len() + 1;
        self.parsed.push(content, Locator::Derived { index });
        self.blocks.insert(format!("b{index}"), json!({"source_range":range(node),"element":node.tag_name().name(),"source_id":node.attribute("id"),"mapping":"enclosing XML element, not an exact normalized-text span"}));
        Ok(())
    }
    fn required(&mut self, node: Node<'_, '_>, reason: &str) -> String {
        self.partial = true;
        self.unsupported.push(json!({"source_range":range(node),"element":node.tag_name().name(),"source_id":node.attribute("id"),"reason":reason}));
        self.warn("jats_source_required", "Some source structures need the retained XML or associated files. Source-required markers are not recovered content. Inspect metadata.jats.unsupported and the original.");
        format!("[Source required: {}{}; {reason}]", node.tag_name().name(), node.attribute("id").map(|id| format!(" {id}")).unwrap_or_default())
    }
    fn formula(&mut self, node: Node<'_, '_>) -> (String, String) {
        let tex: Vec<_> = node.descendants().filter(|n| n.is_element() && (n.tag_name().name() == "tex-math" ||
            (n.tag_name().name() == "annotation" && n.tag_name().namespace() == Some("http://www.w3.org/1998/Math/MathML") &&
                n.attribute("encoding").is_some_and(|v| matches!(v, "application/x-tex" | "text/x-tex" | "TeX"))))).map(raw_text).filter(|s| !s.trim().is_empty()).collect();
        let (value, representation) = if tex.len() == 1 { (tex[0].clone(), "supplied_tex") }
        else if tex.len() > 1 { (self.required(node, "ambiguous supplied TeX alternatives"), "source_required") }
        else if let Some(alt) = node.descendants().filter(|n| n.is_element() && n.tag_name().name() == "math").find_map(|n| n.attribute("alttext")) {
            (alt.to_owned(), "supplied_alttext")
        } else if let Some(alt) = child(node, "alt-text").map(text).filter(|s| !s.is_empty()) { (alt, "supplied_alttext") }
        else if node.descendants().any(|n| n.is_element() && matches!(n.tag_name().name(), "math" | "graphic" | "inline-graphic" | "alternatives")) {
            let files = node.descendants().filter(|n| n.is_element() && matches!(n.tag_name().name(), "graphic" | "inline-graphic")).filter_map(href).collect::<Vec<_>>().join(", ");
            (self.required(node, &if files.is_empty() { "unsupported mathematical notation in XML".into() } else { format!("formula file {files} was not fetched") }), "source_required")
        } else {
            let value = node.children().filter(|n| !n.is_element() || n.tag_name().name() != "label").map(|n| self.inline(n, false)).collect::<String>();
            if value.trim().is_empty() { (self.required(node, "no supplied notation"), "source_required") } else { (normalized(&value), "supplied_plain") }
        };
        self.formulas.push(json!({"source_range":range(node),"source_id":node.attribute("id"),"label":child(node,"label").map(text),"representation":representation,
            "source_files":node.descendants().filter(|n| n.is_element()).filter_map(href).collect::<Vec<_>>() }));
        (value, representation.into())
    }
    fn inline(&mut self, node: Node<'_, '_>, reference: bool) -> String {
        if node.is_text() { return node.text().unwrap_or("").replace(['\n', '\r', '\t'], " "); }
        if !node.is_element() { return String::new(); }
        let tag = node.tag_name().name();
        match tag {
            "inline-formula" => { let (value, kind) = self.formula(node); if kind == "supplied_tex" { format!("\\({value}\\)") } else { value } },
            "disp-formula" => self.formula(node).0,
            "xref" => {
                let label = text(node); let ids = node.attribute("rid").unwrap_or("").split_whitespace().collect::<Vec<_>>();
                let targets = ids.iter().map(|id| match self.ids.get(*id) {
                    Some(nodes) if nodes.len() == 1 => json!({"id":id,"source_range":range(nodes[0])}), _ => json!({"id":id,"source_range":null}),
                }).collect::<Vec<_>>();
                let resolved = !ids.is_empty() && targets.iter().all(|v| !v["source_range"].is_null());
                self.xrefs.push(json!({"source_range":range(node),"label":label,"rid":ids,"ref_type":node.attribute("ref-type"),"targets":targets,"resolved":resolved}));
                if !resolved || label.is_empty() { self.partial = true; self.warn("jats_xref_unresolved", "A cross-reference has a missing/ambiguous target or an empty supplied label. No reference label or target was guessed."); }
                if label.is_empty() { self.required(node, "empty supplied xref label") } else { label }
            },
            "sup" | "sub" => {
                if node.descendants().any(|n| n.is_element() && n.tag_name().name() == "xref") {
                    return node.children().map(|n| self.inline(n, reference)).collect();
                }
                let value = text(node);
                if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
                    let digits = if tag == "sup" { ['⁰','¹','²','³','⁴','⁵','⁶','⁷','⁸','⁹'] } else { ['₀','₁','₂','₃','₄','₅','₆','₇','₈','₉'] };
                    value.bytes().map(|b| digits[(b - b'0') as usize]).collect()
                } else { format!("{}({value})", if tag == "sup" { "^" } else { "_" }) }
            },
            "name" if reference => format!(" {} ", node.children().filter(|n| n.is_element()).map(text).collect::<Vec<_>>().join(" ")),
            "string-name" if reference => format!(" {} ", node.children().map(|n| self.inline(n, reference)).collect::<String>()),
            "pub-id" if reference => format!(" [{}: {}] ", node.attribute("pub-id-type").unwrap_or("pub-id"), text(node)),
            "break" | "br" => "\n".into(),
            "graphic" | "inline-graphic" | "media" | "inline-media" => self.object(node),
            "ext-link" | "uri" => {
                let value = node.children().map(|n| self.inline(n, reference)).collect::<String>();
                let target = href(node).or_else(|| (tag == "uri").then_some(value.as_str()));
                if let Some(url) = target.filter(|s| safe_link(s)) { self.parsed.links.push(Link { url: url.into(), text: normalized(&value) }); }
                value
            },
            "p" | "list-item" | "title" | "caption" => format!("\n{}\n", node.children().map(|n| self.inline(n, reference)).collect::<String>()),
            "list" => node.children().map(|n| self.inline(n, reference)).collect(),
            "italic" | "bold" | "underline" | "monospace" | "roman" | "sans-serif" | "sc" | "strike" | "named-content" | "styled-content" |
            "label" | "surname" | "given-names" | "suffix" | "prefix" | "collab" | "etal" | "year" | "month" | "day" | "article-title" | "source" | "volume" | "issue" |
            "fpage" | "lpage" | "elocation-id" | "edition" | "comment" | "publisher-name" | "publisher-loc" | "person-group" | "mixed-citation" | "element-citation" | "x" |
            "abbrev" | "email" => {
                let value = node.children().map(|n| self.inline(n, reference)).collect::<String>();
                if reference && matches!(tag, "year" | "article-title" | "source" | "volume" | "fpage" | "lpage" | "person-group") { format!(" {value} ") } else { value }
            },
            _ => self.required(node, "unsupported inline structure"),
        }
    }
    fn object(&mut self, node: Node<'_, '_>) -> String {
        let source_file = href(node).unwrap_or("not supplied");
        self.objects.push(json!({"source_range":range(node),"element":node.tag_name().name(),"source_file":source_file,"fetched":false}));
        if safe_link(source_file) { self.parsed.links.push(Link { url: source_file.into(), text: node.tag_name().name().into() }); }
        // Relative package filenames are not absolute downloadable URLs.
        self.required(node, &format!("associated file {source_file} was not fetched"))
    }
    fn paragraph(&mut self, node: Node<'_, '_>, level: u8) -> Result<(), PmcError> {
        let mut run = String::new();
        for n in node.children() {
            if n.is_element() && matches!(n.tag_name().name(), "disp-formula" | "table-wrap" | "fig" | "list" | "boxed-text" | "disp-quote" | "code" | "preformat") {
                let text = normalized(&run); if !text.is_empty() { self.push(Content::Paragraph { text }, node)?; } run.clear(); self.walk(n, level)?;
            } else { run.push_str(&self.inline(n, false)); }
        }
        let text = normalized(&run); if !text.is_empty() { self.push(Content::Paragraph { text }, node)?; } Ok(())
    }
    fn caption(&mut self, node: Node<'_, '_>) -> Result<(), PmcError> {
        let mut label = children(node, "label").map(text).collect::<Vec<_>>().join(" ");
        for caption in children(node, "caption") {
            for n in caption.children().filter(|n| n.is_element()) {
                let text = normalized(&self.inline(n, false));
                if !text.is_empty() { self.push(Content::Caption { text: if label.is_empty() { text } else { format!("{label}: {text}") } }, n)?; label.clear(); }
            }
        }
        if !label.is_empty() { self.push(Content::Caption { text: label }, node)?; } Ok(())
    }
    fn table(&mut self, node: Node<'_, '_>) -> Result<(), PmcError> {
        if node.descendants().any(|n| n.is_element() && (n.tag_name().name() == "tgroup" || (n != node && n.tag_name().name() == "table"))) {
            let text = self.required(node, "CALS or nested table model is unsupported"); return self.push(Content::Paragraph { text }, node);
        }
        let source_rows = node.descendants().filter(|n| n.is_element() && n.tag_name().name() == "tr").collect::<Vec<_>>();
        if source_rows.len() > 1000 { return Err(PmcError::SizeLimit); }
        let mut rows = Vec::new(); let mut valid = !source_rows.is_empty();
        for (index, row) in source_rows.iter().enumerate() {
            let mut cells = Vec::new(); let mut width: usize = 0;
            for cell in row.children().filter(|n| n.is_element() && matches!(n.tag_name().name(), "th" | "td")) {
                let span = |name| match cell.attribute(name) { None => Some(1), Some(v) => v.parse::<usize>().ok().filter(|n| (1..=1000).contains(n)) };
                let (Some(row_span), Some(col_span)) = (span("rowspan"), span("colspan")) else { valid = false; continue; };
                if row_span > source_rows.len() - index { valid = false; }
                width = width.saturating_add(col_span); if width > 1000 { return Err(PmcError::SizeLimit); }
                let text = normalized(&cell.children().map(|n| self.inline(n, false)).collect::<String>());
                // Adjacent paragraph/list wrappers share one cell line boundary.
                let text = text.lines().filter(|line| !line.is_empty()).collect::<Vec<_>>().join("\n");
                cells.push(Cell { text, row_span, col_span, header: cell.tag_name().name() == "th" });
            }
            if cells.is_empty() { valid = false; } rows.push(cells);
        }
        if valid { self.push(Content::Table { rows }, node) } else { let text = self.required(node, "missing cells or unsupported table spans"); self.push(Content::Paragraph { text }, node) }
    }
    fn list(&mut self, node: Node<'_, '_>, level: u8) -> Result<(), PmcError> {
        let depth = node.ancestors().filter(|n| n.is_element() && n.tag_name().name() == "list").count().saturating_sub(1);
        if depth > 32 { return Err(PmcError::SizeLimit); }
        let ordered = node.attribute("list-type") == Some("order");
        if node.attribute("list-type").is_some_and(|v| !matches!(v, "order" | "bullet" | "simple")) { self.partial = true; self.warn("jats_list_labels_derived", "A nonstandard list type has no automatic numbering conversion. Supplied labels remain text; consult the source for numbering."); }
        for (ordinal, item) in children(node, "list-item").enumerate() {
            let start = self.parsed.blocks.len();
            for n in item.children().filter(|n| n.is_element()) { self.walk(n, level)?; }
            for index in start..self.parsed.blocks.len() {
                let id = self.parsed.blocks[index].id.clone();
                if self.parsed.metadata["list_items"].get(&id).is_none() {
                    self.parsed.metadata["list_items"][&id] = json!({"depth":depth,"ordinal":if ordered {Some(ordinal+1)} else {None},"first":index==start});
                }
            }
        }
        Ok(())
    }
    fn reference(&mut self, node: Node<'_, '_>) -> Result<(), PmcError> {
        let start = self.parsed.blocks.len();
        let label = child(node, "label").map(text).unwrap_or_default();
        let citations = node.children().filter(|n| n.is_element() && n.tag_name().name() != "label").collect::<Vec<_>>();
        if citations.is_empty() { let text = self.required(node, "reference has no supplied citation"); self.push(Content::Paragraph { text }, node)?; }
        for citation in citations {
            let text = normalized(&self.inline(citation, true));
            if !text.is_empty() { self.push(Content::Paragraph { text: if label.is_empty() { text } else { format!("{label}. {text}") } }, node)?; }
        }
        if label.is_empty() { self.partial = true; self.warn("jats_reference_label_missing", "A reference has no supplied label. No bibliography number was invented."); }
        self.rendered_refs.insert(node.range().start);
        self.references.push(json!({"source_id":node.attribute("id"),"label":label,"source_range":range(node),"blocks":self.parsed.blocks[start..].iter().map(|b|b.id.clone()).collect::<Vec<_>>(),"displayed":true}));
        Ok(())
    }
    fn walk(&mut self, node: Node<'_, '_>, level: u8) -> Result<(), PmcError> {
        if !node.is_element() { return Ok(()); }
        match node.tag_name().name() {
            "p" => self.paragraph(node, level),
            "title" => { let text = normalized(&self.inline(node, false)); if !text.is_empty() { self.push(Content::Heading { level: level.min(6), text }, node)?; } Ok(()) },
            "sec" | "app" => {
                for n in node.children().filter(|n| n.is_element()) { self.walk(n, if matches!(n.tag_name().name(), "sec" | "app") { level.saturating_add(1) } else { level })?; } Ok(())
            },
            "list" => self.list(node, level),
            "table" => self.table(node),
            "table-wrap" | "fig" | "supplementary-material" => {
                self.caption(node)?;
                for n in node.children().filter(|n| n.is_element() && !matches!(n.tag_name().name(), "label" | "caption" | "object-id")) { self.walk(n, level)?; } Ok(())
            },
            "disp-formula" => {
                let (value, _) = self.formula(node); self.push(Content::Math { text: value }, node)?;
                if let Some(label) = child(node, "label") { self.push(Content::Caption { text: text(label) }, label)?; } Ok(())
            },
            "ref" => self.reference(node),
            "code" | "preformat" => self.push(Content::Code { language: node.attribute("language").map(str::to_owned), text: raw_text(node) }, node),
            "disp-quote" => { let text = normalized(&node.children().map(|n| self.inline(n, false)).collect::<String>()); self.push(Content::Quote { text }, node) },
            "graphic" | "inline-graphic" | "media" | "inline-media" => {
                let text = self.object(node); self.push(Content::Paragraph { text }, node)?;
                for caption in children(node, "caption") { for n in caption.children().filter(|n| n.is_element()) { let text = normalized(&self.inline(n, false)); self.push(Content::Caption { text }, n)?; } } Ok(())
            },
            "body" | "back" | "ref-list" | "ack" | "fn" | "fn-group" | "table-wrap-foot" | "app-group" | "boxed-text" | "fig-group" | "table-wrap-group" | "disp-formula-group" | "floats-group" | "notes" => {
                for n in node.children().filter(|n| n.is_element()) { self.walk(n, level)?; } Ok(())
            },
            "label" => self.push(Content::Paragraph { text: text(node) }, node),
            _ => { let text = self.required(node, "unsupported block structure"); self.push(Content::Paragraph { text }, node) },
        }
    }
}
pub(super) fn parse(bytes: &[u8], selected: &Selected) -> Result<Parsed, PmcError> {
    let doc = metadata::xml(std::str::from_utf8(bytes).map_err(|_| PmcError::InvalidXml)?)?; let article = metadata::article(&doc)?;
    let body = child(article, "body").ok_or(PmcError::BodyUnavailable)?;
    // A heading, bibliography, or unsupported-object marker alone is not a paper body.
    if !body.descendants().any(|n| n.is_element() && matches!(n.tag_name().name(), "p" | "code" | "preformat" | "td" | "th") && !text(n).is_empty()) { return Err(PmcError::BodyUnavailable); }
    let mut ids: HashMap<String, Vec<Node<'_, '_>>> = HashMap::new();
    for node in article.descendants().filter(|n| n.is_element()) { if let Some(id) = node.attribute("id") { ids.entry(id.into()).or_default().push(node); } }
    let mut parsed = Parsed::new(&selected.record.title, PARSER); parsed.metadata["list_items"] = json!({});
    let mut reader = Reader { parsed, ids, blocks: Map::new(), xrefs: vec![], references: vec![], formulas: vec![], objects: vec![], unsupported: vec![], rendered_refs: HashSet::new(), partial: false };
    if let Some(meta) = child(article, "front").and_then(|n| child(n, "article-meta")) {
        for abstract_node in children(meta, "abstract") {
            if child(abstract_node, "title").is_none() { reader.push(Content::Heading { level: 2, text: "Abstract (source front matter)".into() }, abstract_node)?; }
            for n in abstract_node.children().filter(|n| n.is_element()) { reader.walk(n, 2)?; }
        }
    }
    reader.walk(body, 2)?;
    if let Some(back) = child(article, "back") { reader.walk(back, 2)?; }
    if let Some(floats) = child(article, "floats-group") { reader.walk(floats, 2)?; }
    for reference in article.descendants().filter(|n| n.is_element() && n.tag_name().name() == "ref") {
        if !reader.rendered_refs.contains(&reference.range().start) {
            reader.partial = true; reader.references.push(json!({"source_id":reference.attribute("id"),"label":child(reference,"label").map(text),"source_range":range(reference),"blocks":[],"displayed":false}));
            reader.warn("jats_reference_unrendered", "Some source references are not represented as reading blocks. Their exact XML ranges remain in metadata.jats.references. This is a partial read, not faithful bibliography coverage.");
        }
    }
    reader.warn("jats_locations_derived", "Reading blocks use derived locations. metadata.jats.blocks gives half-open UTF-8 byte ranges of enclosing elements in the retained XML, not exact normalized-text spans. Inline styles and reference field boundaries are normalized.");
    reader.parsed.metadata["jats"] = json!({"partial":reader.partial,"source_version":selected.meta.jats_version,"blocks":reader.blocks,"xrefs":reader.xrefs,"references":reader.references,
        "formulas":reader.formulas,"external_objects":reader.objects,"unsupported":reader.unsupported,"source_ranges":"half-open UTF-8 byte offsets in the retained OAI XML"});
    Ok(reader.parsed)
}
