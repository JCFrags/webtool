# Explicit scholarly discovery and metadata

Ordinary web search remains separate. Scholarly operations use one explicit
provider. They do not call an LLM, fetch search-result paper bodies, merge
versions by DOI or title, or rotate providers after a failure.

## CLI

```sh
webtool scholar search arxiv 'all:graph neural networks' --limit 5
webtool scholar search openalex 'graph neural networks' --limit 5
webtool scholar doi 10.1234/example
webtool scholar arxiv 2401.12345v2
webtool scholar arxiv 2401.12345v2 --full-text
webtool scholar pmc pmc:PMC3974642.1
webtool scholar pmc pmc:PMC3974642.1 --full-text --expected-datestamp 2014-04-08
```

Search defaults to five results and accepts limits from 1 to 20. arXiv queries
use the official API query syntax. OpenAlex uses its works search. Crossref is
available for a selected DOI only, not discovery or cross-provider enrichment.
All commands support `--refresh` and the global `--format json` option.

Each successful result supplies a saved document ID. Use `read ID`,
`extract ID metadata`, or `export ID --kind original --output FILE` without
another provider request. Shared libraries can attach the saved ID with
`library add NAME ID`. An exact-version arXiv metadata record also supports
`cite ID --as bibtex` or `cite ID --as csl` offline. That citation remains a
preprint citation, not a substituted associated journal citation.

`scholar arxiv` requires a literal modern or legacy identifier with an explicit
`vN`. The default saves selected metadata, abstract, and license evidence.
`--full-text` fetches the pinned PDF only when the selected version reports a
supported item-specific reuse license. The current conservative allowlist is
CC0 1.0, CC BY 3.0/4.0, and CC BY-SA 3.0/4.0 at their official license URLs.
Attribution, license notices, and applicable share-alike terms still apply.
The arXiv distribution license is not permission for this service to
redistribute the paper. Missing, ambiguous, or other terms stay unknown.
Open access alone does not establish reuse rights.

If reuse is not established, the result contains metadata and links only. No
PDF is fetched. If an explicitly permitted PDF cannot be fetched or read, the
metadata remains usable and `full_text_error` describes the failure. The CLI
prints the result and returns a failure exit code for either requested
full-text failure. Check `content_state`, not just HTTP success. A
`full_text_read` result supplies the saved paper ID for `read ID`.

Ordinary automatic arXiv full-text reads enforce the same reuse check. They
return a rights error instead of silently substituting the abstract. Earlier
saved snapshots and offline citations remain unchanged and readable.

## HTTP contract

All four operations are explicit POST routes:

| Route | Request |
| --- | --- |
| `/v1/scholarly/search` | `provider` (`arxiv` or `openalex`), `query`, optional `limit` and `refresh` |
| `/v1/scholarly/doi` | `doi`, optional `refresh` |
| `/v1/scholarly/arxiv` | exact-version `id`, optional `full_text` and `refresh` |
| `/v1/scholarly/pmc` | namespaced `id`, optional `full_text`, `refresh`, and `expected_datestamp` |

The response contains `snapshot`, `document_id`, `cached`, `age_seconds`,
`warnings`, and nullable `full_text_error`. The same snapshot is stored in
`Document.metadata.scholarly`. Atom, JSON, or selected arXiv HTML is retained
as the original artifact before normalization. A permitted full-text record
has the PDF original and retains the metadata artifact in arXiv provenance.
Source HTTP status and exact version remain explicit. The PMC route retains OAI
front matter and, after reuse/selection checks, full JATS. Its `.N` suffix asserts
the delivered version, not arbitrary historical retrieval. See [PMC.md](PMC.md)
for source-currency fields, rights gates, structures, and fidelity limits.

Provider queries and selected metadata are cached for one day. `observed_at`
and `age_seconds` describe the retained observation, not a later cache
validation. Identical accepted responses return the existing immutable
snapshot. `--refresh` does not rewrite old snapshots or relabel their age.

