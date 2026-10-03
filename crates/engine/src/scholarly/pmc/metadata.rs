//! Selected PMC OAI front matter. An OA flag is not a reuse license.
use std::collections::BTreeMap;
use chrono::{DateTime, NaiveDate};
use roxmltree::{Document, Node, ParsingOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use webtool_protocol::*;
use super::{PmcError, Wanted, OAI_URL};

pub(super) const OAI_NS: &str = "http://www.openarchives.org/OAI/2.0/";
const XLINK: &str = "http://www.w3.org/1999/xlink";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Metadata {
    pub pmcid: String,
    pub versioned_pmcid: Option<String>,
    pub pmc_version: Option<String>,
    pub article_instance: Option<String>,
    pub oai_identifier: String,
    pub oai_datestamp: String,
    pub oai_response_date: String,
    pub sets: Vec<String>,
    pub front_sha256: String,
    pub jats_version: Option<String>,
    pub custom_meta: BTreeMap<String, String>,
    pub license_urls: Vec<String>,
    pub license_statements: Vec<String>,
    pub copyright: Vec<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub citation_date_key: Option<String>,
    pub full_text_eligible: bool,
}
pub(super) struct Selected { pub meta: Metadata, pub record: ScholarlyRecord }
pub(super) fn xml(input: &str) -> Result<Document<'_>, PmcError> {
    let doc = Document::parse_with_options(input, ParsingOptions { allow_dtd: false, nodes_limit: 200_000 })
        .map_err(|_| PmcError::InvalidXml)?;
    if doc.descendants().any(|n| n.ancestors().take(130).count() > 128) { return Err(PmcError::SizeLimit); }
    Ok(doc)
}
pub(super) fn children<'a, 'i>(node: Node<'a, 'i>, name: &str) -> std::vec::IntoIter<Node<'a, 'i>> {
    node.children().filter(|n| n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == node.tag_name().namespace()).collect::<Vec<_>>().into_iter()
}
pub(super) fn child<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> { children(node, name).next() }
fn one<'a, 'i>(node: Node<'a, 'i>, name: &str) -> Result<Option<Node<'a, 'i>>, PmcError> {
    let mut found = children(node, name); let value = found.next();
    if found.next().is_some() { return Err(PmcError::InvalidMetadata); } Ok(value)
}
pub(super) fn text(node: Node<'_, '_>) -> String {
    node.descendants().filter(|n| n.is_text()).filter_map(|n| n.text()).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}
