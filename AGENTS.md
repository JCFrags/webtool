# Agent handoff

## Project intent

Build a Rust-first, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Current milestone: usable YouTube captions

Work on `feat/media-captions`, issue #7. PR #6 was merged only at authorized
25059c5d42fef93bdb980817b89c5fe210b187fe with a green build; issue #5 closed.
Open a linked draft PR early and push coherent checkpoints. Keep the new PR
unmerged; mark ready only when the bounded caption workflow works.

Use official yt-dlp with its documented JavaScript runtime/EJS support. Reuse
installed Node when supported; no extra runtime is needed on this workstation.
Keep helper binaries and machine-specific config outside Git. No browser cookies,
login sessions, proxies, audio/video downloads, transcription, or playlists.
Let yt-dlp fetch the selected subtitle using its source context and headers;
do not send a signed caption URL to the generic HTTP client. Bound subprocesses,
file sizes, and time; clean temporary files on success or failure.

Default YouTube watch/youtu.be reads now yield captions, not surrounding HTML.
Renderer Auto is the default; explicit Http or selector stays HTML. Captions
forces the media reader. ReadRequest.language defaults to en; cache keys include
language and media parser revision 2. Keep existing request constructors current.
Honor requested language, prefer provided tracks, label automatic captions, and
never silently translate. Keep explicit reader choices, stable video metadata,
verbatim original caption bytes, cue text/timestamps, and meaningful errors.
Cache by language as well as source. Bump media parser revision when changing it.

Build with `cargo build --locked -p webtool-cli -p webtool-server`, restart only
this server without deleting data, then try one short public video. Inspect
reading, a known phrase, timestamps, and original export. No suites, broad lint,
benchmarks, provider sweeps, or new test framework. Rerun only failed steps. If
YouTube blocks this host, retain the actual blocker and keep the PR draft.

Preserve PDF defaults, search pins, and the one cached CI build. No browser/OCR
integration or unrelated refactor. Update README.md and docs/STATUS.md with
actual helper versions, setup, commands, outcomes, and limitations.

Keep target/, data/, runtime/, caches, weights, binaries, and secrets out of Git.
Preserve license notices; MANIFEST.sha256 describes the original archive only.

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

GitHub repository roots retrieve only a pinned README.
Blob, tree, release, issue, pull-request, and complete-repository readers are not implemented.
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
