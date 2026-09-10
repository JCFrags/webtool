# Implementation and validation status

Imported snapshot date: September 9, 2026.
Bootstrap verified: September 10, 2026 UTC (September 9 local).

## Milestone 5: GitHub source reading

Verified September 10, 2026. Branch `feat/github-source-reading`, issue #9, PR #10
(not merged). PR #8 was squash-merged at authorized
4500215be8d3449130890783e9f66e8265fb0781 after confirming its unchanged head and
green build, using --match-head-commit. Main fast-forwarded to `3d7df72`;
issue #7 closed. No local changes or runtime data were discarded.

- Normal locked debug build and source-checkpoint CI passed, existing warnings
  only. Dependencies, PDF defaults, captions config, search pins, and CI unchanged.
- Fresh directory read: JCFrags/webtool/tree/main/crates/engine/src resolved to
  `3d7df722944aafe26b6f3b1311649fe46b8b2060`. Nine immediate entries with pinned
  blob/tree links; all listing blocks derived. Original API JSON: 2,107 bytes,
  truncated=false. Visible repository_directory_only warning; no child ingestion.
- Followed its sources.rs link with refresh and saved in library github. Actual
  Rust source, not HTML: 2,312 bytes, 33 lines, Rust language label, exact whitespace.
  Metadata retained requested SHA, repository, path, and resolved commit;
  source.version is that same commit. File read had no warnings.
- Find github_readme returned the source block, lines 1–33. Plain output inspected.
  Export cmp matched `git show COMMIT:crates/engine/src/sources.rs` from the already
  existing checkout. No cloning or application gh subprocess was introduced.
- Startup guard found a stale PID: the old process was gone and port 8420 had no
  listener. After checking this, one server was started with the preserved
  runtime/media-config.toml. No unrelated process was stopped or data deleted.
  No source request failed or needed a retry.

Behavior version: github-source/2 participates in read cache keys. Native routing
is Auto-only without CSS; explicit HTTP/browser remains explicit. Root README
scope warning remains. Files use native readers with real filenames; directory
originals are API JSON and readable listings are derived. Missing references,
paths, public access, rate limits, incomplete trees and unsupported objects have
separate errors. Ref resolution checks at most eight candidate splits; full SHAs
or percent-encoded reference slashes give explicit boundaries. Path traversal is
limited to 16 components. All content requests follow immutable commit/tree/blob
identities, not mutable download URLs. No repository-wide ingestion.

No concrete blocker. Only the requested directory/file workflow was exercised.
Slash-ref ambiguity, encoded paths, missing/rate-limited objects, root README link
supplements, and truncation handling were inspected in source, not separate live
checks. Symlinks/submodules are unsupported. GitHub API response bounds include
base64 overhead; no authenticated quota is configured. README link supplements
handle common inline/reference definitions, not complex CommonMark or unmarked
directory destinations. Highest-value next gap: improve README link interpretation
and directory-target links without changing retained source text.

### Working commands

Server remains at http://127.0.0.1:8420. Startup (do not start another copy):

```sh
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
./target/debug/webtoold --config runtime/media-config.toml
```

