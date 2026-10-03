//! Two bounded synthetic checks. No public provider request or broad corpus.
use super::*;
use serde_json::Value;
fn fixture(prefix: &str, body: &str) -> Vec<u8> {
    format!(r#"<OAI-PMH xmlns="http://www.openarchives.org/OAI/2.0/"><responseDate>2026-01-02T03:04:05Z</responseDate><request verb="GetRecord" identifier="oai:pubmedcentral.nih.gov:12345" metadataPrefix="{prefix}">{OAI_URL}</request><GetRecord><record><header><identifier>oai:pubmedcentral.nih.gov:12345</identifier><datestamp>2024-02-03</datestamp><setSpec>pmc-open</setSpec></header><metadata><article xmlns="https://jats.nlm.nih.gov/ns/archiving/1.4/" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:mml="http://www.w3.org/1998/Math/MathML" article-type="research-article" dtd-version="1.4"><front><journal-meta><journal-title-group><journal-title>Synthetic Journal</journal-title></journal-title-group></journal-meta><article-meta><article-id pub-id-type="pmcid">PMC12345</article-id><article-id pub-id-type="pmcid-ver">PMC12345.2</article-id><article-id pub-id-type="pmcaid">12345</article-id><article-id pub-id-type="pmcaiid">67890</article-id><article-id pub-id-type="doi">10.1234/example</article-id><article-version article-version-type="pmc-version">2</article-version><title-group><article-title>Source title</article-title></title-group><contrib-group><contrib contrib-type="author"><name><surname>First</surname><given-names>A.</given-names></name></contrib><contrib contrib-type="author"><collab>Literal group</collab></contrib></contrib-group><pub-date pub-type="collection"><year>2023</year></pub-date><pub-date pub-type="epub"><year>2023</year><month>5</month><day>6</day></pub-date><pub-history><event event-type="pmc-last-change"><date iso-8601-date="2024-02-02 11:22:33.456"><year>2024</year><month>2</month><day>2</day></date></event></pub-history><permissions><copyright-statement>Source copyright</copyright-statement><license xlink:href="http://creativecommons.org/licenses/by/4.0/"><license-p>Unrestricted reuse with attribution.</license-p></license></permissions><abstract><title>Summary</title><p>Abstract only.</p><p>Second paragraph.</p></abstract><custom-meta-group><custom-meta><meta-name>pmc-status-live</meta-name><meta-value>yes</meta-value></custom-meta></custom-meta-group></article-meta></front>{body}</article></metadata></record></GetRecord></OAI-PMH>"#).into_bytes()
}
#[tokio::test]
async fn pmc_selection_currency_rights_and_saved_citation() {
    for bad in ["PMC12345", "pmc:12345", "pmc:PMC0123", "pmc:PMC123.0", "pmc:PMC1.2.3", "pmc:PMC123#x", "pmc:PMC1?x"] { assert!(Wanted::parse(bad).is_err()); }
    let wanted = Wanted::parse("pmc:PMC12345.2").unwrap();
    let raw = fixture("pmc_fm", ""); let selected = metadata::parse(&raw, &wanted, "pmc_fm").unwrap();
    assert_eq!(selected.meta.article_instance.as_deref(), Some("67890"));
    assert_eq!(selected.record.dates["publication_collection"].precision, "year");
    assert_eq!(selected.record.dates["pmc-last-change"].precision, "literal");
    assert_eq!(selected.record.content_state, ScholarlyContentState::AbstractOnly);
    assert_eq!(selected.record.abstract_text.as_deref(), Some("Summary\n\nAbstract only.\n\nSecond paragraph."));
    assert!(full_permission(&selected.meta, &selected.record).is_ok());
    let full = metadata::parse(&fixture("pmc", "<body><p>Real supplied body.</p></body>"), &wanted, "pmc").unwrap();
    assert!(selected.meta.same_selection(&full.meta));
    assert!(matches!(metadata::parse(&raw, &Wanted::parse("pmc:PMC12345.1").unwrap(), "pmc_fm"), Err(PmcError::VersionUnavailable)));
    let mut stamp = wanted.clone(); stamp.expected_datestamp = Some("2024-02-04".into());
    assert!(matches!(metadata::parse(&raw, &stamp, "pmc_fm"), Err(PmcError::SourceChanged)));
    let literal = String::from_utf8(raw.clone()).unwrap();
    for changed in [literal.replace("by/4.0", "by-nc/4.0"), literal.replace("license xlink:", "license specific-use=\"textmining\" xlink:")] {
        let restricted = metadata::parse(changed.as_bytes(), &wanted, "pmc_fm").unwrap();
        assert!(!restricted.meta.full_text_eligible); assert!(matches!(full_permission(&restricted.meta, &restricted.record), Err(PmcError::ReuseNotEstablished)));
    }
    let embargo = literal.replace("<meta-value>yes</meta-value>", "<meta-value>no</meta-value>");
    let blocked = metadata::parse(embargo.as_bytes(), &wanted, "pmc_fm").unwrap(); assert_eq!(blocked.record.content_state, ScholarlyContentState::Unavailable);
    assert!(full_permission(&blocked.meta, &blocked.record).is_err());
    assert!(matches!(metadata::parse(literal.replace("<article-version article-version-type=\"pmc-version\">2", "<article-version article-version-type=\"pmc-version\">3").as_bytes(), &wanted, "pmc_fm"), Err(PmcError::IdentityMismatch)));
    for malformed in [b"<unclosed".as_slice(), b"<!DOCTYPE x [<!ENTITY test 'not allowed'>]><x>&test;</x>".as_slice()] { assert!(metadata::parse(malformed, &wanted, "pmc_fm").is_err()); }
    let altered = metadata::parse(literal.replace("Source title", "Different title").as_bytes(), &wanted, "pmc_fm").unwrap(); assert!(!selected.meta.same_selection(&altered.meta));
    let temp = tempfile::tempdir().unwrap(); let mut config = crate::config::Config::default(); config.data_dir = temp.path().join("data"); let engine = Engine::new(config).await.unwrap();
    let original = engine.store.put_bytes(&raw, "application/xml", "pmc_oai_metadata").await.unwrap();
    let response = |time: &str| Response { bytes: raw.clone(), url: wanted.url("pmc_fm").into(), status: 200, observed_at: time.into() };
    let first = engine.save_pmc_metadata(response("2026-01-02T03:04:05Z"), original.clone(), selected).await.unwrap();
    let again = engine.save_pmc_metadata(response("2026-02-02T03:04:05Z"), original, metadata::parse(&raw, &wanted, "pmc_fm").unwrap()).await.unwrap();
    assert_eq!(first.id, again.id); assert_eq!(first.source.retrieved_at, again.source.retrieved_at);
    assert_eq!(engine.store.bytes(&first.source.original).await.unwrap(), raw);
    let citation = engine.citation(&first.id, "csl").await.unwrap(); let csl: Value = serde_json::from_str(citation["text"].as_str().unwrap()).unwrap();
    assert_eq!(csl["version"], "2"); assert_eq!(csl["issued"]["date-parts"][0], json!([2023,5,6])); assert_eq!(csl["author"][0]["family"], "First"); assert_eq!(csl["author"][1]["literal"], "Literal group");
}
#[test]
fn pmc_jats_structures_and_explicit_unsupported_cases() {
    let wanted = Wanted::parse("pmc:PMC12345.2").unwrap();
    let body = r#"<body><sec id="s1"><title>Section one</title><p>Before <xref ref-type="bibr" rid="r1">[β1]</xref> and <inline-formula><tex-math><![CDATA[x_{i} + 1]]></tex-math></inline-formula>.<disp-formula id="e1"><tex-math><![CDATA[a^2+b^2=c^2]]></tex-math><label>(1)</label></disp-formula>After.</p><disp-quote><p>Supplied quote.</p></disp-quote><p>Citation <sup><xref ref-type="bibr" rid="r1">[β1]</xref></sup>.</p><table-wrap id="t1"><label>Table 1</label><caption><title>Cell boundaries</title><p>Supplied qualification.</p></caption><table><thead><tr><th colspan="2">Header</th></tr></thead><tbody><tr><td rowspan="2"><p>A<br/>B</p><list list-type="bullet"><list-item>C</list-item></list></td><td>7</td></tr><tr><td>8</td></tr></tbody></table><table-wrap-foot><fn><p>Footnote stays.</p></fn></table-wrap-foot></table-wrap><list list-type="order"><list-item><p>First item.</p></list-item><list-item><p>Second item.</p></list-item></list><fig id="f1"><label>Figure 1</label><caption><title>Caption stays.</title></caption><graphic xlink:href="relative-figure.jpg"/></fig><p>Unresolved <xref ref-type="bibr" rid="missing">[9]</xref>, empty <xref ref-type="bibr" rid="r1"/>, and <inline-formula><mml:math><mml:mfrac><mml:mi>a</mml:mi><mml:mi>b</mml:mi></mml:mfrac></mml:math></inline-formula>.</p><code language="rust">let x = 1;
  // exact code</code></sec><table-wrap><table><tgroup cols="2"><tbody><row><entry>Not guessed</entry></row></tbody></tgroup></table></table-wrap><chem-struct-wrap>Unsupported structure</chem-struct-wrap></body><back><ref-list><title>References</title><ref id="r1"><label>β1</label><mixed-citation><name><surname>Family</surname><given-names>A</given-names></name><string-name>Literal Name</string-name> (<year>2023</year>) <article-title>Source reference.</article-title><pub-id pub-id-type="doi">10.1234/reference</pub-id></mixed-citation></ref></ref-list></back>"#;
    let raw = fixture("pmc", body); let selected = metadata::parse(&raw, &wanted, "pmc").unwrap(); let parsed = jats::parse(&raw, &selected).unwrap();
    assert!(parsed.blocks.iter().any(|b| matches!(&b.content, Content::Math { text } if text == "a^2+b^2=c^2")));
    assert!(parsed.blocks.iter().any(|b| matches!(&b.content, Content::Paragraph { text } if text == "After.")));
    let rows = parsed.blocks.iter().find_map(|b| if let Content::Table { rows } = &b.content {Some(rows)} else {None}).unwrap();
    assert_eq!(rows[0][0].col_span,2); assert!(rows[0][0].header); assert_eq!(rows[1][0].row_span,2); assert_eq!(rows[1][0].text,"A\nB\nC"); assert_eq!(rows[2][0].text,"8");
    assert!(parsed.blocks.iter().any(|b| matches!(&b.content, Content::Code { text,.. } if text == "let x = 1;\n  // exact code")));
    let full_text = parsed.blocks.iter().map(|b| b.content.text()).collect::<Vec<_>>().join("\n");
    for required in ["[β1]", "β1. Family A Literal Name", "Supplied quote.", "Citation [β1].", "Caption stays.", "Supplied qualification.", "Footnote stays.", "[9]", "Source required"] { assert!(full_text.contains(required), "{required}"); }
    assert_eq!(parsed.metadata["jats"]["references"][0]["label"], "β1"); assert_eq!(parsed.metadata["jats"]["xrefs"][0]["resolved"], true);
    assert_eq!(parsed.metadata["jats"]["partial"], true); assert_eq!(parsed.metadata["jats"]["external_objects"][0]["fetched"], false);
    assert!(parsed.warnings.iter().any(|w| w.code == "jats_xref_unresolved"));
    for block in parsed.metadata["jats"]["blocks"].as_object().unwrap().values() {
        let start = block["source_range"][0].as_u64().unwrap() as usize; let end = block["source_range"][1].as_u64().unwrap() as usize; assert_eq!(raw[start], b'<'); assert!(end <= raw.len());
    }
    let invalid = fixture("pmc", "<body><p>Body.</p><table><tr><td colspan=\"0\">No guessed span</td></tr></table></body>");
    let p = jats::parse(&invalid, &metadata::parse(&invalid, &wanted, "pmc").unwrap()).unwrap(); assert!(!p.blocks.iter().any(|b| matches!(b.content, Content::Table {..}))); assert_eq!(p.metadata["jats"]["partial"], true);
    let empty = fixture("pmc", "<body><sec><title>Heading only</title></sec></body>"); assert!(matches!(jats::parse(&empty, &metadata::parse(&empty, &wanted, "pmc").unwrap()), Err(PmcError::BodyUnavailable)));
}
