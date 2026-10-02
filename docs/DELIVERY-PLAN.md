# Research service delivery plan

## Approved direction

Build a Rust-centered research service that competes with Ketch on useful workflows,
source fidelity, and measured end-to-end speed. Keep the existing Rust server,
thin CLI, SQLite store, document model, and retained originals. Use specialist
helpers when they improve quality at an acceptable resource cost.

This is staged implementation work, not a backend rewrite. Deliver usable core
workflows before adding optional language-model features. A language percentage,
successful build, or nonempty document is not product acceptance.

## Core milestones

1. Compare the existing HTML/PDF paths with selected challengers on eight retained
   HTML inputs and four small PDFs. Start with Python Trafilatura and Docling.
   Record exact source losses, cold/warm time, and memory. Do not adopt a winner
   without evidence or start a large benchmark corpus.
2. Improve web search and reading. Reuse service-owned clients, bound provider
   waits, expose partial results, decode source encodings, and preserve code,
   tables, math, links, lists, and qualifications. Keep one selected extractor per
   format and bounded HTTP-to-browser recovery.
3. Expose a consistent HTTP/OpenAPI contract and an MCP connector to the same
   operations. Exercise real CLI, HTTP, and agent-client use. Give each batch input
   an explicit result or error. Add useful Python and TypeScript client examples.
4. Add code discovery and revision-pinned navigation, first-party versioned
   documentation, and optional external code/docs providers. Keep snippets distinct
   from fetched source files and show index coverage limits.
5. Add arXiv/OpenAlex scholarly discovery and selected Crossref metadata/status
   enrichment. Follow with a focused structured-paper route through Europe PMC/PMC.
   Preserve exact versions, identifiers, citations, source licenses, and honest
   abstract-only or unavailable states.
6. Complete persistent crawl frontier/resume, bounded sitemap expansion, robots
   handling, and rolling scheduling. Add explicit historical Wayback lookup/read
   with capture timestamps. Do not substitute archives silently for live pages.
7. Add bounded video search and caption selection, then explicit permitted
   single-video/native-audio downloads with progress, cancellation, format
   constraints, and storage budgets. Playlists, compatible resume, and optional
   speech recognition follow after the single-video workflow works.

## Final milestone: optional LLM workers

Only start this feature after the core milestones are practically accepted.
Ordinary search, reads, saved-library operations, crawling, and media operations
must continue to work without an LLM or configured provider.

The first optional feature is a quick answer to a search question. Interpret its
initial source limit as the first five fetched search-result pages, not five pages
of search-result listings. Keep the source count and content budget explicit.
Unavailable pages remain visible rather than being treated as read evidence.

- Provide an explicit command or flag. Do not make synthesis a default search step.
- Let the user configure and change the provider endpoint and model through the
  CLI. Support a documented compatible API for both cloud and local servers.
  Do not claim that every provider protocol is interchangeable.
- Prompt for credentials without echo. Keep credentials at the service boundary
  in protected storage, not in shell arguments, ordinary config output, artifacts,
  logs, or client responses. Do not configure a provider or contact it implicitly.
- Fetch sources through the ordinary bounded reader. Supply selected source text
  and stable source references, not arbitrary provider snippets labeled as read
  evidence. Display the sources and failed reads with the generated answer.
- Treat page text as untrusted data. The answer worker does not gain tools,
  credentials, or permission to follow instructions from a retrieved page.
- Bound source bytes, model context, generation, time, concurrency, and cost.
  Support cancellation. Cloud use must make external data transfer explicit.
- Label the answer as generated. Keep it separate from originals and parsed
  documents. Require source citations and do not claim they prove every assertion.
- Verify disabled behavior, one configured endpoint, a model change, an unavailable
  endpoint, and cancellation. Do not add a mandatory local model runtime.

No LLM implementation or model call belongs in the initial core work.

## Shared implementation boundaries

Use one Rust operation layer, one document model, one job system, and one SQLite
store. OpenAPI describes the HTTP API and does not require FastAPI. Use thin MCP
and client adapters rather than duplicate engines or databases.

Keep original bytes and parser/helper identities. Use exact source locations only
when supported by the retained artifact. Mark generated, normalized, OCR, and
uncertain locations as derived. Preserve existing snapshot IDs.

Use separate bounded capacity for network, browser, parsing, and media work.
Long downloads must not consume all ordinary reading capacity. Use isolated
browser state and process arguments, not arbitrary user-selected commands.

Optional provider adapters must report configuration, availability, errors,
unsupported filters, and partial results honestly. Keep provider budgets shared
across clients. Do not use key rotation, proxies, or mirror cascades to evade limits.

Open metadata, public visibility, and open-access labels do not automatically grant
permission to redistribute source files. Retain item-specific rights evidence.
The official YouTube API's caption and combined-client policy limits require a
separate adoption decision. An unofficial extractor is not a terms exemption.

## Verification and delivery

Use the existing locked CLI/server build and focused checks for changed behavior.
Exercise each useful workflow through its real interface with isolated diagnostic
data. Fix reproduced failures. Avoid new broad test frameworks and repeated review
rounds. Keep every content failure visible instead of hiding it in average scores.

Compare equivalent inputs, providers, and cache states. Report extraction,
networking, helper startup, queueing, and retained-evidence costs separately.
No performance superiority has been established by the planning research.

Keep task work in focused branches/worktrees. Screen exact outgoing source,
commit metadata, and reports before publication. Follow the normal pull-request
checks. Verify remote integration and installed activation separately.

The existing read-quality PR #24 remains unmerged until its preserved restriction
is explicitly lifted. Do not bypass that boundary through another pull request.
Preserve published alpha assets, current data/configuration, historical snapshots,
and rollback binaries. Paid commitments, account/credential use, remote exposure,
large model preparation, and media-source permissions retain their approval gates.
Continue independent safe work when one of these gates blocks a dependent step.