Reproduction uses existing library github and replaces only temporary exports:

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
webtool --format json read https://github.com/JCFrags/webtool/tree/main/crates/engine/src --refresh >runtime/github-directory.json
DIR_ID=$(python3 -c 'import json;print(json.load(open("runtime/github-directory.json"))["id"])')
webtool read "$DIR_ID"
FILE_URL=$(python3 -c 'import json;print(next(l["url"] for l in json.load(open("runtime/github-directory.json"))["links"] if l["text"]=="blob: sources.rs"))')
webtool --format json read "$FILE_URL" --library github --refresh >runtime/github-file.json
FILE_ID=$(python3 -c 'import json;print(json.load(open("runtime/github-file.json"))["id"])')
webtool read "$FILE_ID"
webtool find "$FILE_ID" github_readme
webtool export "$FILE_ID" --kind original --output runtime/github-export.rs --force
COMMIT=$(python3 -c 'import json;print(json.load(open("runtime/github-file.json"))["metadata"]["github"]["resolved_commit"])')
git show "$COMMIT:crates/engine/src/sources.rs" >runtime/github-pinned.rs
cmp runtime/github-pinned.rs runtime/github-export.rs
```

These commands reflect the current main commit at proof time; after main changes,
use its reported commit and a symbol in that revision (or use the recorded SHA).
Directory ID: `f05e3e00e5127e6ec7f857a031d67ca955d3a6a834fcf4c092b485f3e37ebc86`.
File ID: `cce49eb3f720cc8d684ff37ecfb0a9c4fb00a2d784f579a3cf76833ca2bbe26a`.
No suites, new test framework, benchmarks, broad lint or unrelated regression
reruns were used. The historical milestones below describe their prior state.

## Milestone 4: YouTube captions

Verified September 10, 2026 UTC. Branch `feat/media-captions`, issue #7, PR #8.
PR #6 was squash-merged at authorized 25059c5d42fef93bdb980817b89c5fe210b187fe
with a green build and --match-head-commit. Main fast-forwarded to `02edc6b`;
issue #5 closed. Runtime data was preserved; PR #8 is not merged.

- Normal locked debug build passed; source checkpoint CI passed. PDF defaults,
  search versions, Cargo.lock, and CI are unchanged. Added only the existing nix
  dependency's resource feature for per-helper file-size limits.
- Official PyPI yt-dlp 2026.08.19 + yt-dlp-ejs 0.8.0 installed locally with uv.
  Existing `/usr/bin/node` v24.18.0 satisfies the official EJS Node >=22 requirement.
  No runtime, ffmpeg, impersonation package, browser, cookies, proxies, models,
  transcription, or audio/video download was used.
- One source: https://youtu.be/jNQXAC9IVRw (“Me at the zoo”, 19 seconds).
  Default auto read selected six provided English cues, saved in library captions.
  Stable video metadata, language/origin, and null (not invented) HTTP status retained.
- Timestamped plain output and original VTT inspected. Find “elephants” returned
  b1, 1200–3360 ms. First cue exactly matches original:
  `All right, so here we are, in front of the\nelephants`.
- Exported 440 bytes equal the verbatim downloaded track retained in data/objects/;
  SHA-256 `5c7fcf32df4558291ee896d133264e6efea2b39ffbadd36f764ce9e8c392ec71`.
- Nonfatal helper warning: no impersonation target available. Caption retrieval
  still succeeded; the warning is preserved, not suppressed. No source blocker.
- Automatic-origin selection, absent-language/blocked-source errors, timeout
  cleanup, and language cache separation are implemented but not separate live
  tests. No provider sweep, test suite, broad lint, benchmark, or smoke retry ran.

Media/native-caption parser revisions are 2. `read --renderer auto` routes only
supported YouTube video URLs to captions; explicit http/CSS remains HTML. Exact
language matching prefers provided over automatic tracks and rejects translated
`tlang` URLs. yt-dlp's own downloader receives selected track/source headers via
temporary load-info-json, not a generic HTTP subtitle fetch. TempDir cleanup,
process groups, one overall deadline, bounded output, and Unix RLIMIT_FSIZE apply.
Signed URLs are redacted from diagnostics and excluded from saved metadata.
Missing helper, blocked source, unavailable captions, and malformed cues are
separate API errors. Metadata alone cannot succeed as a transcript.

### Setup and working commands

Official references checked: [installation](https://github.com/yt-dlp/yt-dlp#installation)
and [EJS](https://github.com/yt-dlp/yt-dlp/wiki/EJS). GitHub rendered reads failed;
the official raw README/wiki sources were read instead. Helper source inspected
from the installed official package, including subtitle header propagation.

```sh
uv tool install 'yt-dlp[default]' --index-url https://pypi.org/simple
yt-dlp --version
node --version
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
# Already running; do not start a second server:
./target/debug/webtoold --config runtime/media-config.toml
```

Ignored runtime/media-config.toml copies config.example.toml and sets
`ytdlp_path = "/home/mainpc/.local/bin/yt-dlp"` and
`ytdlp_js_runtime = "node:/usr/bin/node"`. PID/log remain runtime/webtoold.pid
and runtime/webtoold.log; address http://127.0.0.1:8420. Only this project's old
server was stopped, after checking its executable path.

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
webtool doctor
# Library captions already exists from this proof.
webtool --format json read 'https://youtu.be/jNQXAC9IVRw' --language en --library captions --refresh >runtime/youtube-read.json
DOC_ID=$(python3 -c 'import json;print(json.load(open("runtime/youtube-read.json"))["id"])')
webtool read "$DOC_ID"
webtool find "$DOC_ID" 'elephants'
webtool export "$DOC_ID" --kind original --output runtime/youtube-original.vtt --force
HASH=$(python3 -c 'import json;print(json.load(open("runtime/youtube-read.json"))["source"]["original"]["sha256"])')
cmp "data/objects/$HASH" runtime/youtube-original.vtt
webtool library items captions
```

