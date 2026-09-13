# Agent handoff

## Project intent

Build a Rust-first, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Current work: live reading quality

The user approved continued live testing and correction across page types. A
successful fetch or build is not product acceptance. Keep PR #24 unmerged and
published alpha assets unchanged. Preserve the existing service configuration,
helpers, libraries, historical snapshots, and rollback builds. The narrower
historical test limits below describe previous milestones, not a ban on this
approved live pass. Use reproduced failures to choose focused corrections.

Compare actual CLI text and Markdown with retained originals. Inspect useful
qualifications, equations, list structure, action links, and code, not only HTTP
status or block counts. The extractor's quality estimate does not validate these.
Do not treat one site's HTTP denial as permanent. Do not treat an older nonempty
snapshot as evidence of a successful read without inspecting its content.

For offline CLI parsing, `ingest --name` accepts a filename, not a URL or path.
Ingestion keeps exact bytes but has no public URL base for relative links. Use a
fresh ordinary URL read to verify link resolution and retrieval behavior. Keep
intermediate parser builds in isolated data directories so cache identity cannot
reuse another implementation of the same uncommitted parser revision.

## Preserved follow-up: truthful, fast page reads

The reported page failures have a bounded correction, installed and verified from
clean build `f99fb0acbb914d7146a2dd799535b2caae3839ce`. Keep PR #24 ready and
unmerged. Do not publish or repeat installation for final documentation. Search,
settings, helpers, dependencies, schemas, libraries and old snapshots are unchanged.

Inspect retained inputs before changing selection. The Amazon response contained
only inactive payloads and recommendation chrome, not product content. The ASUS
response had real product sections beside a separate inert filter panel. A
nonempty extraction is not proof of useful source content. Lightpanda can return
its own error page with exit zero after a root navigation failure.

The installed ASUS read used HTTP without recovery: 31 blocks, 108 links, 0.201
seconds. Installed search returned five results from both providers in 0.785
seconds without warnings. These are individual timings, not a latency guarantee.
Amazon remained unavailable after one 2.519-second debug attempt. Britannica's
HTTP denial and Facebook's robots restriction are not bypassed. Small ASUS
`Filter` and `Need Help?` labels remain. See STATUS for exact proof and rollback.
Do not expand the selector, alter timeouts, or start another backlog priority
without new evidence and approved scope.

## Preserved follow-up: clean disclosure and overlay reading

The user approved another read-quality follow-up on `feat/read-quality`, PR #24.
Keep the PR unmerged and do not publish. Preserve the installed search-ad policy,
settings, helpers, dependencies, schemas, historical snapshots and alpha assets.
Focus on main content inside collapsed sections and newsletter/popup clutter.
Inspect raw and intermediate inputs first. Use a bounded diagnostic article and
retained real article/code/table inputs, not a broad corpus or new framework.
Run the normal locked build, focused checks and practical ordinary reads. After
proof, install once with rollback binaries/receipts and a freshly verified idle
project-server restart using its existing absolute config/data paths.

This follow-up is installed and verified from clean build
`c9ad49b6ecbbd9afde300cf24f335f8ccf2b9800`. Doctor and the running process match.
The installed diagnostic and FDA ordinary read stayed HTTP-only. The old saved
FDA JSON, libraries, settings and helper hashes remain unchanged. Previous
binaries/receipts are preserved for rollback. Do not repeat installation for
final documentation. Keep the PR unmerged and the remaining backlog deferred.

The active `rs-trafilatura/0.2.2` call uses `extract.rs` and html-cleaning, not the
older `extractor/pipeline.rs` pruning path. It already retains delivered collapsed
bodies. Do not assume hidden-node rules in `selector/discard.rs` affect this path.
The confirmed losses are deleted button labels, flattened summary boundaries,
and widget context removed before later filters. Correct the selection copy
before the single extractor. Do not unhide all elements, click controls, fetch
lazy panels, recover script-state content, or bypass login/paywall/consent gates.
Keep explicit CSS and originals untouched. Record unavailable selected panels
as warnings rather than inventing their content.

