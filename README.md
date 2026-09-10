# webtool

A text-based research CLI and shared Rust server for a small, trusted group.
Search the web, read sources, extract structures, and save documents in shared libraries.
There is no TUI, alternate screen, browser-control interface, or default LLM workflow.

## Delivery status

The normal server now includes native PDF text reading through pinned Xberg.
A two-page public PDF passed URL read, local ingestion, page-location checks,
phrase finding, and byte-identical original export. Default HTML nested code and
tables also passed the preceding bounded milestone. This is not a full V1 release.

One short YouTube video also passed English caption read/find/export through yt-dlp.
Lightpanda 0.3.6 passed local JavaScript and public quote-page capture.
OCR, Office formats, Chromium, fastCRW, and Docker remain unverified. No model
downloads or full Rust tests were run. See [STATUS.md](docs/STATUS.md) for exact
commands, evidence, and limits. Historical archive logs are not current results.

## What is implemented in source

| Area | Current implementation |
|---|---|
| CLI | Ordinary commands, text and Markdown output, JSON, JSONL batches, stderr warnings, meaningful failure exits |
| Shared service | Axum API, concurrent requests, bounded processing, one server-local SQLite database |
| Search | Native DuckDuckGo, Brave, Startpage, and Yahoo adapters, result deduplication, reciprocal-rank merging |
| Reading | HTTP/HTML, pinned GitHub files and immediate directories, original-byte retention, explicit selection |
| Libraries | Shared named collections, references to saved documents, attributed notes and tags |
| Local search | SQLite FTS5 keyword search, literal matching, optional regex matching |
| Extraction | Tables, code, links, images, metadata, outlines, CSS selections, JSON pointers |
| Imports | Files and stdin uploaded from the client, including native text, structured data, feed, caption, and notebook readers |
| Crawling | Incremental library attachment and progress, bounded same-origin HTTP jobs, robots, cancellation and restart status |
| Browser helpers | Explicit Lightpanda or Chromium DOM capture, plus an experimental fastCRW integration |
| Documents | Default Xberg native PDF text with reported pages and labeled supplemental tables; Office formats unverified |
| Media | Configured yt-dlp: provided/automatic YouTube captions, exact language selection, timestamped storage and original export; no media download |
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

## arXiv papers and saved citations (not yet live-verified)

Auto reading recognizes arxiv.org `/abs/` and `/pdf/` URLs, modern/legacy IDs,
explicit `vN`, and optional `.pdf`. It queries the official Atom API, validates
identity/version, then retrieves only the pinned PDF through the existing Xberg
reader. Explicit HTTP, CSS, and browser choices are not overridden.

**Current blocker:** the official API returned HTTP 500 twice for
`cond-mat/0207270v1`. No PDF or saved-paper citation workflow was verified.
These are the intended commands to retry when that upstream failure clears:

```sh
webtool read https://arxiv.org/abs/cond-mat/0207270v1 --library papers --refresh
webtool cite DOCUMENT_ID --as bibtex
webtool cite DOCUMENT_ID --as csl
```

The PDF remains the original export. Metadata includes ordered literal authors,
abstract, categories, submitted/updated dates, available DOI/journal reference,
requested and resolved identity, plus the retained API-response artifact and
provenance. Metadata/abstract alone is never accepted as full text. An unavailable
requested version is not replaced with latest. Citation generation from a saved
ID is offline; existing DOI BibTeX/RIS/CSL behavior remains. Saved arXiv citations
identify the preprint and version, not an associated journal publication. Names
are not split into surnames. Citation dates use the returned version's updated
date when parseable; missing fields are omitted. Bibliographic escaping is literal,
not a TeX/math interpretation layer.

