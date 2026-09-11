# Agent handoff

## Project intent

Build a Rust-first, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Current milestone: local alpha candidates

Work on chore/alpha-packaging, issue #21, PR #22. PR #20 merged at its authorized
unchanged head with green CI and --match-head-commit. Main merge: 6a99b00.
Issue #19 closed. Features are frozen at 0.1.0-alpha.1. No dependency upgrades,
API/schema changes, CI changes, installation, server startup, merge, tag or release.

The single locked native release build succeeded at ec909cc6da5c7ba6e90f960d0b11070804e18d00.
Assembly then failed on an absent Cargo cache .cargo-checksum.json. Only assembly
was retried using Cargo.lock checksums and a conservative native dependency
inventory. The script now uses lockfile checksums. No binary rebuild followed.
Existing archives/source retain the build checkpoint, including its old script.
Do not claim that a later packaging correction was compiled into those binaries.

See docs/STATUS.md for artifact hashes and failed server-dependent proof, and
read docs/THIRD-PARTY.md plus packaging/licenses/index.json before distribution.
Keep the PR draft while proof and publication blockers remain. Recover exact
attribution files, not just LICENSE basenames. Native offline cargo metadata
needs --filter-platform matching the Rust host to avoid unrelated target caches.
Do not bundle raw dependency archives: they can contain excluded test-only models.

The server was absent before packaging. Do not use historical PID records as proof
of a live process. Both installed binaries, receipts, helper hashes, client settings,
config and database were preserved. Do not start a server to hide the proof gap.
No suites, new retrieval/ingestion, benchmarks or platform matrix are authorized.
This project is unrelated to Pi/Glance/Chrono and Terminal Agent Browser release
coordination. Direct those requests to their owning worker.

## Confirmed local caption setup

Official PyPI helper installed with `uv tool install 'yt-dlp[default]' --index-url
https://pypi.org/simple`: yt-dlp 2026.08.19 and EJS 0.8.0. Existing Node v24.18.0
satisfies documented Node >=22; no new runtime or ffmpeg is needed for this path.
Machine helper settings live in ignored runtime/media-config.toml. Generic
startup: `webtoold --config /absolute/path/server.toml --data-dir
/absolute/path/existing-data`. Use the intended absolute paths outside the checkout.

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

Crawl results are consumed with buffer_unordered and next(), never collected as
an entire batch before publishing. Attach each usable document and persist its
job update before waiting again. Keep same-depth batches, bounded candidate
admission (including excluded URLs), query semantics, and fresh HTTP reads so
ordinary-read caches cannot bypass redirect scope/robots checks. Failed attempts
consume budget. visited counts completed attempts; failed is separate from
extraction warnings. Zero usable results after attempted reads is Failed, not
successful Partial. Old job payloads without failed deserialize with zero; no
historical recount is implied. Cancellation retains saved library documents.
One three-page/depth-one local crawl verified attachment/read/search and doctor
while a 15-second sibling was pending, final IDs/counts and no duplicate/excluded
fetches. No failure/cancellation/restart campaign was run.

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
The arxiv module uses official abstract-page HTML only, resolver
arxiv-abstract-html/2. The same cond-mat/0207270v1 paper passed four-page PDF reading,
body find, original-byte comparison and offline saved-ID BibTeX/CSL. A local probe
returned HTTP 200 and its retained HTML matched the product's metadata artifact.
Earlier API HTTP 500 results were local observations, not a global-outage diagnosis.
No API requests were retried during this continuation.
Require the "for this version" row and sole unlinked history marker to agree;
check other identity/version links, not mere occurrence in history. Canonical
links may be unversioned. Use the selected history timestamp for citations, not
the latest revision/date or a typeset date inside the PDF. Literal display authors
and citation meta authors are retained without surname inference. Journal DOI and
arXiv DOI remain distinct. Preserve mathematical notation in retained metadata.
Keep text/html arxiv_metadata provenance and PDF original separate. Citation
metadata-origin claims come from the stored record, including legacy records.
Shared HTTP gate/pacing and one-day cache remain. Layout changes, contradictory
identity or missing selection evidence fail explicitly. Other identifier/version
forms were source-inspected, not an extra paper corpus. No scholarly search.

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
