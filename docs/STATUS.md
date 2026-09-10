# Implementation and validation status

Imported snapshot date: September 9, 2026.
Bootstrap verified: September 10, 2026 UTC (September 9 local).

## Runnable bootstrap results

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
do not delete data or reset main. No merge has been authorized.

### Remaining blockers and next task

No blocker remains for the default runnable milestone. Optional Xberg documents,
OCR models/native runtimes, fastCRW, Lightpanda, Chromium, and yt-dlp are not
validated or configured; doctor reports them unavailable. They are not implied
working by a successful default build. No provider cycling was needed.

Highest-value next task: validate default HTML source fidelity on a representative
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
