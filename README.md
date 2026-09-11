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

## Install and connect

Use a current stable Rust toolchain and the system build tools needed by Cargo.
Clone this repository, then use the small local installer. It builds optimized
binaries with the committed lockfile and normal features. It does not use sudo,
edit shell profiles, manage services or download browsers, media helpers or models.
Cargo may download locked Rust dependencies. No published binary release/tag yet.

### Client machine

```sh
cd /path/to/webtool
./scripts/install-local.sh --client-only
# Optional destination: --bin-dir /absolute/user-owned/bin
export PATH="$HOME/.local/bin:$PATH" # current shell only
cd /tmp
webtool connect http://research-server:8420
webtool config show
webtool doctor
webtool library list
```

Client-only builds/installations select only `webtool-cli`, not the engine or
server. The client uploads files through HTTP. It never creates server storage,
opens the server SQLite file or requires document/browser/media helpers.

`connect` validates and saves an absolute HTTP(S) endpoint without contacting it.
URLs may include a reverse-proxy base path, but not credentials, queries or
fragments. It saves `server` in `$XDG_CONFIG_HOME/webtool/client.toml`, falling
back to `$HOME/.config/webtool/client.toml` on Linux. An unset, empty or relative
XDG_CONFIG_HOME uses the HOME fallback. Existing unrelated TOML settings are
preserved, though comments/formatting may be rewritten. Updates use a synced
same-directory temporary file and atomic replacement. Malformed files and target
symlinks are refused rather than overwritten.

Endpoint precedence, highest first:
1. `--server URL`
2. `WEBTOOL_SERVER`
3. Saved client `server` setting
4. `http://127.0.0.1:8420`

`webtool config show` reports the effective endpoint, configuration path and
selection source without network access. `--format json` works for both new
commands. An override does not change the saved setting. A disconnected server
produces an actionable error; no command automatically starts another server.
`doctor` also reports the selected endpoint and server build commit, or `unknown`
for builds without provenance/older servers. A dirty source build is labeled.

### Host machine

The default installer installs both `webtool` and `webtoold` into `~/.local/bin`.
Existing unrelated executables are refused. Per-binary checksum receipts allow
later replacement only when the installed file still matches the prior install.
Do not delete those receipts to force replacement. Use a different bin directory
if a name is occupied. Binaries are replaced by rename, not overwritten in place;
running processes are not restarted. Keep a copy of previous binaries/receipts
when planning a rollback. No automatic update mechanism is installed.

```sh
cd /path/to/webtool
./scripts/install-local.sh
# Select an absolute config path and the intended absolute data directory.
# Stop your existing server gracefully before starting its replacement.
cd /tmp
/home/USER/.local/bin/webtoold \
  --config /absolute/path/server.toml \
  --data-dir /absolute/path/existing-data
```

For a new host, copy `config.example.toml` to the selected config path and edit it
before startup. Keep loopback binding unless trusted-LAN access is intentional.
Configure optional helpers only on the host. Select the existing data directory
for an existing deployment; do not copy examples into a different empty database.
The server prints its effective listening address, resolved absolute data directory,
config path and build commit. An occupied bind address fails before opening storage.

Relative paths retain their historical working-directory meaning, not the config
file's or binary's directory. A relative data directory produces a warning. Use
absolute config/data paths and absolute configured helper paths for installed
startup. No checkout-local working directory is required with these explicit paths.

This project's preserved deployment uses:

```sh
cd /tmp
/home/mainpc/.local/bin/webtoold \
  --config /home/mainpc/Projects/webtool/runtime/media-config.toml \
  --data-dir /home/mainpc/Projects/webtool/data
```

Start only one process for this data directory. The command does not install a
system service. No systemd, Docker, firewall, TLS, account or automatic-update
management is provided.

For explicit trusted-LAN binding, add `--bind 0.0.0.0:8420` to the host command,
then connect clients to `http://HOST_LAN_ADDRESS:8420`, not `0.0.0.0`. This does not
open ports or change firewall rules. Libraries are shared by every connected user.
Contributor names are attribution, not authentication. The service can reach HTTP
services visible to its host. It is **not an authenticated public service** and
must not be exposed directly to the public internet.

### Development

Use debug builds while editing:

```sh
cargo build --locked -p webtool-cli -p webtool-server
```

