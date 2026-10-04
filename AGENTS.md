# Agent handoff

## Project intent

Build a Rust-centered, shared research service with a conventional text CLI.
The users are a small trusted group, not separate enterprise tenants.
Every library is visible to every connected user.
Do not introduce a TUI, permission hierarchy, quality profiles, or distributed job infrastructure.

## Current work: Sprint 1 and the Ketch gate

PR #24 is merged. The accepted research core is delivered from
`d961aeb0407607fa3e414b5ebafc08b6c6191fe0`. Its published alpha assets remain
unchanged. Do not repeat that delivery to start the next sprint.

The user approved parallel Sprint 1 development in four areas: Documents,
GitHub/code, Backend, and Linux/Docker product readiness. Make installation and
operation usable by other people without workstation-specific paths or personal
configuration. Keep the shared Rust service and thin CLI. See
[the two-sprint plan](docs/SPRINTS.md) for scope and acceptance.

Sprint 2 covers optional answers, scholarly/bibliography extensions, search
extensions, and media follow-ons. Do not start Sprint 2 or merge Sprint 1 on
GitHub until the Ketch comparison gate passes. Compare pinned builds on the same
inputs. Measure complete input-to-output wall clock, system resources, and result
quality. A brand-new subagent must perform subjective quality checks with product
identities hidden. Webtool must meet or exceed Ketch quality and complete
wall-clock performance in overlapping keyless workflows. Larger binaries and higher CPU/RAM are allowed
because Webtool does more, but resource use must not be grossly larger than needed.
Report complete costs and check concrete avoidable bloat. No fixed 10% ceiling
applies. Keyed adapters remain in scope for source-contract and local/mock
confidence checks. Record live-provider limits rather than claim untested parity.

Ordinary research stays LLM-free. Optional answers remain in Sprint 2, not an
implicit search step. Credentials, paid calls, cloud model transfer, model
preparation/use, live media, and exposure changes retain separate decisions.

The earlier freezes below are historical. Their source-fidelity and preservation
rules still apply. Preserve the installed service, data, configuration, helpers,
historical worktrees, snapshots, rollback files, and published alpha assets until
a verified replacement is ready. Use focused branches/worktrees and isolated
finite-lived diagnostic services. Verify remote integration and installed
activation separately after the gate. Do not bypass normal checks.

Historical activation claims below are dated evidence, not current service health.
Check the actual process and configuration before any deployment action.

## Preserved baseline: live reading quality

The user approved continued live testing and correction across page types. A
successful fetch or build is not product acceptance. Keep PR #24 unmerged and
published alpha assets unchanged. Preserve the existing service configuration,
helpers, libraries, historical snapshots, and rollback builds. The narrower
historical test limits below describe previous milestones, not a ban on this
approved live pass. Use reproduced failures to choose focused corrections.

The current correction is installed and active from clean source
`44f6a45e2acef20dfda215555349da236c3ee1fd`. CI and actual installed Chemistry,
MDN, Britannica, and news reads passed. These four reads used HTTP without
recovery. The earlier binary pair and matching receipts remain available for
rollback. Do not reinstall for later documentation-only changes. See STATUS for
exact observations and remaining gaps, including Amazon readiness failure.

Compare actual CLI text and Markdown with retained originals. Inspect useful
qualifications, equations, list structure, action links, and code, not only HTTP
status or block counts. The extractor's quality estimate does not validate these.
Do not treat one site's HTTP denial as permanent. Do not treat an older nonempty
snapshot as evidence of a successful read without inspecting its content.

For offline CLI parsing, `ingest --name` accepts a filename, not a URL or path.
Ingestion keeps exact bytes but has no public URL base for relative links. Its
selected blocks can differ from an ordinary URL read even for identical bytes.
Use a fresh ordinary URL read to verify selection, link resolution, and retrieval. Keep
intermediate parser builds in isolated data directories so cache identity cannot
reuse another implementation of the same uncommitted parser revision.