Saved ID: `1a28a8f63209abdb9912532346d649a1e87c509c68a3a905454e54cce99fc9ef`.
The historical milestones below retain their original outcomes and former setup;
current server configuration is recorded above.

## Milestone 3: usable PDF reading

Verified September 10, 2026 UTC. Checkout `/home/mainpc/Projects/webtool`, branch
`feat/pdf-reading`; issue #5, PR #6 (not merged). PR #4 was squash-merged with
`--match-head-commit 7558b25529e9130697a72d144624784d9579f715` after checking its
head and successful build. Local main fast-forwarded to `de4ad69`, and issue #3
closed. Existing runtime data was preserved.

### Build and delivered behavior

Both commands passed with existing unused-import warnings only:

```sh
cargo build --locked -p webtool-cli -p webtool-server --features webtool-server/documents
cargo build --locked -p webtool-cli -p webtool-server
```

The pinned Xberg 1.1.1 adapter compiled without dependency fixes. No dependency
versions or Cargo.lock changed; the lockfile already selected xberg-native-pdf
1.1.4. Documents are now a default server feature. The CLI remains independent
of the engine. The single CI command is unchanged and now includes documents.

Document parser revision: `xberg/1.1.1+source-blocks/2`.
Page text uses existing Paragraph blocks, with no trimming or Markdown rewrite.
Defaults request plain output and disable upstream quality rewriting. Missing
page numbers remain derived; reported page numbers are retained without boxes.
Supplemental table blocks remain extractable and are explicitly labeled in plain
and Markdown output, rather than silently repeating page text. Cell header roles
and merged spans are not inferred from Xberg's simple matrix. Metadata retains
full upstream documents and output-envelope errors/summary/additional results.

Empty/whitespace-only page objects are skipped with visible diagnostics; they do
not count as successful extraction. No text and no nonempty table cells is an
error. OCR absence is explicit, without inferring that every empty page is a
scan. Partial page-count and upstream warnings are retained. A failed extraction
still leaves retained original bytes under the existing artifact behavior, but
has no successful saved-document handle or saved normalized upstream metadata.
These empty/scan paths were inspected in source, not exercised with more PDFs.

### Actual bounded proof

One public PDF only:
https://sample-files.com/downloads/documents/pdf/fillable-form.pdf

| Check | Result |
|---|---|
| Fresh URL read | HTTP 200, application/pdf, 54,059 bytes |
| Pages | Two Paragraph blocks, reported pages 1 and 2, bbox null |
| Ordinary output | Readable form labels; no whole-page code fences |
| Native text accuracy check | Existing pdftotext independently confirmed page titles/positions; its reference output was inspected |
| Find | “Personal Information” found in b2, page 2 |
| Local ingest of URL-retained original | Same two blocks and exact upstream page strings |
| Original local export | cmp byte-identical to retained input |
| Artifact hash | Both documents match SHA-256 8413d3f961113a06b9ebedd0a3e9963517379cd62d3600f5067d497e3dcfa269 |
| Upstream metadata | Title, author, dates, page dimensions/count, native method, ocr_used=false retained |
| Tables extract | Empty array, matching upstream (this PDF supplied no structured tables) |
| Existing HTML | Previous Rust Book saved document read once successfully |
| doctor | documents enabled; OCR unavailable/not compiled |

