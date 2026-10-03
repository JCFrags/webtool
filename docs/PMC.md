# Explicit PMC structured-paper selection

This route uses PMC OAI-PMH only. It does not implement Europe PMC REST, search,
a provider fallback, PDF retrieval, or associated-file downloads. Ordinary URL
reading and generic XML ingestion are unchanged.

## Select, inspect, and read

```sh
webtool scholar pmc pmc:PMC3974642
webtool scholar pmc pmc:PMC3974642.1 --full-text --expected-datestamp 2014-04-08
webtool --format json scholar pmc pmc:PMC3974642.1 --full-text
webtool read SAVED_ID
webtool find SAVED_ID 'library size'
webtool extract SAVED_ID outline
webtool extract SAVED_ID tables
webtool extract SAVED_ID metadata
webtool cite SAVED_ID --as csl
webtool export SAVED_ID --kind original --output paper.xml
```

The default saves selected front matter and abstracts, not a paper body. Inspect
the result before requesting `--full-text`. Use `--refresh` for an explicit new
source observation. Saved-ID reads, find, extract, citations, and exports do not
contact PMC. Attach a saved snapshot with `library add NAME SAVED_ID`.

The HTTP route is `POST /v1/scholarly/pmc`:

```json
{"id":"pmc:PMC3974642.1","full_text":true,"refresh":false,"expected_datestamp":"2014-04-08"}
```

`full_text` and `refresh` default to false. `expected_datestamp` is optional. The
response uses the existing `ScholarlyResponse`. Check the record's `content_state`
and nullable `full_text_error`, not HTTP success alone. A known full-text failure
returns usable saved metadata with that error. The CLI prints the result and
returns a nonzero exit code. A partial JATS read has `full_text_read` plus
`snapshot.partial`, warnings, and source-required markers. It is not complete
formula, media, or bibliography fidelity.

## Selected identity and source currency

IDs must use `pmc:PMCdigits`, optionally followed by `.N`. A suffix asserts the
version delivered by the numeric OAI identifier. It does not retrieve an arbitrary
historical version. A mismatch fails with `pmc_version_unavailable`. This route
does not call the ID Converter or treat a PMC Article Instance ID as a history API.

Front matter must agree with the requested OAI identifier and PMCID. Supplied
`pmcid-ver`, `pmc-version`, and `pmcaiid` are retained and cross-checked. Missing
version or instance fields permit metadata only. Full-text acceptance requires a
complete supplied selection. The service compares the full record's selection,
OAI datestamp, and exact front-element SHA-256 with the inspected metadata. Changed
front matter fails with `pmc_source_changed`. It does not silently replace the
selected snapshot. `--expected-datestamp` asserts the exact OAI source datestamp.

These values have different meanings:

- `dates.oai_datestamp` is source-record currency, not publication or observation.
- `pmc.selection.oai_response_date` is the provider's response date.
- `pmcid-ver` and `pmc-version` describe the delivered article version.
- `pmcaiid` identifies the supplied article instance.
- Publication, submission, acceptance, release, and `pmc-last-change` values stay
  separate with their supplied precision. An unzoned last-change timestamp stays
  a literal. No timezone is invented.
- `observed_at`, provenance observation times, and cache age describe retained
  observations. They are not a later validation or a claim that this is newest.

The metadata cache lasts one day. The full-text cache binds the selected front
hash and datestamp as well as identity. Refresh does not rewrite old saved IDs.
The selected source's article type is an attributed status claim. This is not an
exhaustive correction, retraction, or journal-version search.

## Item-specific reuse and originals

The `pmc-open` set, open-access labels, free access, and text-mining availability
are not sufficient reuse evidence. Full text requires one unscoped article-level
license with an explicit supported URL: CC0 1.0, CC BY 3.0/4.0, or CC BY-SA
3.0/4.0. HTTP/HTTPS license aliases are normalized for comparison. Supplied ALI
license references must agree. Missing, conflicting, scoped, or other terms stay
unknown. Supplied suppression, embargo, and unavailable flags block full text.