For original exports, use `export ID --kind original --output /path/to/file`.
The output is a file path. `--output -` creates a literal file named `-`, not
stdout. Read the exported file to compare bytes. Preserve existing files unless
replacement is intended and explicitly selected with `--force`.

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

### Shared Cargo targets across worktrees

Use an external build lock for every Cargo command when workers share a target
directory. Copy diagnostic binaries to a private evidence directory before you
release that lock. Another worktree can overwrite the target's binaries.

A lock prevents concurrent writes, but it does not make Cargo's cached workspace
artifacts source-exact. The observed dependency files use relative workspace
paths. Cargo reused another worktree's protocol artifact when its build was newer
than this worktree's source mtimes. This caused missing local types and an extra
`Job` field from the other source without compiling the local protocol.

After you acquire the lock, update mtimes for the existing crate roots
`crates/protocol/src/lib.rs`, `crates/engine/src/lib.rs`,
`crates/server/src/lib.rs`, and `crates/cli/src/main.rs`, plus existing workspace
`build.rs` files, before Cargo runs. Change no file contents and touch only the
current worktree. This forced the current packages to compile and preserved shared
dependencies. A per-worktree target also avoids this reuse, but do not duplicate
large build trees or remove another worker's evidence only for recovery.

### Private diagnostic process lifetime

Before a practical check starts a server or stdio client, set a finite lifetime
and a close reserve. Require `0 < close reserve < lifetime`. At startup, record
an absolute work deadline at `start + lifetime - close reserve` and a close
deadline at `start + lifetime`. A per-call timeout or a duration in a receipt
does not enforce the complete check's lifetime.

Record commands before execution and process IDs, start times, and executable
identities before use. Check identity and remaining work time before each
operation. Bound HTTP reads, subprocess waits, and stdio waits by the remaining
time. Stop when the work budget is insufficient. Do not replace an expired
process to complete the same scenario.

Before shutdown signals, recheck ownership. Wait for and reap each owned process.
Verify that its private listener is closed within the close deadline. Preserve
the commands, deadlines, results, and closure receipts beside the proof.

## Deferred optional integration boundaries

Xberg 1.1.1's existing APIs compiled with the committed lockfile; no dependency
change was needed. Default search integration and dependency pins remain unchanged.
OCR/ONNX model readiness and fastCRW APIs still require separate work. Do not
upgrade the search dependency from =0.3.1 (0.3.2 creates a search-tui cycle).

The fastCRW adapter is experimental and does not own the crawler yet.
The locked crw-renderer 0.34.0 `fetch` takes `render_js: Option<bool>` before
`wait_for_ms: Option<u64>`. Keep the browser wait in the latter argument.
A successful feature check does not establish runtime behavior or source-access policy.
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

HTML uses one bounded decoded view for gates, extraction, links, and saved CSS
replay. See [ENCODING.md](docs/ENCODING.md) for BOM/HTTP/meta precedence, decoded
limits, replacement warnings, and exact original retention. XHTML and captured
DOM remain explicitly UTF-8-only. Current source uses HTML main-content/12 plus
html-encoding/1, CSS source-blocks/9, and recovery /2. Earlier identities above
describe preserved snapshots. Markdown uses pinned CommonMark parsing and exact
source slices for complete, unchanged groups. Selected styles and image references
remain source-bound. General image layout, table nesting, mathematical, and
ambiguous style mappings remain incomplete. See [DOCUMENTS.md](docs/DOCUMENTS.md).
The bounded fixtures do not establish general quality or installed activation.

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