Both PDF outputs retained `document_structure_partial` warnings. No hidden
upstream errors appeared. The old HTML kept its existing derived-location warning.
`pdftotext` was already installed and used only for this reference check; it is
not an application dependency or replacement extraction engine.
No cargo test, broad lint, benchmarks, OCR, model downloads, extra feature matrix,
or new test framework was used. No smoke steps needed a retry.

No concrete blocker remains. This is bounded native-text PDF support, not an
accuracy guarantee for arbitrary PDFs. Real table detection, scanned/empty PDF
runtime behavior, complex layouts, form values, encrypted inputs, Office, and
other formats remain unverified. No Office support claim follows from compilation.

### Local server and exact reproduction

The updated server is left running at http://127.0.0.1:8420 with the existing data/.
Only this project's old PID was terminated after verifying its executable path.
PID/log: runtime/webtoold.pid and runtime/webtoold.log. No data was deleted.

Startup (do not start a second copy while it is running):

```sh
cd /home/mainpc/Projects/webtool
./target/debug/webtoold --config config.example.toml
```

The commands below reuse ignored runtime/ and explicitly replace named temporary
exports. The URL read uses refresh; local ingest always parses.

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
mkdir -p runtime
webtool doctor
webtool --format json read https://sample-files.com/downloads/documents/pdf/fillable-form.pdf --refresh >runtime/pdf-url.json
URL_ID=$(python3 -c 'import json;print(json.load(open("runtime/pdf-url.json"))["id"])')
webtool read "$URL_ID"
webtool find "$URL_ID" 'Personal Information'
webtool export "$URL_ID" --kind original --output runtime/fillable-form.pdf --force
webtool --format json ingest runtime/fillable-form.pdf >runtime/pdf-local.json
LOCAL_ID=$(python3 -c 'import json;print(json.load(open("runtime/pdf-local.json"))["id"])')
webtool read "$LOCAL_ID"
python3 -m json.tool runtime/pdf-local.json
webtool export "$LOCAL_ID" --kind original --output runtime/fillable-form-export.pdf --force
cmp runtime/fillable-form.pdf runtime/fillable-form-export.pdf
webtool --format json extract "$LOCAL_ID" tables
webtool read 04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded
```

Saved URL ID: `bdca71b0310fdf092265cb50d0f2c2cedb6cfe596ad943547a80864c38cc4996`.
Saved local ID: `64ef0a3524cc691e76a2d08d6c8028e44eaac1aa08ca128e753235f48d88fe7b`.

The sections below are historical milestones; their former feature/merge state
is superseded by this section.

## Milestone 2: selected HTML source fidelity

Verified September 10, 2026 UTC. Checkout `/home/mainpc/Projects/webtool`, branch
`fix/html-source-fidelity`; issue #3 and PR #4. Bootstrap PR #2 was squash-merged
with `--match-head-commit a2db698e9761e9c1b0e1bb8869d858bf8d9a6cca` after checking
its unchanged head and successful build. Main advanced to
`24225fe` and issue #1 closed. No runtime data was removed. PR #4 is not merged.

### Changed behavior

- HTML parser revision is `rs-trafilatura/0.2.2+source-blocks/2` (explicit CSS
  uses `explicit-css+source-blocks/2`). Old stored document schemas are unchanged.
- Walk selected list/quote containers in order, emitting prose runs around
  nested blocks instead of flattening them or repeating descendant text.
- Only unique original-element matches restore preformatted text and language
  classes (`language-` or `lang-` on pre/code), or table cell boundaries, header
  flags, rowspan, and colspan. Table row/cell traversal excludes nested rows.
- Never replace an entire selected container with its original subtree. This
  avoids restoring descendants removed by content selection. Ambiguous matches
  and partial container prose retain derived locators and visible warnings.
- Existing plain rendering already preserves code whitespace; it needed no
  change. Plain tables remain tab-separated values, not visual merged-cell
  layouts. JSON retains spans/header flags; Markdown's existing HTML table
  representation supports spans. No document schema or storage changes.
- Search pin, extractor dependency, Cargo.lock, and CI are unchanged.

### Actual checks

| Check | Outcome |
|---|---|
| Normal locked debug CLI/server build | Passed; existing warnings only |
| Authored default-extraction HTML ingest, no selector | Passed: 10 blocks in order |
| Nested code inside li and blockquote | Exact tabs/spaces/newlines; Rust and Python language labels restored |
| Merged table | Rows have 2, 2, 3 cells; Mode rowspan=2, Limits colspan=2; header flags retained |
| Selection and duplication | No navigation in blocks/plain output; each surrounding phrase and code sample appears once |
| Locators | Both code blocks and table have verified HTML locators; four split prose runs honestly derived |
| Plain CLI output | Inspected code indentation and table values; no alternate-screen UI |
| Original fixture export | All 1085 bytes identical by cmp |
| Fresh public Rust Book read | 16 code blocks, two tables, all with verified HTML locators |
| Retained public original comparison | All 16 pre texts match exactly in order; both complete cell matrices/flags/spans match; SHA-256 matches artifact |
| Existing bootstrap saved document | Still readable after server restart |

The public URL was https://doc.rust-lang.org/book/ch03-02-data-types.html, read
with `--refresh` and no selector. The original was 44,687 bytes. Tables were 7×3
and 6×2. Ordinary output was inspected and contains every exact code string.
Ten other blocks have derived source locators; this warning remains visible.
No tests, broad lint, benchmarks, optional builds, or additional public pages ran.
A temporary Python HTMLParser comparison inspected the retained pre/table data;
it is not a new test framework or application dependency.

No blocker remains for this milestone. This is not full HTML fidelity: inline
superscripts in the public chapter still flatten into ordinary text; table
captions, nested table representation, and complete list hierarchy remain outside
this bounded fix. A selected code/table without a unique original match keeps the
cleaned content and a derived locator; no original match is invented.

### Exact local reproduction

The updated server remains running at http://127.0.0.1:8420 using the existing
`data/`. Logs and PID remain in `runtime/webtoold.log` and `runtime/webtoold.pid`.
Startup after stopping the existing server, if needed:

```sh
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
./target/debug/webtoold --config config.example.toml
```

In another terminal, create the single temporary example (not a fixture corpus):

```sh
cd /home/mainpc/Projects/webtool
mkdir -p runtime
cat >runtime/html-fidelity.html <<'HTML'
<!doctype html>
<html lang="en"><head><title>Technical extraction smoke</title></head><body>
<nav><p>EXCLUDED NAVIGATION</p><pre><code>navigation_only();</code></pre></nav>
<main><article><h1>Technical extraction smoke</h1>
<p>This guide documents exact code and table structure for a small shared research reader. The examples preserve indentation, newlines, and source values without executing code.</p>
<ul><li>Before list code.<pre><code class="language-rust">  fn main() {
	println!("Exact nested code");
  }