For an offline Rust probe, do not link an engine rlib to an independently chosen
newest protocol rlib. Cargo can retain several incompatible crate instances.
Prefer the actual CLI, or keep the probe's document types within the engine crate.

## Preserved follow-up: exclude search ads

The user approved paid/sponsored search-result exclusion on the existing
`feat/read-quality` branch and PR #24. Preserve the read improvements below and
keep the PR unmerged. This is a narrow exception to the prior search freeze, not
approval for other backlog work. Keep configured providers, ranking, settings,
dependency pins, API/data schemas, CI and published alpha assets unchanged.

Inspect paid-result context before conversion to title/URL/snippet. The pinned
search library drops that context, so URL-only filtering cannot enforce this
policy. Do not delete ordinary results because their prose mentions advertising.
Keep saved-library search and historical snapshots unchanged. Verify a bounded
paid/organic parser check and an ordinary installed search. Use the normal locked
build and existing CI, then one supported installation with matching rollback
binaries/receipts. Restart only the freshly verified, idle project server with
its existing absolute config/data paths. Do not merge or publish.

This follow-up is installed and verified from clean build
`51b42fffd958eda9f988244290ca7e690c7aef67`. Doctor and the running process match.
The installed ordinary search returned DuckDuckGo and Brave results. Focused
paid/organic checks passed. Libraries, settings and helpers are preserved.
Do not repeat installation for documentation or start other backlog work.

## Preserved milestone: reliable, clean read

Issue #23, PR #24, branch `feat/read-quality`, from updated main. Read quality was
the prior active scope. See [ROADMAP.md](docs/ROADMAP.md) for the preserved backlog.
The following constraints and evidence describe that completed work. No packaging.
Use the normal locked CLI/server build, not `cargo test` as a compilation shortcut.
The existing integration-test `Job` initializer lacks `failed`; fixing that test
belongs outside this milestone. Keep diagnostic inputs under ignored `runtime/`.

Inspect retained raw HTML and intermediate extraction before changing the existing
selector. Separate displayed main content from all-page link discovery. Preserve
short substantive pages, discussion replies, code, tables, references and captions.
Add compact default reading and `read --details` without changing JSON or originals.
Auto ordinary web reads may try configured Lightpanda once after HTTP only on
missing-content or JavaScript-shell evidence. Do not bypass denials, challenges,
limits or network errors. Keep explicit choices, native routes and crawl explicit.

The initial Rust Book and JavaScript two-page proof is complete. The user approved
a focused follow-up for table display, cell boundaries and footer/navigation
selection. Use the retained Chemistry page and saved table inputs, with bounded
excerpts. Preserve article references and independent link discovery. No suites,
fixture framework, broad corpus or benchmarks. Use debug builds, then one installer
run for this follow-up after the focused proof. Preserve previous binaries and
matching receipts for rollback. Verify current process identity before service
actions. Restart only this project's server with its existing absolute config/data
paths. Keep libraries, helpers and client settings. Return PR #24 to ready after
verification; do not merge or publish.

The table/footer follow-up is verified and installed from clean build
`dc290a7e427a35daa6db3dfd516b743b0d205e04`. Doctor confirmed the running build.
The retained Chemistry proof preserved all headings and links. Installed ordinary
Chemistry read stayed HTTP-only and excluded navigation/print footers. The initial
Rust Book/JavaScript proof remains historical evidence. Do not repeat installation
for final documentation. The search-ad follow-up above is complete. The later read
follow-ups preserve this work; other backlog priorities remain deferred. See
STATUS for IDs, rollback and limitations.

## Published alpha provenance

`v0.1.0-alpha.1` is published from exact build/source
`33ad146a9d456d4f653da00c2ae298446cdf4f31`. PR #22 merged as `ce023cd`.
Candidate-2 checksums, versions, exact-source inspection, doctor and saved read
passed with zero unresolved bounded material gaps. Retain the original and
candidate-2 artifacts unchanged. See STATUS for historical evidence.