The Xberg adapter uses source-blocks/3. Default output is plain, quality rewriting
is disabled, and OCR is disabled when absent. Supplied DOCX structures, PPTX notes,
and XLSX grids have a small authored-fixture proof. Internal OOXML relationships
and numbering corroborate source facts without another extractor or external
fetch. Missing or ambiguous structures retain text with warnings. PDF keeps native
page order and uses supplied bounding boxes only for unique complete text matches.
Native tables remain heuristic supplements and can misidentify aligned prose.
Metadata retains the first upstream document and the full output envelope.
Downloadable figures, fine-grained universal mapping, scans, real PDF table
accuracy, and general Office layouts remain unverified or unsupported. Do not
remove supplemental_table_blocks used to label repeated table text. Empty
extraction fails. Empty pages in partial documents produce warnings. See
[DOCUMENTS.md](docs/DOCUMENTS.md) for exact format scope and model prerequisites.

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

Crawl uses a rolling same-depth FuturesUnordered scheduler. Checkpoint and attach
each usable document before refilling a vacant slot. Deeper work waits for its
current-depth siblings. Keep bounded candidate admission, including exclusions,
query semantics, and fresh HTTP reads so ordinary-read caches cannot bypass
redirect scope or robots checks. Started attempts are charged before waits and
network activity. Interrupted attempts keep that charge. Explicit resume can
consume another charge for the interrupted URL. `visited` counts completed page
attempts. Failed pages, extraction warnings, and interruptions remain separate.
Zero usable results after attempted reads is Failed, not successful Partial.
Cancellation retains saved library documents. Old payloads deserialize new counts
with zero and have no resumable frontier. No historical recount is implied.

The bounded frontier is durable in additive SQLite schema 2. Both queued and
running jobs become interrupted at startup. Startup makes no hidden fetch or
rescheduling request. Resume is explicit and requires pending work and remaining
attempt budget. Completed failures and exclusions are not retried. Opt-in explicit
or robots-advertised sitemap trees have separate candidate, attempt, depth, body,
and scope limits. Compressed sitemap metadata is refused. Robots handling is
stricter but is not full RFC 9309 compliance. See [CRAWL.md](docs/CRAWL.md).

A private small graph verified early library use, rolling refill, cancellation,
hard restart, and explicit resume: seven completed page entries, five saved
documents, two failed pages, nine charged attempts, and two interruptions. The
sitemap phase admitted 32 candidates and made eight metadata attempts. This is
not every crash checkpoint, a large public crawl, or production migration proof.
Before schema-2 activation, preserve a consistent prior database plus originals,
binaries, and receipts. Schema-1 binaries refuse schema 2. Rollback restores the
prior database, never lowers `user_version`.

GitHub auto routing now supports root README, actual blob bytes, and immediate
nonrecursive tree listings. source_resolver=github-source/2 versions cache keys.
Resolve refs via bounded candidates (8 maximum); require explicit SHA or encoded
slash boundary when ambiguous. Traverse at most 16 path components through pinned
Git trees; do not follow symlinks/submodules or replace native errors with HTML.
Directories retain API JSON, derived locations, and explicit scope/truncation
warnings. README link supplements are limited, not a full CommonMark parser.
Ordinary URL routing of issues, PRs, and releases remains unsupported. Explicit
public object list/body/comment operations, admitted-map comparison, and lexical
declaration/context navigation use the separate [code interface](docs/CODE.md).
Complete-repository ingestion remains unimplemented.
The earlier arxiv-abstract-html/2 native resolver used official abstract-page
HTML only. The same cond-mat/0207270v1 paper passed four-page PDF reading,
body find, original-byte comparison and offline saved-ID BibTeX/CSL. A local probe
returned HTTP 200 and its retained HTML matched the product's metadata artifact.
Earlier API HTTP 500 results were local observations, not a global-outage diagnosis.
No application retry was used in that earlier pass. Wire-level retry counts were
not measured.
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
forms were source-inspected, not an extra paper corpus.

