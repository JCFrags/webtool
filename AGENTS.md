# Agent handoff

## Project intent

Build a Rust-first, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Current milestone: usable PDF reading

Work on `feat/pdf-reading`, issue #5. PR #4 was merged only at the authorized
7558b25529e9130697a72d144624784d9579f715 with its build green; issue #3 closed.
Open a linked draft PR early, push coherent checkpoints, mark ready after the
bounded PDF workflow works. Do not merge this new PR without authorization.
Preserve local changes and runtime data. Do not force-push.

Use the existing pinned Xberg adapter, no replacement PDF parser or models.
Start integration with:

```sh
cargo build --locked -p webtool-cli -p webtool-server --features webtool-server/documents
```

Documents are now enabled in the server default. The pinned-feature build and
normal build both passed without dependency/lockfile changes. Continue with:

```sh
cargo build --locked -p webtool-cli -p webtool-server
```

Keep the CLI independent of the engine and the one cached Ubuntu CI build.
No search-provider upgrades, browser, OCR, model downloads, Office verification
claims, unrelated refactoring, or Markdown rewrite. Only targeted dependency
changes required for the pinned adapter; commit the real lockfile if changed.

Preserve original bytes, reported pages, readable unmodified upstream page text,
metadata, warnings, and upstream output. Empty page objects are not success.
Do not label every empty page a scan; report unavailable OCR when appropriate.
Keep tables available through extract and label supplemental tables in reading.
Bump the parser revision for normalization changes. Preserve old saved documents.

Verify one small public text PDF through URL read and local ingestion of the same
bytes under runtime/. Inspect ordinary output/JSON page locations, find a known
phrase, and compare original export with input. Read one existing saved HTML
once. Restart only this project's server without deleting data. No cargo test,
broad lint, benchmarks, format matrix, new test framework, or repeated smoke
campaigns. Rerun only failed steps; compilation is not extraction accuracy.
Update README.md and docs/STATUS.md with actual supported behavior and limits.

Keep target/, data/, runtime/, caches, weights, downloaded binaries, and secrets
out of Git. Preserve licenses. MANIFEST.sha256 describes the imported archive.

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