</code></pre>After list code.</li></ul>
<blockquote>Before quote code.<pre class="language-python"><code>  if ready:
    print("Exact quote code")
</code></pre>After quote code.</blockquote>
<table><thead><tr><th rowspan="2">Mode</th><th colspan="2">Limits</th></tr><tr><th>Min</th><th>Max</th></tr></thead><tbody><tr><th>safe</th><td>0</td><td>17</td></tr></tbody></table>
<p>The table describes safe mode with a minimum of zero and maximum of seventeen. Header cells and merged cells are part of the source, not generated summaries.</p>
</article></main></body></html>
HTML
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
# Ingest always parses; it is not a cached URL read.
webtool --format json ingest runtime/html-fidelity.html >runtime/html-fidelity.json
DOC_ID=$(python3 -c 'import json;print(json.load(open("runtime/html-fidelity.json"))["id"])')
python3 -m json.tool runtime/html-fidelity.json
webtool read "$DOC_ID"
webtool export "$DOC_ID" --kind original --output runtime/html-fidelity-original.html --force
cmp runtime/html-fidelity.html runtime/html-fidelity-original.html
webtool --format json read https://doc.rust-lang.org/book/ch03-02-data-types.html --refresh >runtime/rust-data-types.json
PUBLIC_ID=$(python3 -c 'import json;print(json.load(open("runtime/rust-data-types.json"))["id"])')
webtool read "$PUBLIC_ID"
webtool export "$PUBLIC_ID" --kind original --output runtime/rust-data-types-original.html --force
# Inspect retained source pre/table elements alongside parsed JSON:
webtool --format json extract "$PUBLIC_ID" css --expression 'pre,table'
webtool --format json extract "$PUBLIC_ID" code
webtool --format json extract "$PUBLIC_ID" tables
```

`--force` above only replaces the named temporary exports. All example files and
local result JSON stay under ignored runtime/. Fixture document from this run:
`de34d85d7ef36f8965d19375046332382a6fb5651489490d1c5ec69c0f1b53aa`.
Public document:
`04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded`.
New extraction revision participates in the saved document ID; existing IDs
continue to read the stored result. URL reads need `--refresh` to avoid old cache.

## Historical runnable bootstrap results

The following records milestone 1, before its authorized merge. Current branch,
server revision, and workflow are recorded above.


- Rust 1.98.0 / Cargo 1.98.0 on Fedora 44; debug build.
- `cargo build --locked -p webtool-cli -p webtool-server`: passed with
  existing unused-import and optional-document dead-code warnings.
- A real Cargo.lock was generated and committed. Default HTML and search stay enabled.
- `webtoold` started; `webtool doctor` reached API v1 at http://127.0.0.1:8420.
- Created shared library `bootstrap` and ingested tests/fixtures/source.md.
- Read saved document and found `Exact code` in block b3, source lines 5–7.
- Original export: 87 bytes; `cmp` confirmed byte-for-byte equality.
- Default `read https://example.com`: passed, returned “Example Domain”,
  the domain-purpose paragraph, and “Learn more”, without a selector or browser.