Current scholarly operations explicitly select arXiv or OpenAlex discovery,
Crossref singleton metadata, or exact-version arXiv inspection. Resolver
arxiv-abstract-html/3 adds selected-version license evidence. Full-text reuse must
be established before the scholarly or automatic native arXiv route fetches a PDF.
Metadata or abstract-only content is not a paper body. Saved historical IDs and
citations remain available. Source precision, literal author order, attributed
status claims, rights, and observation age stay explicit. See
[SCHOLARLY.md](docs/SCHOLARLY.md). Explicit PMC OAI metadata and permitted JATS
reading are documented in [PMC.md](docs/PMC.md). Version/datestamp assertions do
not select arbitrary history. Missing formula graphics and external objects
remain partial and source-required. Europe PMC REST is not implemented.

Pinned repository discovery, maps, selected-file search, exact file reads, and
explicit docs.rs releases are separate from the ordinary GitHub URL resolver.
See [CODE.md](docs/CODE.md) for revision identities and coverage limits.
Explicit dated archive lookup/read never replaces live reading silently. See
[ARCHIVES.md](docs/ARCHIVES.md) for capture identity and replay limits.

Ordinary web search supports web results only. Paid-result filtering is always on at
the server's HTML-to-result boundary, before URL unwrapping, limits and merging.
The pinned library's flat result cannot preserve sponsored container context.
Use the local result parser and retain the merge URL guard. Web search is not
cached; library search remains separate. See [SEARCH.md](docs/SEARCH.md) for
markers, evidence and exact limitations. Do not promise absence of concealed ads
or unknown future markup. Image, video, news, date, language and domain-filter
interfaces remain incomplete. There are no semantic rerankers or automatic LLM calls.
Explicit video search, track inventory, and exact-language caption origin selection
use shared bounded helper admission. Search descriptions are metadata, not
transcripts. Explicit single-media preview and transfer use the common jobs system
with separate capacity. They default disabled and require finite operator/caller
allocations and permitted sources/access methods. Exact native inputs remain
retained beside stream-copy derivatives. Transcription, playlists and compatible
media resume remain unsupported. See [VIDEO.md](docs/VIDEO.md) and
[MEDIA-JOBS.md](docs/MEDIA-JOBS.md).

## Shared connector foundations

The HTTP API exposes generated OpenAPI at `/openapi.json`, safe bounded Problem
errors, and Python/TypeScript client examples. Engine producers select typed error
kinds with stable codes, status intent, and fixed public messages. Anyhow retains
internal context and causes. HTTP and batch use typed downcasts, never prefixes
or formatted messages. Add new typed variants at the producer and preserve unknown
internal faults as 500. See [API.md](docs/API.md) and [ERRORS.md](docs/ERRORS.md)
for migration limits and native-helper protocol exceptions.

The locked reqwest 0.12.28 client retries protocol negative acknowledgments by default. For a new
transport with a no-retry or exact-request-budget contract, set
`retry(reqwest::retry::never())` explicitly on the client builder. No retry loop
in application code does not establish this behavior. This requirement does not
claim that all legacy clients already disable retries.

`webtool mcp` is a bounded stdio HTTP adapter to the same server. It starts no
listener or engine. Saved-ID passages preserve exact continuation and original
artifact references. Exercise `tools/list` after schema changes: tagged enum
schemas need explicit root object types. See [MCP.md](docs/MCP.md). Source and
local diagnostic verification do not establish installed activation.

## Remaining source fidelity

Continue from reproduced live failures, not an assumed completed product. Image
layout tables, source-currency metadata, and ambiguous inline-style mappings still
need work. The source-bound style and CommonMark changes do not establish universal
fidelity. Do not expand parsing or optional-integration validation without a
concrete need and the required approval.

## Boundaries not to expand

No enterprise accounts, private workspaces, Redis, vector service, Kubernetes, or mandatory LLM runtime.
No default browser automation actions such as clicking, typing, or checkout.
No hidden fallback chain across several extraction engines.
No claim that Rust percentage or binary size proves lower total deployment cost.
