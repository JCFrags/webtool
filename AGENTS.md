# Agent handoff

## Project intent

Build a Rust-first, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Current milestone: crawl into shared libraries

Work on feat/crawl-library-workflow, issue #13. PR #12 was merged at authorized
8f447a8ed69eacc76e0e262b03e5d1307b557de9 with green build and match-head guard;
issue #11 closed. Stream bounded crawl results as they complete. Attach documents
and persist progress before waiting for slow siblings. Preserve depth, URL query
semantics, same-origin/redirect/robots restrictions, page budgets (including failed
attempts), cancellation, and interrupted-on-restart behavior. No resumable frontier.
Use existing HTTP/extraction/SQLite/workers. Keep CLI progress on stderr, data on
stdout. Report saved IDs, visited attempts, failures and bounded scope honestly.
Build normally with cargo build --locked -p webtool-cli -p webtool-server.
Use one local site and one three-page/depth-one crawl. While its delayed response
is pending, verify a completed child is attached/readable, then inspect final counts,
library search and request log for duplicates/robots exclusions. Temporary files in
runtime/. Rerun only failures. No suites/framework, benchmarks, public crawl,
browser crawl, sitemap work, parser cleanup, scheduler, or dependencies changes.
Preserve all helpers, config/data, PDF/GitHub/captions/Lightpanda, pins and CI. Restart
only this server with runtime/media-config.toml; never a second listener on 8420.
Update docs and mark ready after proof. Do not merge this milestone PR.

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
source-blocks/3 walks only selected containers and preserves their direct text;
split prose runs stay derived. The public JavaScript quote page exposed dropped
selected div/span text; the fix does not restore original container subtrees.
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

Lightpanda 0.3.6 is verified with the official release digest. Its path is in
runtime/media-config.toml, alongside unchanged yt-dlp settings. Telemetry is disabled.
Capture version lightpanda-json-dom/2 uses fetch JSON content/http_status/url and
explicit done plus readyState-complete waits. 0.3.6 can exit zero after a fatal
fetch/wait diagnostic; inspect stderr and the envelope, not exit status alone.
Current upstream CLI docs differ from 0.3.6 help (including wait defaults and
--fail-on-http-error); do not assume newer flags work on this binary.
DOM originals and selectors are snapshot-relative, never original HTTP responses.
Null/unknown navigation remains explicit. Local JS code/table/base-link/export and
public quote extraction passed; public fragments remain derived. Readiness is not
application completeness. Helpers remain process-per-request with bounded resources.
Chromium's historical timeout is unresolved; fastCRW is still unverified. Do not
change browser binaries/config in place and assume cached results describe them.

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