- Live `search 'Rust programming language' --limit 5`: exit 0, five links,
  no warnings. Brave and DuckDuckGo both contributed. Inspected returned URLs:
  https://en.wikipedia.org/wiki/Rust_(programming_language),
  https://rust-lang.org/learn/,
  https://www.geeksforgeeks.org/rust/introduction-to-rust-programming-language/,
  https://rust-lang.org/, https://rust-lang.org/en-US/.
  Snippets are provider output; destination pages were not read by this search.
- Full tests, optional feature builds, lint, benchmarks, coverage, Docker,
  DOI, repository readers, and captions were not run in this milestone.

### Build corrections

metadata-search-engine-rs 0.3.2 has a dependency cycle through search-tui.
Pinning 0.3.1 resolves it without importing a TUI or disabling search.
Caption timestamp replacement required a string replacement argument.
Search futures are collected before bounded stream execution to satisfy
Axum's handler-future lifetime requirements; concurrency remains bounded at four.

### Local operation

Checkout: `/home/mainpc/Projects/webtool`, branch `feat/bootstrap-running-cli`.
The server is left running on loopback only. Data lives in ignored `data/`;
logs, PID, and smoke outputs live in ignored `runtime/`.
One server process per data directory. Do not start a second while it is running.

```sh
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
mkdir -p runtime
nohup ./target/debug/webtoold --config config.example.toml >runtime/webtoold.log 2>&1 </dev/null &
echo $! >runtime/webtoold.pid
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
webtool doctor
webtool library create bootstrap --description 'Runnable CLI smoke'
webtool --format json ingest tests/fixtures/source.md --library bootstrap --actor JCFrags
DOC_ID=cd6afccebfcf8a897537c855c8e33c70686eb91372878e7ed33133475905f4da
webtool read "$DOC_ID"
webtool find "$DOC_ID" 'Exact code'
webtool export "$DOC_ID" --kind original --output runtime/source-original.md
cmp tests/fixtures/source.md runtime/source-original.md
webtool read https://example.com
webtool search 'Rust programming language' --limit 5
```

