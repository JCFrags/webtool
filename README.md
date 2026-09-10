# webtool

A text-based research CLI and shared Rust server for a small, trusted group.
Search the web, read sources, extract structures, and save documents in shared libraries.
There is no TUI, alternate screen, browser-control interface, or default LLM workflow.

## Delivery status

The normal server now includes native PDF text reading through pinned Xberg.
A two-page public PDF passed URL read, local ingestion, page-location checks,
phrase finding, and byte-identical original export. Default HTML nested code and
tables also passed the preceding bounded milestone. This is not a full V1 release.

OCR, Office formats, browsers, captions, and Docker remain unverified. No model
downloads or full Rust tests were run. See [STATUS.md](docs/STATUS.md) for exact
commands, evidence, and limits. Historical archive logs are not current results.

## What is implemented in source

| Area | Current implementation |
|---|---|
| CLI | Ordinary commands, text and Markdown output, JSON, JSONL batches, stderr warnings, meaningful failure exits |
| Shared service | Axum API, concurrent requests, bounded processing, one server-local SQLite database |
| Search | Native DuckDuckGo, Brave, Startpage, and Yahoo adapters, result deduplication, reciprocal-rank merging |
| Reading | HTTP retrieval, original-byte retention, automatic HTML selection, explicit CSS selection |
| Libraries | Shared named collections, references to saved documents, attributed notes and tags |
| Local search | SQLite FTS5 keyword search, literal matching, optional regex matching |
| Extraction | Tables, code, links, images, metadata, outlines, CSS selections, JSON pointers |
| Imports | Files and stdin uploaded from the client, including native text, structured data, feed, caption, and notebook readers |
| Crawling | Persistent bounded jobs, basic robots rules, same-origin traversal, cancellation, restart status |
| Browser helpers | Explicit Lightpanda or Chromium DOM capture, plus an experimental fastCRW integration |
| Documents | Default Xberg native PDF text with reported pages and labeled supplemental tables; Office formats unverified |
| Media | Optional yt-dlp metadata and existing-caption retrieval, without video download |
| Bibliography | DOI metadata retrieval as BibTeX, RIS, or CSL JSON |
| Exports | Markdown, JSON, retained originals, and selected tables as CSV |

The default packages compile; optional integrations remain unverified.
Some integration boundaries are intentionally marked experimental.

## Build and run

Install a current stable Rust toolchain on a network-connected development machine.
Run these commands from this repository's root.

```sh
cargo build --locked -p webtool-cli -p webtool-server
```

Cargo.lock is committed. Use debug builds during these bounded milestones. Full tests and
optional integration checks remain manual, not prerequisites for this milestone.

Start the server in one terminal.

```sh
./target/debug/webtoold --config config.example.toml
```

Use the CLI from another terminal.

```sh
export PATH="$PWD/target/debug:$PATH"
webtool doctor
webtool library create research --description "Shared research sources"
webtool ingest tests/fixtures/source.md --library research --actor Alice
webtool saved
webtool search "Exact code" --library research
```

Copy a full document ID from the ingest or saved output.

```sh
DOC_ID="replace-with-the-full-64-character-document-id"
webtool read "$DOC_ID"
webtool find "$DOC_ID" "Exact code"
webtool extract "$DOC_ID" code
webtool note "$DOC_ID" --actor Alice --text "Reviewed the original." --tag reviewed
webtool export "$DOC_ID" --kind markdown --output source.md
```

The exact executed bootstrap sequence is recorded in docs/STATUS.md.

## Connect several people

Install only `webtool` on client machines.
Keep the server, database, browser helpers, and document processors on the shared machine.

All connected users have the same access.
Names attribute contributions and do not establish identity or permissions.
The server has no authentication and can reach HTTP services visible to its host.
Do not expose it directly to the public internet.

On a trusted LAN, bind the server deliberately.

```sh
./target/debug/webtoold --bind 0.0.0.0:8420 --data-dir ./data
```

Point each client at that machine.