fn value(node: Node<'_, '_>, name: &str) -> Option<String> { child(node, name).map(text).filter(|s| !s.is_empty()) }
pub(super) fn href<'a, 'i>(node: Node<'a, 'i>) -> Option<&'a str> { node.attribute((XLINK, "href")) }
pub(super) fn article<'a, 'i>(doc: &'a Document<'i>) -> Result<Node<'a, 'i>, PmcError> {
    let root = doc.root_element();
    if root.tag_name().name() != "OAI-PMH" || root.tag_name().namespace() != Some(OAI_NS) { return Err(PmcError::InvalidMetadata); }
    if let Some(error) = child(root, "error") {
        return Err(match error.attribute("code") { Some("idDoesNotExist" | "cannotDisseminateFormat" | "noRecordsMatch") => PmcError::Unavailable, _ => PmcError::InvalidMetadata });
    }
    let get = one(root, "GetRecord")?.ok_or(PmcError::InvalidMetadata)?;
    let record = one(get, "record")?.ok_or(PmcError::InvalidMetadata)?;
    let header = one(record, "header")?.ok_or(PmcError::InvalidMetadata)?;
    if header.attribute("status") == Some("deleted") { return Err(PmcError::Unavailable); }
    let metadata = one(record, "metadata")?.ok_or(PmcError::InvalidMetadata)?;
    let articles: Vec<_> = metadata.children().filter(|n| n.is_element()).collect();
    if articles.len() != 1 || articles[0].tag_name().name() != "article" { return Err(PmcError::InvalidMetadata); }
    if articles[0].tag_name().namespace().is_some_and(|ns| !matches!(ns,
        "https://jats.nlm.nih.gov/ns/archiving/1.4/" | "http://jats.nlm.nih.gov/ns/archiving/1.4/")) { return Err(PmcError::InvalidXml); }
    Ok(articles[0])
}
pub(super) fn datestamp(input: &str) -> Option<ScholarlyDate> {
    if input.len() == 10 && NaiveDate::parse_from_str(input, "%Y-%m-%d").is_ok() {
        Some(ScholarlyDate { value: input.into(), precision: "day".into() })
    } else if input.ends_with('Z') && DateTime::parse_from_rfc3339(input).is_ok() {
        Some(ScholarlyDate { value: input.into(), precision: "timestamp".into() })
    } else { None }
}
fn date(node: Node<'_, '_>) -> Option<ScholarlyDate> {
    if let Some(literal) = node.attribute("iso-8601-date") {
        // PMC last-change can include a time without a timezone. Do not invent UTC.
        return Some(datestamp(literal).unwrap_or_else(|| ScholarlyDate { value: literal.into(), precision: "literal".into() }));
    }
    let year = value(node, "year")?.parse::<i32>().ok()?;
    let month = value(node, "month"); let day = value(node, "day");
    let (value, precision) = match (month, day) {
        (None, None) => (year.to_string(), "year"),
        (Some(m), None) => { let m = m.parse::<u32>().ok()?; NaiveDate::from_ymd_opt(year, m, 1)?; (format!("{year:04}-{m:02}"), "month") },
        (Some(m), Some(d)) => { let m = m.parse::<u32>().ok()?; let d = d.parse::<u32>().ok()?; (NaiveDate::from_ymd_opt(year, m, d)?.to_string(), "day") },
        _ => return None,
    };
    Some(ScholarlyDate { value, precision: precision.into() })
}
fn license_url(input: &str) -> Option<String> {
    let url = url::Url::parse(input).ok()?;
    if !matches!(url.scheme(), "http" | "https") || !matches!(url.host_str(), Some("creativecommons.org" | "www.creativecommons.org")) ||
        !url.username().is_empty() || url.password().is_some() || url.port().is_some() || url.query().is_some() || url.fragment().is_some() { return None; }
    let path = url.path().trim_end_matches('/');
    matches!(path, "/publicdomain/zero/1.0" | "/licenses/by/3.0" | "/licenses/by/4.0" | "/licenses/by-sa/3.0" | "/licenses/by-sa/4.0")
        .then(|| format!("https://creativecommons.org{path}/"))
}
pub(super) fn parse(bytes: &[u8], wanted: &Wanted, prefix: &str) -> Result<Selected, PmcError> {
    let input = std::str::from_utf8(bytes).map_err(|_| PmcError::InvalidXml)?;
    let doc = xml(input)?; let article = article(&doc)?;
    let root = doc.root_element();
    let request = one(root, "request")?.ok_or(PmcError::InvalidMetadata)?;
    if request.attribute("verb") != Some("GetRecord") || request.attribute("identifier") != Some(wanted.oai().as_str()) ||
        request.attribute("metadataPrefix") != Some(prefix) || text(request) != OAI_URL { return Err(PmcError::IdentityMismatch); }
    let response_date = value(root, "responseDate").filter(|s| DateTime::parse_from_rfc3339(s).is_ok()).ok_or(PmcError::InvalidMetadata)?;
    let record_node = article.parent().and_then(|n| n.parent()).ok_or(PmcError::InvalidMetadata)?;
    let header = child(record_node, "header").ok_or(PmcError::InvalidMetadata)?;
    let oai_identifier = value(header, "identifier").ok_or(PmcError::InvalidMetadata)?;
    if oai_identifier != wanted.oai() { return Err(PmcError::IdentityMismatch); }
    let oai_datestamp = value(header, "datestamp").ok_or(PmcError::InvalidMetadata)?;
    let stamp = datestamp(&oai_datestamp).ok_or(PmcError::InvalidMetadata)?;
    let sets = children(header, "setSpec").map(text).collect::<Vec<_>>();
    let front = one(article, "front")?.ok_or(PmcError::InvalidMetadata)?;
    let meta = one(front, "article-meta")?.ok_or(PmcError::InvalidMetadata)?;
    let mut identifiers = BTreeMap::new();
    for id in children(meta, "article-id") {
        let name = id.attribute("pub-id-type").ok_or(PmcError::InvalidMetadata)?; let v = text(id);
        if v.is_empty() || identifiers.insert(name.to_owned(), v.clone()).is_some_and(|old| old != v) { return Err(PmcError::InvalidMetadata); }
    }
    let pmcid = identifiers.get("pmcid").filter(|s| **s == wanted.pmcid).cloned().ok_or(PmcError::IdentityMismatch)?;
    if identifiers.get("pmcaid").is_some_and(|v| v != wanted.number()) { return Err(PmcError::IdentityMismatch); }
    let versioned_pmcid = identifiers.get("pmcid-ver").cloned();
    if let Some(id) = &versioned_pmcid {
        let selected = Wanted::parse(&format!("pmc:{id}"))?;
        if selected.pmcid != pmcid || selected.version.is_none() { return Err(PmcError::IdentityMismatch); }
    }
    let versions: Vec<_> = children(meta, "article-version").filter(|n| n.attribute("article-version-type") == Some("pmc-version")).map(text).collect();
    if versions.len() > 1 { return Err(PmcError::InvalidMetadata); }
    let pmc_version = versions.first().cloned();
    if let Some(version) = &pmc_version {
        if !super::positive_digits(version, 9) || versioned_pmcid.as_ref().is_some_and(|id| id != &format!("{pmcid}.{version}")) { return Err(PmcError::IdentityMismatch); }
    }
    let article_instance = identifiers.get("pmcaiid").cloned();
    if article_instance.as_ref().is_some_and(|id| !super::positive_digits(id, 12)) { return Err(PmcError::InvalidMetadata); }
    let title = child(meta, "title-group").and_then(|n| value(n, "article-title")).filter(|s| !s.is_empty()).ok_or(PmcError::InvalidMetadata)?;
    let mut authors = Vec::new();
    for group in children(meta, "contrib-group") { for contrib in children(group, "contrib").filter(|n| n.attribute("contrib-type") == Some("author")) {
        let name = child(contrib, "name");
        let literal = child(contrib, "string-name").or_else(|| child(contrib, "collab")).map(text);
        let orcid = children(contrib, "contrib-id").find(|n| n.attribute("contrib-id-type") == Some("orcid")).map(text);
        authors.push(ScholarlyAuthor { literal, given: name.and_then(|n| value(n, "given-names")), family: name.and_then(|n| value(n, "surname")), orcid, id: None });
    } }
    let mut dates = BTreeMap::from([("oai_datestamp".into(), stamp)]); let mut warnings = Vec::new();
    for n in children(meta, "pub-date") {
        let mut key = format!("publication_{}", n.attribute("pub-type").or_else(|| n.attribute("date-type")).unwrap_or("unspecified"));
        if dates.contains_key(&key) { key = format!("{key}_{}", dates.len()); }
        if let Some(d) = date(n) { dates.insert(key, d); } else { warnings.push(Warning::new("pmc_date_unparsed", "A source publication date has unsupported or incomplete components. Consult the retained XML.")); }
    }
    if let Some(history) = child(meta, "history") { for n in children(history, "date") {
        if let Some(d) = date(n) { dates.insert(format!("history_{}", n.attribute("date-type").unwrap_or("unspecified")), d); }
    } }
    if let Some(history) = child(meta, "pub-history") { for event in children(history, "event") {
        if let Some(d) = child(event, "date").and_then(date) { dates.insert(event.attribute("event-type").unwrap_or("pmc_event").into(), d); }
    } }
    let citation_date_key = ["publication_epub", "publication_ppub", "publication_pub"].iter().find(|k| dates.contains_key(**k)).map(|k| k.to_string());
    let abstracts: Vec<_> = children(meta, "abstract").map(|abstract_node| {
        let parts = abstract_node.descendants().filter(|n| n.is_element() && matches!(n.tag_name().name(), "title" | "p")).map(text).filter(|s| !s.is_empty()).collect::<Vec<_>>();
        if parts.is_empty() { text(abstract_node) } else { parts.join("\n\n") }
    }).filter(|s| !s.is_empty()).collect();
    let abstract_text = (!abstracts.is_empty()).then(|| abstracts.join("\n\n"));
    let mut custom_meta = BTreeMap::new();
    if let Some(group) = child(meta, "custom-meta-group") { for entry in children(group, "custom-meta") {
        if let (Some(k), Some(v)) = (value(entry, "meta-name"), value(entry, "meta-value")) {
            if custom_meta.insert(k, v.clone()).is_some_and(|old| old != v) { return Err(PmcError::InvalidMetadata); }
        }
    } }
    let unavailable = custom_meta.get("pmc-status-live").is_some_and(|v| v == "no") || custom_meta.get("pmc-status-released").is_some_and(|v| v == "no") ||
        custom_meta.get("pmc-status-embargo").is_some_and(|v| v == "yes") || custom_meta.get("pmc-prop-legally-suppressed").is_some_and(|v| v == "yes");
    let permissions = child(meta, "permissions");
    let licenses = permissions.map(|p| children(p, "license").collect::<Vec<_>>()).unwrap_or_default();
    let mut license_urls = Vec::new(); let mut license_statements = Vec::new();
    for license in &licenses {
        if let Some(url) = href(*license) { license_urls.push(url.into()); }
        for n in license.descendants().filter(|n| n.is_element() && n.tag_name().name() == "license_ref" && n.tag_name().namespace() == Some("http://www.niso.org/schemas/ali/1.0/")) { license_urls.push(text(n)); }
        license_statements.extend(children(*license, "license-p").map(text));
    }
    let supported = if licenses.len() == 1 && licenses[0].attribute("specific-use").is_none() && licenses[0].attribute("content-type").is_none() {
        href(licenses[0]).and_then(license_url).filter(|url| license_urls.iter().all(|v| license_url(v).as_ref() == Some(url)))
    } else { None };
    let rights = ScholarlyRights { decision: if supported.is_some() { ReuseDecision::Permitted } else { ReuseDecision::Unknown }, license: supported,
        evidence_url: Some(wanted.url("pmc_fm").to_string()), basis: if licenses.len() == 1 {
            "Selected article-level permissions are retained from PMC front matter. Only explicit supported CC0, CC BY, or CC BY-SA URLs establish shared-text reuse. Preserve attribution, license, and applicable share-alike terms. Third-party material can have separate restrictions; associated files are not fetched."
        } else { "Selected article permissions are missing or ambiguous. OA flags, free access, and text-mining availability do not establish shared full-text reuse." }.into() };
    let full_text_eligible = !unavailable && sets.iter().any(|s| s == "pmc-open") && rights.decision == ReuseDecision::Permitted &&
        versioned_pmcid.is_some() && pmc_version.is_some() && article_instance.is_some();
    if versioned_pmcid.is_none() || pmc_version.is_none() || article_instance.is_none() { warnings.push(Warning::new("pmc_selected_identity_incomplete", "The source lacks a complete selected version/instance. Metadata only; no version or instance was invented.")); }
    let journal = child(front, "journal-meta").and_then(|n| child(n, "journal-title-group")).and_then(|n| value(n, "journal-title"));
    let copyright = permissions.map(|p| children(p, "copyright-statement").map(text).collect()).unwrap_or_default();
    let version = versioned_pmcid.clone();
    let record = ScholarlyRecord { id: format!("pmc:{}", version.as_ref().unwrap_or(&pmcid)), identifiers, title, authors, dates, abstract_text,
        content_state: if unavailable { ScholarlyContentState::Unavailable } else if abstracts.is_empty() { ScholarlyContentState::NotRead } else { ScholarlyContentState::AbstractOnly },
        locations: vec![ScholarlyLocation { id: article_instance.clone().map(|v| format!("pmc-instance:{v}")), landing_url: Some(format!("https://pmc.ncbi.nlm.nih.gov/articles/{}/", version.as_ref().unwrap_or(&pmcid))), pdf_url: None, version, primary: true, rights }], metadata_license: None,
        status: ScholarlyStatus { coverage: "Selected PMC source article-type only. This is not an exhaustive correction or retraction search.".into(),
            claims: vec![ScholarlyStatusClaim { field: "article-type".into(), related_id: None, label: article.attribute("article-type").unwrap_or("not supplied").into(), source: "PMC OAI selected article".into(), date: None, record_id: Some(oai_identifier.clone()) }] }, warnings };
    let meta = Metadata { pmcid, versioned_pmcid, pmc_version, article_instance, oai_identifier, oai_datestamp, oai_response_date: response_date, sets,
        front_sha256: hex::encode(Sha256::digest(&bytes[front.range()])), jats_version: article.attribute("dtd-version").map(str::to_owned), custom_meta, license_urls, license_statements, copyright,
        journal, volume: value(meta, "volume"), issue: value(meta, "issue"), pages: value(meta, "elocation-id").or_else(|| value(meta, "fpage")), citation_date_key, full_text_eligible };
    wanted.check(&meta)?; Ok(Selected { meta, record })
}
impl Metadata {
    pub fn same_selection(&self, other: &Self) -> bool {
        self.pmcid == other.pmcid && self.versioned_pmcid == other.versioned_pmcid && self.pmc_version == other.pmc_version &&
            self.article_instance == other.article_instance && self.oai_datestamp == other.oai_datestamp && self.front_sha256 == other.front_sha256
    }
}