Library and export already exist after this run. Use a new library/output name
for a later run, or explicit `--force` only when replacing the export is intended.
To stop, verify the PID in runtime/webtoold.pid belongs to this webtoold and send
SIGTERM. Rollback means stopping this process and using a prior feature commit;
do not delete data or reset main. At bootstrap handoff no merge had been authorized; PR #2 has since been merged as recorded above.

### Remaining blockers and next task

No blocker remains for the default runnable milestone. Optional Xberg documents,
OCR models/native runtimes, fastCRW, Lightpanda, Chromium, and yt-dlp are not
validated or configured; doctor reports them unavailable. They are not implied
working by a successful default build. No provider cycling was needed.

Bootstrap next task (now completed by milestone 2 above): validate default HTML source fidelity on a representative
public documentation page containing code and tables, against retained originals.

## Historical archive checks

The supplied Python SQLite/package logs report 33 passing checks. The supplied
Chromium probes timed out. These historical checks are not Rust test results.
The existing 70 Rust tests were kept but not run or expanded during bootstrap.
`MANIFEST.sha256` describes archive bytes only; it is not a current-tree checksum.
Downloads README(20260910-045733).md and STATUS.md matched the archive exactly.
The older shared-terminal proposal specifies Python; the current direction and
Rust implementation supersede that stack choice.

## Implementation state

### Core source is present

The repository contains the application, not just interfaces or pseudocode.
It includes a CLI, HTTP server, storage, native readers, search orchestration, job execution, and tests.
The default build and small smoke sequence above now run; this does not validate every advertised capability.

### Optional integrations need validation

The Xberg implementation calls the documented Rust API and retains the serialized upstream result.
Page, slide, and sheet references are assigned only from supplied fields.
Tables are exposed as supplemental blocks without invented header roles or spans.

The yt-dlp implementation invokes the configured helper and retrieves an available VTT track.
It does not download video or generate a replacement transcript.

The browser implementation uses bounded subprocesses with output limits and timeouts.
DOM captures retain warnings about unknown status, redirects, and incomplete readiness.
An experimental fastCRW feature provides an additional explicit renderer.

These are source implementations, not working integrations demonstrated in this environment.

## Differences from the architecture proposal

| Proposal | Delivered source |
|---|---|
| Integrate selected fastCRW crawler components | Application-owned bounded crawler, plus an experimental fastCRW renderer adapter |
| Select browser engines automatically by capability | Explicit HTTP, Lightpanda, Chromium, or experimental fastCRW selection |
| Reuse warm browsers | Fresh helper process per browser read |
| Full structured document and figure handling | Page text and supplemental tables, with full upstream metadata retained |
| Broad scholarly and repository reading | DOI export, generic URL reading, and revision-pinned GitHub README retrieval |
| Broad format-specific search | General web metasearch and local keyword search |
| Polished terminal layouts for every result | Plain reading and numbered search, with indented JSON for several structural commands |
| Fully resumable operations | Persisted crawl status and results, not a persisted frontier |

These differences are unfinished work, not evidence that the proposal was fully implemented.

## Deferred validation

Beyond bootstrap, check representative source fidelity before adding integrations.
Full tests and optional integration checks remain manual. Documents, browser
readiness, crawl recovery, captions, and resource measurements need separate work.
Do not expand this milestone into those checks.

## Operational limits

The server assumes one instance per data directory and a trusted network.
All data is shared, with no authentication or per-user quotas.
Request sizes and worker concurrency are bounded, but sustained-load capacity has not been tested.
A disconnected foreground client has no universal reconnectable job record.
Only crawls currently use persisted job records.

The cache is time-based and does not perform conditional HTTP revalidation.
Failed parses can leave an unreferenced original artifact on disk.
Artifact garbage collection and full-library backup export are not implemented.

Use new-format fixtures before extending advertised support.
Keep this document accurate after each actual build and test run.