```sh
export WEBTOOL_SERVER="http://research-server:8420"
webtool library list
```

Run one server process per data directory.
Clients never open the SQLite file or require a shared filesystem.

## Read and search

```sh
webtool search "Rust async cancellation"
webtool read https://example.com
webtool read https://example.com --selector body
webtool read https://example.com --refresh
webtool --format json read https://example.com
webtool --format markdown read https://example.com
webtool find "$DOC_ID" "timeout" --ignore-case
webtool extract "$DOC_ID" links
webtool extract "$DOC_ID" css --expression "main table"
```

A successful URL read is saved automatically.
Adding it to a library creates a reference, not a second document copy.
A refresh that produces identical content can reuse the existing document ID.
Its retained snapshot date remains the original date, while cache freshness is updated separately.

Search snippets are provider output, not verified excerpts from destination pages.
Brave and DuckDuckGo returned links in one bootstrap smoke query; broader availability and relevance remain untested.
There is no automatic semantic reranker, image-search command, or date-filter implementation yet.

## Formats

| Reader | Included scope |
|---|---|
| Text and code | UTF-8 content, preserved whitespace, source line ranges |
| Markdown | Basic headings, paragraphs, fenced code, source line ranges, not a complete CommonMark parser |
| JSON and JSONL | Original text, validated syntax, exact pointer or line references |
| CSV and TSV | Empty fields, zeros, multiline values, ragged rows, no inferred header row |
| RSS and Atom | Feed entry titles, summaries, and links, not full destination articles |
| SRT, WebVTT, SBV | Existing caption cues and integer-millisecond timestamps |
| Jupyter notebooks | Version 4 cells and saved text outputs, no code execution |
| XML | Original XML, with a limited JATS prose reader |
| HTML | Selected content, code, tables, image references, and original-element matching when unambiguous |
| Xberg (default server) | Native PDF text verified on one two-page input. Office, spreadsheet, image, ebook, and email formats are not verified |

Non-UTF-8 HTML and text are rejected rather than silently corrupted.
Image-only scans cannot be read without OCR, which is unavailable in the normal build.
Figure extraction and downloadable figure assets are not complete.
No formula recalculation, notebook execution, video transcription, or document editing is performed.

## Structured output and exports

```sh
webtool --format json extract "$DOC_ID" tables
webtool extract "$DOC_ID" json-pointer --expression /present
webtool export "$DOC_ID" --kind original --output original.bin
webtool export "$DOC_ID" --kind table-csv --table 1 --output table.csv
webtool read "$DOC_ID" --start-block 2 --end-block 5
webtool read "$DOC_ID" --page 4
```

CSV export rejects merged cells rather than discarding their spans.
CSV values are preserved, including strings that spreadsheet software could interpret as formulas.
Existing output files are not replaced unless `--force` is explicitly supplied.

Pipe regular text into a pager when needed.

```sh
webtool read "$DOC_ID" | less
webtool --format jsonl batch urls.txt > results.jsonl
```

Text output escapes terminal control characters.
JSON and original-file exports preserve source content.
Warnings and progress stay on stderr.
Some text commands still print indented JSON instead of a custom table layout.

## Crawl and map

```sh
webtool crawl https://example.com --max-pages 20 --max-depth 2 --library research
webtool jobs
webtool jobs JOB_ID --wait
webtool jobs JOB_ID --cancel
webtool map https://example.com
webtool map https://example.com/sitemap.xml
```

Crawling uses HTTP and stays within the seed origin.
Basic robots directives and per-origin delays are implemented, not full RFC conformance.
A running job becomes `interrupted` after a server restart.
Queued jobs are rescheduled, and previously saved documents remain available.
The crawler does not yet persist its frontier for exact continuation.
Map reads one page or sitemap and does not recursively expand sitemap indexes.

## PDF reading (normal server)

No feature flag or model installation is needed for native PDF text:

```sh
cargo build --locked -p webtool-cli -p webtool-server
./target/debug/webtoold --config config.example.toml
# In another terminal:
./target/debug/webtool read https://sample-files.com/downloads/documents/pdf/fillable-form.pdf --refresh
./target/debug/webtool ingest ./local.pdf
```

`doctor` reports compiled document support separately from unavailable OCR.
The server retains original bytes, reported page numbers, document metadata,
and the full upstream result/envelope. Page content is readable paragraph text,
not a programming-code block; its supplied whitespace is retained. PDF title,
author, dates, page count, and format metadata remain available through extract.

Structured tables remain available via `extract ID tables`. Ordinary and Markdown
reading explicitly label supplemental tables because they may repeat page text.
Xberg cell matrices do not establish original header roles or merged-cell spans;
no such geometry is inferred. Complete upstream table details remain in metadata.
Empty/whitespace-only page objects do not count as successful content. A fully
empty extraction fails with an OCR-availability explanation, not a blank-document
success. A missing-text page is not automatically classified as a scan. Partial
page and upstream warnings remain visible.

Only a small native-text PDF was verified. Scans, complex layouts, tables from
real PDFs, Office formats, and form-field interpretation need separate validation.
No PDF form values are edited and no OCR/model runtime is installed by default.

## Optional integrations (not verified)

### OCR

```sh
cargo check -p webtool-server --features ocr
```

OCR requires native runtime dependencies, configured models, and separate validation.
It is not part of the default server build.
The supplied minimal Docker image is not an OCR deployment recipe.

### Browser helpers

Set installed executable paths in `config.example.toml`.

```toml
lightpanda_path = "/usr/local/bin/lightpanda"
chromium_path = "/usr/bin/chromium"
```

```sh
webtool read https://example.com --renderer lightpanda
webtool read https://example.com --renderer chromium
```

Helper DOM dumps do not reliably expose navigation status or redirect URLs.
The application records this limitation instead of assigning a fabricated status.
A fixed wait budget does not prove that late-loading content is complete.
Browsers currently launch per request rather than through a persistent browser pool.

The `crw-browser` feature adds an experimental native fastCRW adapter.
It requires explicit `crw_renderer` configuration and `--renderer crw`.
It is not used silently when Lightpanda or Chromium is selected.
Its configuration and API compatibility still require compilation and integration tests.

### Captions and media metadata

Set `ytdlp_path` to an installed yt-dlp executable.

```sh
webtool media "https://www.youtube.com/watch?v=VIDEO_ID" --language en --library research
```

The reader requests an existing VTT track in the exact requested language.
It labels automatically generated tracks and errors when no supported track is available.
YouTube may require an additional JavaScript runtime for yt-dlp.
This helper and live caption retrieval were not tested here.

### Bibliography

```sh
webtool cite 10.1038/nphys1170 --as bibtex
webtool cite 10.1038/nphys1170 --as csl
```

This retrieves metadata and does not claim access to a paper's full text.
Bibliography libraries, citation formatting, and Zotero synchronization remain future work.

## Test and deployment files

- `scripts/check.sh` compiles and runs the Rust tests when Cargo is available.
- `scripts/validate_local.py` runs the real SQL migration and package checks using Python's standard library.
- `.github/workflows/ci.yml` defines one cached, locked default build on pull requests and manual dispatch. Tests and optional checks are manual.
- `Dockerfile` and `compose.yaml` provide an unbuilt development deployment.
- `AGENTS.md` lists the next implementation and validation tasks.

```sh
./scripts/check.sh
python3 scripts/validate_local.py --report /tmp/local-validation.json
```

The Python script is development tooling, not an application runtime dependency.
Do not interpret its success as Rust test success.

## Development boundaries

Keep one document model, storage system, operation queue, and public interface.
Prefer library integrations over importing entire upstream servers.
Do not add quality profiles, automatic extractor chains, or a TUI.
Add source format support only with fidelity fixtures and explicit limitations.

See [architecture](docs/ARCHITECTURE.md), [API](docs/API.md), [status](docs/STATUS.md), and [sources](docs/SOURCES.md).