HTTP reads to arXiv hosts share one process-wide connection gate and a three-second
delay after each response/error. This covers all users of this server; coordinate
any other machines/processes separately. Normal Auto paper reads cache for one day;
`--refresh` bypasses that document cache. No automatic retries or fallback provider.
Follow the [arXiv API terms](https://info.arxiv.org/help/api/tou.html): metadata is
CC0, but e-print redistribution requires a suitable license or copyright-holder
permission. Availability through this tool does not grant redistribution rights.

## GitHub repositories, files, and directories

Default `read` uses GitHub APIs for public repository roots and blob/tree URLs.
An explicit `--renderer http`, CSS selector, or browser choice bypasses native
routing. A root still reads only its pinned README, with a visible scope warning.

```sh
webtool read https://github.com/JCFrags/webtool
webtool read https://github.com/JCFrags/webtool/tree/main/crates/engine/src --refresh
# Follow a displayed immutable /blob/COMMIT/... link to read a file.
webtool read https://github.com/JCFrags/webtool/blob/3d7df722944aafe26b6f3b1311649fe46b8b2060/crates/engine/src/sources.rs --refresh
```

Mutable refs resolve to commit SHAs before any file/tree retrieval. File bytes
use the real filename and existing readers; commit, path, and requested ref are
in metadata. Source text/code keeps its original whitespace and line locations.
Directories show immediate entries only, with pinned file/directory links. Their
original artifact is the API JSON, while listing blocks are explicitly derived.
Truncated or incomplete trees remain visible; a directory read does not read files.

Slash-containing refs are resolved by checking at most eight possible ref/path
splits. Ambiguous URLs error rather than selecting a branch. Use a full commit
SHA or encode reference slashes (`feature%2Fname`) to give an explicit boundary.
Paths are UTF-8 percent-decoded once and limited to 16 components. Symlinks and
submodules are explicitly unsupported. Public API rate limits and response byte
limits apply; base64 API overhead counts toward the response limit.

Root README text remains unchanged. Supplemental links pin common inline and
reference-definition destinations; external links remain external. This is not
full CommonMark: complex links and directory links without a trailing slash need
further work. One directory-to-Rust-file workflow is verified, not every ref/error
case. See docs/STATUS.md for exact evidence and export comparison commands.

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

Crawling uses fresh HTTP reads and stays within the seed origin. Every redirect
hop must stay in that origin and pass robots rules. Completed pages are attached
to the selected library and job progress is saved immediately, without waiting for
slow siblings. Saved reads, doctor, and library search remain usable during a crawl.

```sh
webtool library items research
webtool search 'distinctive phrase' --library research
```

`visited` counts completed attempts, including failures. `failed` counts unsuccessful
completed attempts, separately from document extraction warnings. Pending/cancelled
requests are not counted as visited. The page budget reserves attempts before
launch, so failures cannot create extra retries beyond the limit. Saved IDs are
unique documents, not a promise that every discovered URL was read.

Depth zero is the seed. Same-depth batches preserve traversal semantics; query
order and repeated parameters are not sorted or removed during deduplication.
Fragments are removed. Page/depth/discovery limits and robots exclusions are
reported as warnings. `partial` can reflect scope/extraction warnings with zero
retrieval failures. No usable documents from attempted reads means `failed`;
`jobs JOB_ID --wait` prints the final record then exits nonzero for failed or
interrupted jobs. Progress stays on stderr; JSON stdout remains machine-readable.

Basic robots directives and per-origin delays are implemented, not full RFC conformance.
`jobs JOB_ID --cancel` stops pending work without removing saved documents. Ctrl-C
while waiting only stops the client wait. A running job becomes `interrupted`
after an unclean server restart; queued jobs are rescheduled. The frontier is not
persisted for exact continuation. This is not a complete-site archive.
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

The verified helper is the official Lightpanda **0.3.6** Linux x86_64 release.
The existing binary matched its published SHA-256; no installation was needed.
Set its path in the ignored `runtime/media-config.toml`, preserving yt-dlp settings:

```toml
lightpanda_path = "/home/mainpc/.local/bin/lightpanda"
browser_wait_ms = 2000 # existing key: edit it, do not add a duplicate
```

```sh
./target/debug/webtoold --config runtime/media-config.toml
webtool read https://quotes.toscrape.com/js/ --renderer lightpanda --refresh
```

Do not start a second server. See docs/STATUS.md for setup and the local fixture.
The adapter disables telemetry and uses `fetch --dump html --json`, requesting
`--wait-until done` followed by `--wait-script "document.readyState === 'complete'"`.
The wait budget bounds those conditions, not a guarantee of application completeness.
Persistent background activity can time out. No other browser is tried automatically.

The JSON envelope supplies actual HTTP status and final URL when available. Missing
status remains null; missing final URL keeps the requested URL with a warning.
Only decoded DOM content is retained as `rendered_dom`, not the JSON envelope or
original HTTP bytes. Read warnings, original-export messages, and the download
filename identify the DOM snapshot. HTML selectors refer to that retained snapshot;
ambiguous text fragments remain derived. DOM `<base>` links are resolved without
rewriting retained bytes. Helper stderr remains separate and visible as warnings.

One helper process per request uses existing concurrency, stdout/stderr byte caps,
deadlines and process-group cleanup. The stdout limit includes JSON overhead, and
subresource responses also use the configured byte cap. Capture behavior and HTML
parser revision participate in cache identity. Changing a binary in place requires
`--refresh`. This is not general JavaScript/browser compatibility validation.
Chromium remains unverified, with its existing uncertain navigation/timed capture.

The `crw-browser` feature adds an experimental native fastCRW adapter.
It requires explicit `crw_renderer` configuration and `--renderer crw`.
It is not used silently when Lightpanda or Chromium is selected.
Its configuration and API compatibility still require compilation and integration tests.

## YouTube captions

`read` defaults to `--renderer auto`: YouTube watch/youtu.be URLs use captions;
other URLs keep HTTP reading. `--renderer captions` forces caption selection.
Explicit `--renderer http` or `--selector` keeps HTML selection; no hidden HTML
fallback occurs when caption retrieval fails. The existing `media` command also
uses the caption path. `--language` defaults to `en` and must match a track exactly.
Provided subtitles are preferred; automatic captions are labeled. No translation,
transcription, browser cookies, login sessions, proxies, or audio/video downloads.

Install the official PyPI distribution (using an existing uv installation):

```sh
uv tool install 'yt-dlp[default]' --index-url https://pypi.org/simple
```

The default extra includes yt-dlp-ejs. Official [EJS requirements](https://github.com/yt-dlp/yt-dlp/wiki/EJS)
currently support Node >=22; this host already has Node 24.18.0. No additional
runtime or ffmpeg was installed. Configure local, ignored server settings:

```toml
ytdlp_path = "/path/to/yt-dlp"
ytdlp_js_runtime = "node:/path/to/node"
```

```sh
webtool read 'https://youtu.be/jNQXAC9IVRw' --language en --library research --refresh
webtool media 'https://www.youtube.com/watch?v=jNQXAC9IVRw' --language en
# Use the saved ID for find and original export:
webtool find "$DOC_ID" 'elephants'
webtool export "$DOC_ID" --kind original --output captions.vtt
```

The helper retrieves one untranslated VTT track with its own source metadata and
request headers. Temporary signed metadata and downloads are cleaned afterward;
only verbatim caption bytes and stable video metadata are retained. Cache keys
include language. Missing helper, blocked source, absent track, and malformed
captions have separate errors. `doctor` checks the local executable without
contacting YouTube. Helper diagnostics remain on stderr, with URLs redacted.

Verified: yt-dlp 2026.08.19, EJS 0.8.0, Node v24.18.0; “Me at the zoo” produced six
provided English cues and an identical 440-byte VTT export. A nonfatal missing
impersonation-target warning remained visible; no extra dependency was installed.
Automatic captions, other languages, and blocked-source paths remain unverified.
See docs/STATUS.md for exact local commands and evidence.

## Bibliography

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
