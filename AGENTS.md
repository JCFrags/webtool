# Agent handoff

## Project intent

Build a Rust-first, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Current milestone: GitHub source reading

Work on `feat/github-source-reading`, issue #9. PR #8 was merged only at
4500215be8d3449130890783e9f66e8265fb0781 with a green build and match-head guard;
issue #7 closed. Open a linked draft PR early; push coherent checkpoints.
Mark ready only after the bounded directory/file workflow works. Do not merge.

Extend sources.rs using the existing HTTP client and GitHub APIs, not gh, cloning,
a service, or bulk ingestion. Auto mode routes repo roots to pinned README, blob
URLs to actual bytes, and tree URLs to immediate entries. Explicit HTTP/CSS/browser
choices bypass native routing. Resolve refs immutably, including encoded/slash
refs with bounded work and honest ambiguity/missing/rate-limit errors.

Retain source-file bytes, real filenames, commit/path/ref metadata and source
lines. Directory originals are API JSON and readable listings are derived, never
claims that child files were read. Keep incomplete listing warnings visible and
README relative links pinned. Version source-resolution changes in cache keys.

Build with `cargo build --locked -p webtool-cli -p webtool-server`. Restart only
this project's server with runtime/media-config.toml, without deleting data or
starting a second listener. Read one JCFrags/webtool directory, follow one pinned
Rust-file link, inspect code/commit/lines, find a symbol, and compare original
export with the same pinned source. Fresh retrieval; runtime/ holds outputs.
No suites, framework, benchmarks, broad lint, or unrelated regression reruns.
Only failed steps may be rerun. Preserve PDF defaults, captions setup, search
pins, and the single CI build. No issues/PRs/releases, auth, browser, or OCR work.
Update README.md and docs/STATUS.md with results and concrete limits.

## Confirmed local caption setup

Official PyPI helper installed with `uv tool install 'yt-dlp[default]' --index-url
https://pypi.org/simple`: yt-dlp 2026.08.19 and EJS 0.8.0. Existing Node v24.18.0
satisfies documented Node >=22; no new runtime or ffmpeg is needed for this path.
Machine paths live only in ignored runtime/media-config.toml. Start this server
with `./target/debug/webtoold --config runtime/media-config.toml`.

One live 19-second video passed with six provided English cues. Nonfatal yt-dlp
impersonation warnings remain visible; do not install extra dependencies only to
silence them. See docs/STATUS.md for results. Media selection uses load-info-json
to keep source headers, literal language selection, and one track. Keep the
helper's group/time/stdout/stderr bounds and Unix file-size limit. Never retain
signed track metadata in Git or logs. PDF defaults/search pins/CI are unchanged.

## Workspace

| Package | Responsibility |
|---|---|
| `webtool-protocol` | Document blocks, source locators, API messages, text and Markdown rendering |
| `webtool-engine` | Readers, retrieval, search, SQLite, artifacts, shared libraries, persistent crawl jobs |
| `webtool-server` | Axum routes, uploads, process startup, shutdown |
| `webtool-cli` | Thin HTTP client, ordinary commands, exports, stdout and stderr behavior |

The CLI must not depend on the engine package.
Clients must not open the server's database file.
The Python scripts are development checks, not application components.

## Deferred optional integration boundaries

Xberg 1.1.1's existing APIs compiled with the committed lockfile; no dependency
change was needed. Default search and HTML integrations remain unchanged.
OCR/ONNX model readiness and fastCRW APIs still require separate work. Do not
upgrade the search dependency from =0.3.1 (0.3.2 creates a search-tui cycle).

The fastCRW adapter is experimental and does not own the crawler yet.
The current crawler is application-owned, not a completed fastCRW fork.
Do not import upstream CLI, server, cache, authentication, or unrelated agent features to fix an adapter.

## Correctness requirements

- Retain original bytes before normalization.
- Keep code, table values, captions, and exact quotations traceable to saved sources.
- Use `derived` locations when an exact original location is unavailable.
- Preserve real HTTP statuses when supplied, and use null when unavailable.
- Keep search snippets distinct from evidence fetched from destination pages.
- Preserve missing JSON values separately from explicit JSON null.
- Do not execute notebook cells, recalculate spreadsheets, or silently transcribe missing captions.
- Keep partial crawls and parser warnings visible.
- Preserve identical document IDs for identical accepted snapshots.
- Escape terminal control sequences only in presentation, not retained artifacts.

## Known implementation gaps

Default extraction passed the bounded nested-code/merged-table smoke and one live
Rust Book chapter. This is not general extraction-quality validation.
source-blocks/2 walks only selected containers; split prose runs stay derived.
Unique original pre/table matches recover code text/language and table cells.
Do not substitute an entire original list or quote to recover descendants.
Non-UTF-8 decoding is absent.
Markdown parsing implements a limited block reader, not full CommonMark.
HTML table nesting, list hierarchy, inline link placement, and mathematical fidelity need stronger fixtures.

The Xberg adapter preserves upstream page text as paragraphs and keeps table
matrices accessible via extract. Parser revision is source-blocks/2. Default
output is plain, quality rewriting is disabled, and OCR is disabled when absent.
Metadata retains the first upstream document and the full output envelope.
It does not expose downloadable figures or fine-grained document elements.
Table supplements may repeat page text and are explicitly labeled in rendering.
Do not remove the supplemental_table_blocks metadata used for these labels.
One two-page native-text PDF is verified; Office, scans, and real PDF tables are
not. Empty extraction fails; empty pages in partial documents produce warnings.
OCR, layout, and equation recognition require actual model fixtures and accuracy tests.

The browser helper starts a process per request.
It does not report reliable navigation status, redirects, or verified readiness.
The local Chromium smoke test timed out, including a later blank-page probe.
Do not interpret the installed executable as an operational browser integration.

The crawl frontier is not persisted.
Running jobs become interrupted after restart, while queued jobs are rescheduled.
Robots handling is partial and must not be described as fully RFC-compliant.
Automatic sitemap-tree expansion is absent.

GitHub auto routing now supports root README, actual blob bytes, and immediate
nonrecursive tree listings. source_resolver=github-source/2 versions cache keys.
Resolve refs via bounded candidates (8 maximum); require explicit SHA or encoded
slash boundary when ambiguous. Traverse at most 16 path components through pinned
Git trees; do not follow symlinks/submodules or replace native errors with HTML.
Directories retain API JSON, derived locations, and explicit scope/truncation
warnings. README link supplements are limited, not a full CommonMark parser.
Issues, PRs, releases, and complete-repository ingestion remain unimplemented.
There is no dedicated arXiv version-resolution or scholarly-discovery module.

Search supports ordinary web results only.
Image, video, news, date, language, and domain-filter interfaces remain incomplete.
There are no semantic rerankers or automatic LLM calls.

## Deferred source fidelity

Wait for coordinating direction before expanding scope. The public-page smoke
still shows flattened inline superscripts; table captions, full list hierarchy,
and ambiguous mappings need separate source-fidelity work. Do not turn this
milestone into a Markdown rewrite or optional-integration validation campaign.

## Boundaries not to expand

No enterprise accounts, private workspaces, Redis, vector service, Kubernetes, or mandatory LLM runtime.
No default browser automation actions such as clicking, typing, or checkout.
No hidden fallback chain across several extraction engines.
No claim that Rust percentage or binary size proves lower total deployment cost.