Retain attribution, copyright, license notices, and applicable share-alike terms.
Third-party material can have separate restrictions. The route does not fetch
associated figures, equation images, supplements, or other files.

Response-body bytes after HTTP content decompression are stored before XML parsing.
No character or XML normalization changes these retained bytes. Metadata
uses a `pmc_oai_metadata` artifact. Accepted full text uses `pmc_oai_jats` as its
original and retains the front-matter artifact under PMC provenance. Both contain
the OAI envelope, not a normalized reconstruction. The original export and
metadata artifact hashes support source comparison. A shared library attaches
these immutable saved documents, not an external publisher page.

## Narrow JATS reader

`pmc-jats/1` walks supplied abstracts, body, back matter, and floats in source
order. It retains headings, paragraphs, lists, quotations, code, captions,
footnotes, and supported bibliography records. Cross-reference labels are copied
from the source. Unique target IDs resolve to enclosing XML ranges. Missing or
ambiguous targets, empty labels, and unrepresented references make the read
partial. No bibliography number or missing reference is invented.

Supported HTML-model tables retain cell values, line boundaries, header flags,
and positive bounded row/column spans. Adjacent paragraph/list wrappers share
one line boundary in a cell. CALS tables, nested tables, and invalid spans use
source-required markers rather than guessed cell matrices. This is not a table
schema validator or layout renderer.

Supplied TeX, plain formula content, and mathematical alt text are retained.
No TeX is evaluated. Graphic-only formulas, unannotated MathML, ambiguous formula
alternatives, associated files, and unsupported elements have explicit
source-required markers and ranges. The marker is not recovered notation.

Blocks use `Locator::Derived`. `metadata.jats.blocks`, references, cross-references,
formulas, external objects, and unsupported records carry half-open UTF-8 byte
ranges in the retained OAI XML. These ranges enclose source elements. They are
not exact normalized-text spans. Replaying a range can require namespace bindings
inherited from the envelope. Distinguish duplicate captions by their ranges, not
text alone. Inline styles and structured reference field boundaries are normalized.
Source files remain unchanged. Do not describe this reader as general JATS
fidelity or publisher-equivalent rendering.

Saved PMC citations use retained front matter and the delivered version/instance.
CSL and BibTeX use the selected publication date when supplied. They do not use
OAI currency as publication time or re-fetch a DOI record. Saved PMC RIS is
unsupported. Existing DOI and arXiv citation behavior remains unchanged.

## Resource limits and verification boundary

The service owns one reusable PMC client with one active connection and at least
one second of pacing. It shares the scholarly transport, which sets
`reqwest::retry::never()`, refuses redirects, and honors slower provider limits
and cooldowns. PMC requests compression. The configured deadline includes
queueing, metadata, full text, parsing, and persistence. Metadata is capped at the
smaller of the configured source limit and 4 MiB. Full JATS is capped at the
smaller of that limit and 16 MiB. XML is UTF-8 only, with at most 200,000 nodes,
depth 128, and 10,000 reading blocks. DTDs and external entities are not processed.

Use the coordinated normal CLI/server build before the two focused synthetic
PMC checks. Follow the shared-target lock and root-mtime rules in
[SCHOLARLY.md](SCHOLARLY.md). The focused test selector is `scholarly::pmc`.
Synthetic table and TeX cases do not prove their fidelity on a live publisher
paper. A graphic-only formula marker must not be reported as equation recovery.

## Primary sources

- [PMC OAI service](https://pmc.ncbi.nlm.nih.gov/tools/oai/), including automated access and pacing
- [PMC copyright](https://pmc.ncbi.nlm.nih.gov/about/copyright/)
- [PMC text-mining access](https://pmc.ncbi.nlm.nih.gov/tools/textmining/)
- [PMC ID Converter](https://pmc.ncbi.nlm.nih.gov/tools/id-converter-api/), including versions and article instances
- [JATS 1.4 archiving tag library](https://jats.nlm.nih.gov/archiving/tag-library/1.4/)