Records retain namespaced identifiers, literal author order, supplied name
parts and ORCID, separate dates and date precision, location identifiers,
version classes, license evidence, and content state. A year-only Crossref
date remains a year. OpenAlex publication date, publication year, created
date, and updated date remain distinct. OpenAlex authors may be truncated
by the provider. Invalid abstract index gaps or conflicts produce a warning,
not a guessed reconstructed abstract. arXiv API entries can have unversioned
IDs beside versioned links. Explicit versions must agree. If no version is
reported, discovery does not invent one.

Crossref `update-to` retains its direction, related DOI, source, update type,
date, and record ID. A notice can point to an original work. That relation does
not automatically label the selected DOI as retracted. Status coverage is the
selected singleton record only. Empty update metadata is not proof that no
retraction exists. OpenAlex `is_retracted` is an attributed provider claim,
not an independent status check. No exhaustive notice search is implemented.

## Limits and failure behavior

The service owns reusable provider clients. Each provider has one active
connection and conservative pacing. arXiv API discovery and selected HTML
share the existing process-wide arXiv gate, including ordinary reads. The
gate permits one connection and waits at least three seconds after completion
or cancellation. Other metadata providers use at least one second between
requests. Reported slower provider limits and cooldowns are honored.

The configured request deadline includes queueing. Metadata responses are
streamed with a cap of the smaller of the configured source limit and 4 MiB.
An explicitly permitted PDF uses the configured source byte limit. There
are no automatic retries, redirects, mirrors, API keys, accounts, or paid
content requests. A rate or budget limit stops that provider operation.
Required response-shape or identity failures are errors. Invalid individual
results produce an explicitly partial collection when usable results remain.

This service does not implement scholarly full-text search, general PDF license
inference, bulk acquisition, or journal-version substitution. The explicit PMC
route has a separate narrow JATS reader. It preserves supported structures and
reference labels, with derived XML ranges and explicit partial/source-required
warnings. Europe PMC REST remains unimplemented and unverified. Generic XML
reading is not this PMC route and does not gain JATS fidelity from it.

## Focused development checks

Use the normal locked CLI/server build. The focused engine checks use synthetic
provider responses and an isolated store:

```sh
cargo test --locked -p webtool-engine --lib --features documents scholarly
cargo build --locked -p webtool-cli -p webtool-server --features documents
```

For the project's coordinated shared-target workflow, acquire the build lock
before any Cargo command. At lock admission, touch only the current worktree's
existing protocol, engine, and server `src/lib.rs`, CLI `src/main.rs`, and
existing `build.rs` files. Cargo dependency files can contain relative workspace
paths. An unchanged root timestamp can otherwise reuse another worktree's
workspace artifacts. Keep dependency artifacts. Copy the candidate binaries to
the task-owned evidence directory before releasing the lock. Do not clean
another worktree or use its binaries as this source's verification.

The locked reqwest version enables safe protocol-error retries by default.
Scholarly provider clients explicitly use `reqwest::retry::never()` so the
service's no-retry policy also covers those low-level retries.

Use isolated loopback configuration and data for practical checks. Compare saved
metadata with its retained provider response. Check exact versions, author order,
date precision, license evidence, update direction, cache age, and offline
citations. An abstract-only rights result is not a verified full-text extraction.

## Provider sources

- [arXiv API manual](https://info.arxiv.org/help/api/user-manual.html)
- [arXiv API terms](https://info.arxiv.org/help/api/tou.html), including CC0 descriptive metadata and request pacing
- [arXiv reuse policy](https://info.arxiv.org/help/license/reuse.html)
- [OpenAlex authentication](https://help.openalex.org/api/authentication)
- [OpenAlex works](https://help.openalex.org/data/works/attributes/)
- [OpenAlex locations](https://help.openalex.org/data/locations/)
- [Crossref access](https://www.crossref.org/documentation/retrieve-metadata/rest-api/access-and-authentication/)
- [Crossref Retraction Watch metadata](https://www.crossref.org/documentation/retrieve-metadata/retraction-watch/)