The installer runs the release build for installation. Keep binaries, data and
smoke artifacts outside Git. Full tests and optional integration checks remain
manual. See docs/STATUS.md for the bounded installed two-client proof and limits.

## Everyday text output

The default view uses readable entries for `saved`, `library list/items`, jobs
and crawl results. Library create/add give concise confirmations. Full IDs, URLs
and available dates remain copyable. Job warnings stay on stderr alongside crawl
progress; text results retain state, counts, limits, errors and saved IDs.

`extract code`, `tables`, `links` and `outline` show the selected material rather
than its JSON wrapper. Block extracts retain their source locations. Link records
have no individual source positions, and the output says so instead of guessing.
Document reading separates headings, prose, fenced code, tables and captions.
Code lines are not wrapped or prefixed. Tabs and whitespace are retained; terminal
control characters are visibly escaped in human views, not in stored data.

Rectangular ASCII tables use an aligned grid when it fits 88 columns. Headers are
labeled only when source flags identify them. Wide, ragged, merged, multiline,
tabbed or non-ASCII tables use labeled rows/cells with explicit header/span values.
Empty cells and zero values remain visible. Supplemental tables keep their label.
No terminal-width detection, color, pager, TUI or interactive behavior is involved.

`--format json` and `jsonl` keep their existing schemas and framing. Markdown
exports are unchanged. Other structured extracts still use their existing output.
Piping into `head` exits quietly on a closed stdout pipe; unrelated errors still
fail normally.

Before, `library items install-shared` included JSON fields (excerpt):

```text
"title": "shared.txt",
"url": "upload:shared.txt",
```

After (excerpt):

```text
shared.txt
  ID: 4ed73eb0f9d58b9aab56b01561c1c1b3c593bcbeab9d8a13313f89ee0926fe20
  Source: upload:shared.txt
```

This is the bounded everyday presentation pass before a clearly labeled alpha,
not a published release or a claim of general extraction accuracy.

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

## arXiv papers and saved citations

Auto reading recognizes arxiv.org `/abs/` and `/pdf/` URLs, modern/legacy IDs,
explicit `vN`, and optional `.pdf`. The official abstract page is the single
metadata source; no Atom lookup or provider chain runs first. Citation meta tags
and article-specific elements supply metadata, not generic article extraction.
Explicit HTTP, CSS, and browser choices are not overridden.

```sh
webtool read https://arxiv.org/abs/cond-mat/0207270v1 --library papers --refresh
webtool cite DOCUMENT_ID --as bibtex
webtool cite DOCUMENT_ID --as csl
```

Verified with that paper: four full-text PDF pages, a body phrase absent from the
abstract, identical original PDF export, and saved-ID BibTeX/CSL. The selected v1
submission date is July 10, 2002, not the later revision date shown on the page.
The PDF remains the document original. A separate `arxiv_metadata` HTML artifact
retains source URL, status, timestamp, and origin. Abstract-only success is not
allowed. Existing Xberg structure/math limitations remain visible.

Identity checks require the article's "for this version" row and the selected,
unlinked submission-history marker to agree. Versioned download links, metadata
IDs, breadcrumbs and canonical identity must not contradict them. An unversioned
canonical link alone is not version evidence. Changed or ambiguous markup fails
rather than selecting latest. Unversioned requests retain the original request and
resolve to an explicitly verified version before PDF retrieval.

Metadata retains title/math notation, ordered literal authors, abstract, categories,
source dates and available identifiers. Display author names remain unsplit; citation
meta-tag names are also retained. The selected history timestamp supplies citation
dates. Journal DOI and arXiv DOI are separate; unavailable fields are omitted.
Saved-ID citations are offline preprint references, not substituted journal articles.
BibTeX uses literal-name braces and syntax escaping; CSL uses literal names. Existing
DOI negotiation remains compatible, including RIS. Saved-paper RIS is unsupported.

Resolver identity is `arxiv-abstract-html/2`. HTTP reads to arXiv hosts share a
process-wide connection gate and three seconds after completion before the next
request. Coordinate external processes separately. Normal Auto paper reads cache
for one day; `--refresh` bypasses that document cache. No automatic retry/fallback.
Earlier API requests returned HTTP 500 from this host; that was not evidence of a
global outage. This path no longer calls the API. See docs/STATUS.md for evidence.
Metadata availability does not grant PDF redistribution rights; follow the paper's
license and [arXiv terms](https://info.arxiv.org/help/api/tou.html).

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
