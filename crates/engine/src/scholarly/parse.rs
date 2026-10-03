//! Bounded metadata normalization. The complete response remains the original.
use std::collections::BTreeMap;
use chrono::{DateTime, NaiveDate};
use roxmltree::Node;
use serde_json::Value;
use webtool_protocol::*;
use super::ScholarlyError;

type Result<T> = std::result::Result<T, ScholarlyError>;
pub(super) struct Collection { pub records: Vec<ScholarlyRecord>, pub total: Option<u64>, pub partial: bool, pub warnings: Vec<Warning> }
fn string(v: &Value, key: &str) -> Option<String> { v.get(key).and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_owned) }
fn strings(v: &Value) -> Vec<String> { v.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default() }
pub(super) fn unknown_rights(license: Option<String>, evidence_url: Option<String>, basis: &str) -> ScholarlyRights {
    ScholarlyRights { decision: ReuseDecision::Unknown, license, evidence_url, basis: basis.into() }
}
fn record(id: String, title: String) -> ScholarlyRecord {
    ScholarlyRecord { id, identifiers: BTreeMap::new(), title, authors: vec![], dates: BTreeMap::new(), abstract_text: None,
        content_state: ScholarlyContentState::NotRead, locations: vec![], metadata_license: None,
        status: ScholarlyStatus { coverage: "Not checked for updates or retractions.".into(), claims: vec![] }, warnings: vec![] }
}
pub(super) fn date(value: String) -> ScholarlyDate {
    let precision = if DateTime::parse_from_rfc3339(&value).is_ok() { "timestamp" }
        else if NaiveDate::parse_from_str(&value, "%Y-%m-%d").is_ok() { "day" }
        else if value.len() == 4 && value.bytes().all(|b| b.is_ascii_digit()) { "year" }
        else { "literal" };
    ScholarlyDate { value, precision: precision.into() }
}
fn abstract_state(r: &mut ScholarlyRecord) {
    r.content_state = if r.abstract_text.as_ref().is_some_and(|s| !s.is_empty()) { ScholarlyContentState::AbstractOnly } else { ScholarlyContentState::NotRead };
}
fn xml_child<'a, 'i>(node: Node<'a, 'i>, ns: &str, name: &str) -> Option<Node<'a, 'i>> { node.children().find(|n| n.has_tag_name((ns, name))) }
fn xml_text(node: Node<'_, '_>) -> String { node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect::<String>().trim().to_owned() }
fn atom_text(node: Node<'_, '_>, name: &str) -> Option<String> { xml_child(node, "http://www.w3.org/2005/Atom", name).map(xml_text).filter(|s| !s.is_empty()) }
fn arxiv_entry(entry: Node<'_, '_>) -> Result<ScholarlyRecord> {
    let id_url = atom_text(entry, "id").ok_or(ScholarlyError::Malformed)?;
    let parsed = url::Url::parse(&id_url).map_err(|_| ScholarlyError::Malformed)?;
    let identity = crate::arxiv::identify(&parsed).map_err(|_| ScholarlyError::Malformed)?.ok_or(ScholarlyError::Malformed)?;
    let mut version = identity.version.clone();
    let mut landing = None; let mut pdf = None;
    for node in entry.children().filter(|n| n.has_tag_name(("http://www.w3.org/2005/Atom", "link"))) {
        let Some(href) = node.attribute("href") else { continue; };
        let relevant = node.attribute("rel") == Some("alternate") || node.attribute("title") == Some("pdf") || node.attribute("type") == Some("application/pdf");
        if !relevant { continue; }
        let link = url::Url::parse(href).map_err(|_| ScholarlyError::Malformed)?;
        let other = crate::arxiv::identify(&link).map_err(|_| ScholarlyError::Malformed)?.ok_or(ScholarlyError::Malformed)?;
        if other.id != identity.id || other.version.as_ref().is_some_and(|v| version.as_ref().is_some_and(|old| old != v)) { return Err(ScholarlyError::Malformed); }
        if other.version.is_some() { version = other.version; }
        if node.attribute("rel") == Some("alternate") { landing = Some(href.to_owned()); } else { pdf = Some(href.to_owned()); }
    }
    let mut r = record(format!("arxiv:{}{}", identity.id, version.as_ref().map(|v| format!("v{v}")).unwrap_or_default()), atom_text(entry, "title").ok_or(ScholarlyError::Malformed)?);
    r.identifiers.insert("arxiv".into(), identity.id.clone());
    if let Some(v) = &version { r.identifiers.insert("arxiv_version".into(), v.clone()); }
    else { r.warnings.push(Warning::new("arxiv_version_unknown", "The discovery entry has no explicit version. Inspect an explicit vN before full-text use.")); }
    for n in entry.children().filter(|n| n.has_tag_name(("http://www.w3.org/2005/Atom", "author"))) {
        r.authors.push(ScholarlyAuthor { literal: atom_text(n, "name"), given: None, family: None, orcid: None, id: None });
    }
    for key in ["published", "updated"] { if let Some(s) = atom_text(entry, key) { r.dates.insert(key.into(), date(s)); } }
    r.abstract_text = atom_text(entry, "summary"); abstract_state(&mut r);
    if let Some(v) = xml_child(entry, "http://arxiv.org/schemas/atom", "doi").map(xml_text) { r.identifiers.insert("doi".into(), v); }
    r.locations.push(ScholarlyLocation { id: Some(r.id.clone()), landing_url: landing.or(Some(id_url)), pdf_url: pdf,
        version: version.map(|v| format!("v{v}")), primary: true,
        rights: unknown_rights(None, None, "arXiv Atom metadata does not report the selected version's full-text license. No PDF was fetched.") });
    r.metadata_license = Some("https://creativecommons.org/publicdomain/zero/1.0/".into());
    Ok(r)
}
pub(super) fn arxiv(bytes: &[u8], limit: usize) -> Result<Collection> {
    let text = std::str::from_utf8(bytes).map_err(|_| ScholarlyError::Malformed)?;
    let doc = roxmltree::Document::parse(text).map_err(|_| ScholarlyError::Malformed)?;
    let root = doc.root_element();
    if !root.has_tag_name(("http://www.w3.org/2005/Atom", "feed")) { return Err(ScholarlyError::Malformed); }
    let total = xml_child(root, "http://a9.com/-/spec/opensearch/1.1/", "totalResults").map(xml_text)
        .and_then(|s| s.parse::<u64>().ok()).ok_or(ScholarlyError::Malformed)?;
    let entries: Vec<_> = root.children().filter(|n| n.has_tag_name(("http://www.w3.org/2005/Atom", "entry"))).collect();
    let mut result = Collection { records: vec![], total: Some(total), partial: false, warnings: vec![] };
    for entry in entries.iter().take(limit) {
        match arxiv_entry(*entry) { Ok(r) => result.records.push(r), Err(_) => { result.partial = true; result.warnings.push(Warning::new("scholarly_entry_invalid", "An arXiv entry had invalid or conflicting identity metadata and was omitted.")); } }
    }
    if total > 0 && result.records.is_empty() { return Err(ScholarlyError::Malformed); }
    if entries.len() > limit { result.partial = true; result.warnings.push(Warning::new("scholarly_result_limit", "Provider entries beyond the requested limit were omitted.")); }
    if result.records.len() < total.min(limit as u64) as usize { result.partial = true; }
    Ok(result)
}
fn oa_abstract(v: &Value) -> Result<Option<String>> {
    if v.is_null() { return Ok(None); }
    let index = v.as_object().ok_or(ScholarlyError::Malformed)?;
    if index.is_empty() { return Ok(None); }
    let mut tokens = BTreeMap::new();
    for (word, positions) in index {
        for position in positions.as_array().ok_or(ScholarlyError::Malformed)? {
            let n = position.as_u64().filter(|n| *n < 20000).ok_or(ScholarlyError::Malformed)?;
            if tokens.insert(n, word).is_some() { return Err(ScholarlyError::Malformed); }
        }
    }
    let Some(last) = tokens.keys().next_back().copied() else { return Ok(None); };
    if tokens.len() as u64 != last + 1 { return Err(ScholarlyError::Malformed); }
    Ok(Some(tokens.values().map(|s| s.as_str()).collect::<Vec<_>>().join(" ")))
}
fn oa_location(v: &Value, primary: bool) -> ScholarlyLocation {
    let license = string(v, "license_id").or_else(|| string(v, "license"));
    ScholarlyLocation { id: string(v, "id"), landing_url: string(v, "landing_page_url"), pdf_url: string(v, "pdf_url"),
        version: string(v, "version"), primary,
        rights: unknown_rights(license, string(v, "landing_page_url"), "OpenAlex location metadata only. Free-to-read and a reported license do not validate the selected file's reuse rights.") }
}
fn openalex_work(v: &Value) -> Result<ScholarlyRecord> {
    let id = string(v, "id").ok_or(ScholarlyError::Malformed)?;
    let suffix = id.strip_prefix("https://openalex.org/W").filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).ok_or(ScholarlyError::Malformed)?;
    let mut r = record(format!("openalex:W{suffix}"), string(v, "title").or_else(|| string(v, "display_name")).ok_or(ScholarlyError::Malformed)?);
    if let Some(ids) = v["ids"].as_object() { for (key, value) in ids { if let Some(s) = value.as_str() { r.identifiers.insert(key.clone(), s.into()); } } }
    r.identifiers.insert("openalex".into(), id);
    if let Some(s) = string(v, "doi") { r.identifiers.insert("doi".into(), s); }
    for key in ["publication_date", "created_date", "updated_date"] { if let Some(s) = string(v, key) { r.dates.insert(key.into(), date(s)); } }
    if let Some(year) = v["publication_year"].as_u64() { r.dates.insert("publication_year".into(), ScholarlyDate { value: year.to_string(), precision: "year".into() }); }
    let authorships = v["authorships"].as_array().ok_or(ScholarlyError::Malformed)?;
    for a in authorships.iter().take(100) {
        let a = &a["author"];
        r.authors.push(ScholarlyAuthor { literal: string(a, "display_name"), given: None, family: None, orcid: string(a, "orcid"), id: string(a, "id") });
    }
    if authorships.len() >= 100 || v["authors_count"].as_u64().is_some_and(|n| n > authorships.len() as u64) {
        r.warnings.push(Warning::new("scholarly_authors_partial", "OpenAlex returns at most the first 100 authorships. This author list may be incomplete."));
    }
    match oa_abstract(&v["abstract_inverted_index"]) {
        Ok(a) => r.abstract_text = a,
        Err(_) => r.warnings.push(Warning::new("scholarly_abstract_unavailable", "The OpenAlex abstract index has gaps, conflicts, or invalid positions. It was not reconstructed.")),
    }
    abstract_state(&mut r);
    let primary = &v["primary_location"];
    if primary.is_object() { r.locations.push(oa_location(primary, true)); }
    if let Some(locations) = v["locations"].as_array() {
        for loc in locations.iter().take(100) {
            if loc != primary { r.locations.push(oa_location(loc, false)); }
        }
        if locations.len() > 100 { r.warnings.push(Warning::new("scholarly_locations_partial", "Only the first 100 provider locations and the primary location were normalized. The original retains all reported locations.")); }
    }
    r.metadata_license = Some("https://creativecommons.org/publicdomain/zero/1.0/".into());
    r.status.coverage = "OpenAlex discovery record only. Provider-reported status is not an exhaustive notice search or proof that no retraction exists.".into();
    if let Some(value) = v["is_retracted"].as_bool() { r.status.claims.push(ScholarlyStatusClaim { field: "is_retracted".into(), related_id: None,
        label: value.to_string(), source: "OpenAlex".into(), date: None, record_id: None }); }
    Ok(r)
}
pub(super) fn openalex(bytes: &[u8], limit: usize) -> Result<Collection> {
    let v: Value = serde_json::from_slice(bytes).map_err(|_| ScholarlyError::Malformed)?;
    let entries = v["results"].as_array().ok_or(ScholarlyError::Malformed)?;
    let total = v["meta"]["count"].as_u64().ok_or(ScholarlyError::Malformed)?;
    let mut result = Collection { records: vec![], total: Some(total), partial: false, warnings: vec![] };
    for entry in entries.iter().take(limit) {
        match openalex_work(entry) { Ok(r) => { result.partial |= !r.warnings.is_empty(); result.records.push(r); }, Err(_) => {
            result.partial = true; result.warnings.push(Warning::new("scholarly_entry_invalid", "An OpenAlex result with invalid required metadata was omitted."));
        } }
    }
    if (total > 0 || !entries.is_empty()) && result.records.is_empty() { return Err(ScholarlyError::Malformed); }
    if entries.len() > limit || result.records.len() < total.min(limit as u64) as usize { result.partial = true; }
    Ok(result)
}
fn crossref_date(v: &Value) -> Option<ScholarlyDate> {
    if let Some(parts) = v["date-parts"].as_array().and_then(|a| a.first()).and_then(Value::as_array) {
        if (1..=3).contains(&parts.len()) && parts.iter().all(|p| p.as_u64().is_some()) {
            let values: Vec<_> = parts.iter().map(|p| p.as_u64().unwrap()).collect();
            // Keep provider precision. Do not make a year-only date January 1.
            let value = values.iter().enumerate().map(|(i,n)| if i == 0 { format!("{n:04}") } else { format!("{n:02}") }).collect::<Vec<_>>().join("-");
            return Some(ScholarlyDate { value, precision: ["year", "month", "day"][values.len()-1].into() });
        }
    }
    string(v, "date-time").map(date)
}
pub(super) fn crossref(bytes: &[u8], expected_doi: &str) -> Result<Collection> {
    let envelope: Value = serde_json::from_slice(bytes).map_err(|_| ScholarlyError::Malformed)?;
    if envelope["status"] != "ok" || envelope["message-type"] != "work" { return Err(ScholarlyError::Malformed); }
    let v = &envelope["message"];
    let doi = string(v, "DOI").ok_or(ScholarlyError::Malformed)?;
    if !doi.eq_ignore_ascii_case(expected_doi) { return Err(ScholarlyError::Malformed); }
    let title = strings(&v["title"]).into_iter().next().ok_or(ScholarlyError::Malformed)?;
    let mut r = record(format!("doi:{doi}"), title);
    r.identifiers.insert("doi".into(), doi.clone());
    if let Some(authors) = v["author"].as_array() { for a in authors.iter().take(1000) {
        r.authors.push(ScholarlyAuthor { literal: string(a, "name"), given: string(a, "given"), family: string(a, "family"), orcid: string(a, "ORCID"), id: None });
    }
        if authors.len() > 1000 { r.warnings.push(Warning::new("scholarly_authors_partial", "Only the first 1000 Crossref authors were normalized. The original retains all reported authors.")); }
    }
    for key in ["published", "published-print", "published-online", "issued", "created", "deposited", "indexed", "posted", "accepted"] {
        if let Some(d) = crossref_date(&v[key]) { r.dates.insert(key.into(), d); }
        if let Some(s) = string(&v[key], "date-time") { r.dates.insert(format!("{key}.date-time"), date(s)); }
    }
    // This field can contain JATS markup. Preserve it literally, not as parsed full text.
    r.abstract_text = string(v, "abstract"); abstract_state(&mut r);
    if r.abstract_text.is_some() { r.warnings.push(Warning::new("crossref_abstract_markup", "The abstract is publisher-deposited metadata and may contain JATS markup. It is not a paper body.")); }
    r.locations.push(ScholarlyLocation { id: Some(format!("doi:{doi}")), landing_url: string(&v["resource"]["primary"], "URL").or_else(|| string(v, "URL")), pdf_url: None, version: None, primary: true,
        rights: unknown_rights(None, None, "A Crossref DOI record does not establish the reuse rights of a selected full-text file.") });
    if let Some(licenses) = v["license"].as_array() { for license in licenses.iter().take(100) {
        r.locations.push(ScholarlyLocation { id: None, landing_url: None, pdf_url: None, version: string(license, "content-version"), primary: false,
            rights: unknown_rights(string(license, "URL"), Some(format!("https://doi.org/{doi}")), "Crossref-deposited license, scoped by content-version. Start date and full record remain in the original. Not matched to a fetched file.") });
    } }
    r.status.coverage = "Checked the selected Crossref singleton record only. update-to points from this record to the related DOI. Claims do not establish which version was retracted. No exhaustive update search was made; an empty claim list is not proof of no retraction.".into();
    for field in ["update-to", "updated-by"] {
        if let Some(claims) = v[field].as_array() { for claim in claims.iter().take(100) {
            r.status.claims.push(ScholarlyStatusClaim { field: field.into(), related_id: string(claim, "DOI").map(|d| format!("doi:{d}")),
                label: string(claim, "type").or_else(|| string(claim, "label")).unwrap_or_else(|| "unspecified update".into()),
                source: string(claim, "source").unwrap_or_else(|| "Crossref deposited metadata (source unspecified)".into()),
                date: crossref_date(&claim["updated"]), record_id: claim.get("record-id").filter(|v| !v.is_null()).map(|v| v.as_str().map(str::to_owned).unwrap_or_else(|| v.to_string())) });
        }
            if claims.len() > 100 { r.warnings.push(Warning::new("scholarly_status_partial", "Only the first 100 claims in this update field were normalized. The original retains all reported claims.")); }
        }
    }
    Ok(Collection { partial: !r.warnings.is_empty(), records: vec![r], total: None, warnings: vec![] })
}
pub(super) fn selected_arxiv(p: &crate::arxiv::Paper) -> ScholarlyRecord {
    let mut r = record(format!("arxiv:{}", p.versioned_id), p.title.clone());
    r.identifiers.insert("arxiv".into(), p.id.clone()); r.identifiers.insert("arxiv_version".into(), p.version.clone());
    if let Some(doi) = &p.doi { r.identifiers.insert("journal_doi".into(), doi.clone()); }
    if let Some(doi) = &p.arxiv_doi { r.identifiers.insert("arxiv_doi".into(), doi.clone()); }
    r.authors = p.authors.iter().map(|name| ScholarlyAuthor { literal: Some(name.clone()), given: None, family: None, orcid: None, id: None }).collect();
    for (key, value) in &p.source_dates { r.dates.insert(key.clone(), date(value.clone())); }
    r.abstract_text = Some(p.abstract_text.clone()); abstract_state(&mut r);
    r.locations.push(ScholarlyLocation { id: Some(r.id.clone()), landing_url: Some(p.abstract_url.clone()), pdf_url: Some(p.pdf_url.clone()), version: Some(format!("v{}", p.version)), primary: true, rights: p.rights() });
    r.metadata_license = Some("https://creativecommons.org/publicdomain/zero/1.0/".into()); r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_identity_precision_and_notice_direction() {
        let json = br#"{"status":"ok","message-type":"work","message":{"DOI":"10.1234/notice","title":["Notice"],"author":[{"given":"A.","family":"Name"}],"published":{"date-parts":[[2020]]},"update-to":[{"DOI":"10.1234/original","type":"retraction","source":"publisher","updated":{"date-parts":[[2021,6]]}}]}}"#;
        let c = crossref(json, "10.1234/notice").unwrap(); let r = &c.records[0];
        assert_eq!(r.dates["published"].value, "2020");
        assert_eq!(r.status.claims[0].date.as_ref().unwrap().value, "2021-06");
        assert_eq!(r.status.claims[0].related_id.as_deref(), Some("doi:10.1234/original"));
        assert_eq!(r.status.claims[0].field, "update-to"); assert!(r.authors[0].literal.is_none());
        assert!(crossref(json, "10.1234/other").is_err());
        assert!(openalex(br#"{"error":"no access"}"#, 5).is_err());
        assert!(oa_abstract(&serde_json::json!({"one":[0],"three":[2]})).is_err());
        assert!(oa_abstract(&serde_json::json!({"one":[0],"duplicate":[0]})).is_err());
        assert!(arxiv(b"<html>Error</html>", 5).is_err());
    }
    #[test]
    fn atom_unversioned_id_requires_agreeing_explicit_links() {
        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom" xmlns:o="http://a9.com/-/spec/opensearch/1.1/"><o:totalResults>1</o:totalResults><entry><id>http://arxiv.org/abs/2401.12345</id><title>Title</title><author><name>A. Author</name></author><link rel="alternate" href="http://arxiv.org/abs/2401.12345v2"/><link title="pdf" href="http://arxiv.org/pdf/2401.12345v2"/><summary>Abstract</summary></entry></feed>"#;
        assert_eq!(arxiv(atom.as_bytes(), 5).unwrap().records[0].id, "arxiv:2401.12345v2");
        let conflict = atom.replace("pdf/2401.12345v2", "pdf/2401.12345v1");
        assert!(arxiv(conflict.as_bytes(), 5).is_err());
    }
}
