# Agent handoff

## Project intent

Build a Rust-first, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Delivery truth

The source has not been compiled.
No Rust compiler was available, and toolchain downloads failed.
There is no verified lockfile, executable, container build, or live-provider test result.

The actual SQLite schema and package checks passed 33 tests through Python SQLite.
A separate Chromium helper check timed out.
Those results do not validate the Rust implementation.
Read `docs/STATUS.md` and `docs/local-validation.json` before changing status claims.

## Start here

1. Install a current Rust toolchain and resolve dependencies.
2. Generate a real `Cargo.lock` and commit it after review.
3. Run `cargo fmt --all`.
4. Run `cargo test --workspace` and fix compiler errors before adding features.
5. Compile the `documents` and `crw-browser` integrations separately.
6. Run `scripts/smoke_cli.py` against locally built binaries.
7. Add live tests and report exact upstream versions and outcomes.

Do not claim that dependency API inspection established build compatibility.
Keep failures visible in the handoff and release notes.

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

## First compiler checks

The optional integrations target published APIs inspected during implementation.
Their transitive features were not resolved in this environment.
Check these boundaries first:

- `rs_trafilatura::Options` and `ExtractResult` against version `0.2.2`.
- `metadata-search-engine-rs` engine constructors and `SearchResult` against `0.3.2`.
- Xberg's `extract`, `ExtractInput`, `PageConfig`, and serialization against `1.1.1`.
- fastCRW's renderer constructor, deadline, fetch method, and result fields against `0.34.0`.
- Axum handler futures for `Send` requirements, including optional document extraction.
- Native linkage, duplicate SQLite libraries, and the selected ONNX runtime's platform requirements.

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

The default HTTP and HTML flow needs real-world extraction testing.
Non-UTF-8 decoding is absent.
Markdown parsing implements a limited block reader, not full CommonMark.
HTML table nesting, list hierarchy, inline link placement, and mathematical fidelity need stronger fixtures.

The Xberg adapter normalizes page text and simple table matrices.
It retains the full upstream result in metadata.
It does not expose downloadable figures or fine-grained document elements.
Table supplements may repeat values already present in page text.
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

## Improve next

After compilation and baseline tests, prioritize source fidelity over new integrations.
Add representative website, repository, caption, and PDF fixtures.
Measure both cold and warm latency, total process memory, and correct-content retention.
Record missing content and parser failures alongside timings.

Then choose one browser transport and reuse it across concurrent jobs.
Reuse search-provider HTTP clients rather than creating them per query.
Add conditional HTTP validation and bounded streaming responses.
Avoid making those changes before establishing correctness tests.

Academic collections, citation formatting, media analysis, and explicit LLM jobs can follow.
Keep derived model output separate from source text and retain source references.

## Boundaries not to expand

No enterprise accounts, private workspaces, Redis, vector service, Kubernetes, or mandatory LLM runtime.
No default browser automation actions such as clicking, typing, or checkout.
No hidden fallback chain across several extraction engines.
No claim that Rust percentage or binary size proves lower total deployment cost.
