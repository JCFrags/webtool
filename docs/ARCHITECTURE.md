# Architecture

## One application, two binaries

`webtool` is a thin command-line client.
`webtoold` owns fetching, extraction, saved data, and expensive processing.
They share the protocol crate but not the engine dependency graph.

The server exposes one HTTP API.
Optional browser and media helpers are managed subprocesses, not additional public services.
SQLite and original files remain on the server's local filesystem.

## Read flow

1. The client submits a URL or uploads bytes.
2. The server checks the selected reader and configured limits.
3. Duplicate simultaneous URL reads share an in-process source lock.
4. A fresh saved result can satisfy a normal URL read.
5. The selected transport retrieves bytes within its resource budget.
6. The server retains the original content by SHA-256.
7. The appropriate reader produces structured blocks and warnings.
8. The server saves the document and updates its search index.
9. A library receives a reference when explicitly requested.
10. The client renders the same document as text, Markdown, or JSON.

No retrieval operation implicitly calls an LLM.
No extractor ladder or fast-versus-quality profile exists.

## Data model

`Document` contains its source, parser version, blocks, links, metadata, and warnings.
Each source references an original artifact with a SHA-256 hash.
A document ID identifies the accepted content and extraction configuration outcome.
A different original, parser version, or normalized result produces a different ID.

A block has content and a source locator.
Locators distinguish HTML selectors, original line ranges, timestamps, pages, sheets, slides, and JSON pointers.
A `derived` locator explicitly means that an exact original position is unavailable.

Libraries are many-to-many references to documents.
Notes and tags are attributed annotations, not permissions.
All connected users can read every library and saved document.

## Storage

The application opens SQLite connections on blocking tasks.
Write transactions remain short and unrelated to network retrieval.
WAL permits readers alongside database writes.
FTS5 indexes document titles and retained text.
Literal `find` uses the saved blocks instead of treating identifiers as FTS expressions.

Original files use content-addressed paths under `objects/`.
Writes use temporary files and an atomic rename.
Reads verify the stored bytes against their hash.
Document JSON remains the stable API representation.

## Concurrency

Tokio coordinates network requests, helper processes, and job status.
Semaphores bound accepted reading work, network operations, parsing, browser helpers, and crawl jobs.
CPU readers run on blocking tasks while retaining a parsing permit.
Several clients share this capacity rather than launching complete independent research stacks.

These limits bound individual operations, not a demonstrated sustained-load service capacity.
Multipart request buffering and large serialized document outputs need additional load testing.

## Jobs

Crawl submissions are retained as queued jobs before execution.
A bounded local worker pool marks them running and records saved documents.
Clients can poll, stop waiting, reconnect, or request cancellation.

Cancellation does not remove already saved documents.
Restarted servers mark previously running jobs interrupted.
Queued jobs are rescheduled.
The frontier is not yet persisted, so this is not exact resumable crawling.

## External integrations

The native metasearch crate supplies search-engine adapters.
The application owns result merging and warning presentation.
rs-trafilatura selects content while the application maps selected blocks back to original HTML where possible.

Xberg is feature-gated and called as a library.
The application disables its independent cache and retains upstream structures.
Model processing remains explicit and requires separate deployment validation.

Lightpanda, Chromium, and yt-dlp use bounded helper execution without shell commands.
The optional fastCRW adapter is explicitly selected and does not replace other transports silently.
The current helper transport is a prototype, not the intended final warm-browser implementation.

## Expansion

Academic collections can add identifiers, bibliography records, and citation relationships around existing documents.
Media analysis can add derived artifacts while retaining captions, timestamps, and original media references.
Explicit LLM jobs can produce separately labeled outputs linked to their source documents.

Do not change source text to make generated output look authoritative.
Do not add those capabilities before the core compiles and its fidelity tests pass.