Cargo.lock checksums identify authoritative published dependency archives. The
packager verifies extracted inputs against them; a dirty upstream Git snapshot
alone is not incompatibility. Recover source headers and demonstrably applicable
attribution files, not just LICENSE basenames. Native offline cargo metadata needs
--filter-platform matching the Rust host. Generic terms must not be labeled
revision-exact. Enumerated concerns, affected files, scopes, evidence and remedies
live in packaging/licenses/concerns.json; only unresolved records block. Use actual
compiler-artifact inventory for this build: 492 packages, not all 543 native resolved
packages. Do not infer that every resolved dependency was compiled or linked.
SOURCE-ACCESS.md gives exact source locations, checksums, no-Git build commands,
MPL Covered Source and AGPL access/availability duties. No future-source promise.
Do not bundle raw crate test-model payloads or claim Cargo.lock is complete source.

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
change was needed. Default search integration and dependency pins remain unchanged.
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

HTML parser `rs-trafilatura/0.2.2+main-content/9` uses the existing extractor's
standard selection thresholds, without recall-first or internal fallback mode.
Article comments are not appended. The extractor's Forum profile preserves replies
as main content; wider discussion coverage is not verified. All-page links stay
independent of displayed blocks. Page navigation and footer landmarks are removed
from the selection copy before their wrappers can be lost. Article-scoped footers
and endnotes remain eligible for content selection. Modal widgets and identified
email-signup overlays are removed before wrapper stripping can lose their context.
A popup label alone does not justify removal; email overlays need an email field
plus fixed positioning or a widget marker. Remove actual noscript and template
nodes before extraction. The active cleaner can unwrap noscript payloads over
500 bytes and promote their literal markup into prose. Do not remove literal
HTML examples by text matching. Inert choice-control containers are excluded
only outside articles and without substantive block structure. Supplementary
regions named as recently viewed UI are excluded outside main/article, not
ordinary sections about recommendations. Native disclosure summaries retain a
block boundary. Main/article disclosure buttons retain their labels only with a
unique same-scope target and no form fields. Hidden state stays unchanged. Empty
selected panels produce `disclosure_content_unavailable`. Missing/ambiguous target
IDs and remote-only content remain unsupported. A control's `data-modal` can identify a dialog without ARIA. Require one named
modal target and exclude main/article containers. Source selectors can use
unquoted numeric attribute values. Quote only those values for CSS parsing;
explicit user selectors still use the normal parser. Article-scoped prose footers
can contain qualifications. The active extractor's footer and disclaimer class
filters can still remove them despite its article exception. Normalize the footer
layout token and prose disclaimer labels only inside eligible article footers,
not arbitrary subscription or other filter markers. Explicit
CSS bypasses ordinary chrome filtering. Do not remove a substantive section
merely because its class says related/social.

Walk only selected containers. Preserve inline runs and structural boundaries.
The extractor's HTML can join inline text even when its text view retains spaces.
Recover whitespace only from a unique match with identical non-whitespace text,
or a matching original inline quotation boundary. Prefer actual source spacing
when it uniquely matches a complete selected run. Never add whole original
subtrees or substitute paragraphs to repair fragments. Code and table payloads
come from unique original element matches. Keep derived mappings explicit.

The active cleaner deletes MathML and the HTML serializer drops fallback images.
A selection-copy code carrier preserves supplied TeX at the selected position.
Only a surviving generated carrier becomes a Math block. Unannotated MathML uses
a source-required marker. No equations are appended from rejected source regions.
Original bytes, explicit CSS, and math inside code/tables are not changed. Numeric
prose superscripts/subscripts use Unicode notation. Other script text uses an
explicit `^(...)` or `_(...)` representation. Citation links are not exponent text.
The exact `id-lock-subscription` citation-label class can falsely remove a visible
title. Neutralize only that token on its inline citation shape, not subscription
filters globally or access gates on linked destinations.

Optional metadata preserves selected inline link spans, list context, and source
inline-math flows without changing protocol fields. Link spans use half-open UTF-8
byte offsets in block text. They do not come from matching labels against the
all-page link inventory. List context keeps nesting, ordinals, and continuation
blocks. Original start/reversed/value attributes require a unique source match.
Compact math flows join only adjacent selected fragments with retained source
whitespace. Detailed views keep separate provenance blocks. Invalid or missing
metadata falls back to ordinary block rendering. Non-default Markdown ordinals
need explicit list boundaries or labels because Markdown engines can renumber them.

Non-UTF-8 decoding is absent. Markdown parsing implements a limited block reader,
not full CommonMark. General image, table-nesting, mathematical, and inline-style
fidelity remain incomplete. This is not general extraction-quality validation.

Text and Markdown share saved table cell values. Text grids support multiline
ASCII cells and horizontal spans within 88 columns; uncertain widths or row spans
use compact nonaligned rows. Markdown still uses raw HTML for merged tables, not
a rendered preview. Cell conversion preserves list-item and br boundaries instead
of concatenating their text. Explicit CSS uses source-blocks/6 and retains access
to complete original tables, including excluded navigation regions.

For table defects, compare retained original list-item boundaries with saved rows
before changing presentation. The Chemistry authority-control footer contained
joined list items and omitted entries despite retained row/cell spans. Its original
`role="navigation"` wrapper disappeared during extraction, so post-extraction
filtering could not identify it. Remove such landmarks from the selection copy
first. A correct row matrix does not establish content fidelity, and unmatched
selected tables can still reflect upstream omissions. Do not restore guessed
values or whole unselected subtrees. Inspect a saved original without a network
refresh. A block-range JSON read still includes the document link list; project
only the blocks when bounded output is needed.

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
Capture version lightpanda-json-dom/3 uses fetch JSON content/http_status/url and
explicit done plus readyState-complete waits. 0.3.6 can exit zero after a fatal
fetch/wait diagnostic or a root-frame navigation error, returning a synthetic
`Navigation failed` DOM. Inspect stderr and the envelope, not exit status alone.
Reject the root-frame error before extraction. Child-frame failures do not
invalidate readable main content. Cache identity changes do not rewrite old IDs.
Current upstream CLI docs differ from 0.3.6 help (including wait defaults and
--fail-on-http-error); do not assume newer flags work on this binary.
DOM originals and selectors are snapshot-relative, never original HTTP responses.
Null/unknown navigation remains explicit. Local JS code/table/base-link/export and
public quote extraction passed; public fragments remain derived. Readiness is not
application completeness. Helpers remain process-per-request with bounded resources.
Auto ordinary web reads use HTTP first, then at most one configured Lightpanda
attempt for typed missing content or combined application-shell evidence. The
overall read deadline includes queueing; recovery does not recursively acquire
read locks. Preserve useful HTTP partial content on browser failure. Reject login,
challenge and loading-only output. Keep actual renderer/base/locations and separate
HTTP/DOM artifacts in metadata. `http-lightpanda-recovery/1` versions routing cache
identity; old saved IDs are not rewritten. Explicit renderers/selectors, native
GitHub/arXiv/caption routes and HTTP-only crawling remain separate.
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

Search supports ordinary web results only. Paid-result filtering is always on at
the server's HTML-to-result boundary, before URL unwrapping, limits and merging.
The pinned library's flat result cannot preserve sponsored container context.
Use the local result parser and retain the merge URL guard. Web search is not
cached; library search remains separate. See [SEARCH.md](docs/SEARCH.md) for
markers, evidence and exact limitations. Do not promise absence of concealed ads
or unknown future markup. Image, video, news, date, language and domain-filter
interfaces remain incomplete. There are no semantic rerankers or automatic LLM calls.

## Remaining source fidelity

Continue from reproduced live failures, not an assumed completed product. Image
layout tables, source-currency metadata, inline emphasis/code styles, and ambiguous
mappings still need work. Do not turn these into a Markdown parser rewrite or an
optional-integration validation campaign without a concrete need.

## Boundaries not to expand

No enterprise accounts, private workspaces, Redis, vector service, Kubernetes, or mandatory LLM runtime.
No default browser automation actions such as clicking, typing, or checkout.
No hidden fallback chain across several extraction engines.
No claim that Rust percentage or binary size proves lower total deployment cost.
