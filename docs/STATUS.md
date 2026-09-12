# Implementation and validation status

Imported snapshot date: September 9, 2026.
Bootstrap verified: September 10, 2026 UTC (September 9 local).

## Milestone 12: reliable, clean read

Issue [#23](https://github.com/JCFrags/webtool/issues/23), PR
[#24](https://github.com/JCFrags/webtool/pull/24), branch `feat/read-quality`.
Started from updated main `ce023cd`. The sole active priority and deferred work
are in [ROADMAP.md](ROADMAP.md). No search, provider, ranking, configuration,
dependency, API/data-version, CI, packaging, tag, or release changes.

### Implementation and focused proof

- Inspected the retained Rust Book HTTP input and quote-page HTTP/DOM inputs,
  plus rs-trafilatura's intermediate HTML/text before changing selection.
  The Rust page has navigation outside its main content. The quote HTTP source
  has quote data only inside a script that writes the cards. Old ordinary read
  failed with no structured HTML, not a network or access-denial error.
- HTML uses the existing extractor with standard thresholds, no recall-first or
  internal fallback mode, and no separately appended article comments. Upstream
  Forum selection treats replies as content. Short content is not rejected by
  an arbitrary reader length floor. Links stay independent for map/crawl/extract.
- Default text has a compact header and one full saved ID. `read --details` shows
  block IDs, selectors, retrieval/original metadata and full mapping diagnostics.
  Default output omits link inventories and image URLs. Exact code, table cells,
  source page numbers and caption times remain. JSON schemas and Markdown exports
  are unchanged; interactive Markdown has a separate clean renderer.
- Parser identity is `rs-trafilatura/0.2.2+main-content/5`. Selected inline text is
  grouped without restoring removed subtrees. The extractor's HTML joined quote
  and author text even where its text view retained whitespace. Unique whitespace-
  only matches and matching original inline boundaries repair that defect without
  replacing paragraphs or changing their order. Missing MathML notation is marked
  as source-required; retained originals remain complete.
- Auto ordinary web reads use HTTP first, with at most one configured Lightpanda
  attempt after missing-content or combined shell evidence. HTTP errors, access
  denials, challenges and limits are not browser retries. Rendered login/challenge/
  loading-only content is rejected. Useful HTTP partial content survives failure
  with a warning. Existing semaphores and one overall HTTP-plus-helper deadline
  bound the operation; there is no recursive read or second extractor.
- Normal locked CLI/server debug builds passed. The existing unused HashMap import
  warning remains. The final quote-spacing correction passed against the retained
  DOM, without another public request. Failed spacing checks were corrected at the
  actual extraction boundary, not with global markup stripping.
- Ordinary Rust Book Data Types read passed with no observed server child/helper
  process and metadata renderer `http`, recovery false. All 16 code payloads and
  both table matrices exactly matched the historical saved document. All 33 table
  cells remain in the default output. The raw 44,687-byte HTTP input is unchanged.
- Ordinary `https://quotes.toscrape.com/js/` read passed with one observed Lightpanda
  process, actual HTTP 200 and ten quotes in 21 blocks. Login, navigation and footer
  text are absent. Separate artifacts retain the initial 5,808-byte HTTP response
  and 8,986-byte DOM. Metadata identifies the accepted renderer and selection reason.
- Both original exports matched retained objects byte for byte. JSON parsed with
  unchanged top-level schema. Historical saved IDs remain readable; a saved static
  JSON response through the new client matched the previous client byte for byte.
  Details/default output were inspected. No all-page link discovery was removed.

Private proof and before/after outputs are under ignored `runtime/read-quality/`.
Rollback copies of both installed binaries and matching checksum receipts are in
`runtime/read-quality/rollback/`, alongside a consistent pre-restart database copy.
Only this project's verified server was switched to the debug build, with its
existing absolute config/data paths. Installation and active-build verification
are pending at this implementation checkpoint.

### Limits and check-scope deviation

Only the two specified public pages are the live acceptance corpus. General forum,
short-page, cookie/widget, challenge, failure, and MathML coverage is not established
by this proof. They follow the inspected selection/guard paths, not a new campaign.
Superscript/list/Markdown fidelity, broader layout and application completeness
remain limited. Chromium and fastCRW were not exercised. See the preserved roadmap.

A worker exceeded the no-suite instruction. Two filtered `cargo test` attempts
(default and no-default-features) failed on an existing integration-test `Job`
initializer missing `failed`. A later library-only run passed five existing HTML
unit tests. It also attempted synthetic conversion/encoding checks. Further worker
checks were stopped; tests were not changed and these runs are not milestone
acceptance evidence. The worker removed its task-owned test/synthetic artifacts.
The required acceptance remains the normal build and the real two-page workflow.

## Published alpha baseline

PR #22 merged as `ce023cd`; issue #21 closed. `v0.1.0-alpha.1` is published with
its tag at exact artifact build `33ad146a9d456d4f653da00c2ae298446cdf4f31`.
The release and candidate-2 files remain unchanged. Historical milestone sections
below describe their status at the time, not a pending publication instruction.

## Milestone 11 continuation: candidate-2

The original archives below are superseded local evidence and remain unchanged.
Continue issue #21 / PR #22 on chore/alpha-packaging from 100172ee. No new issue,
branch, feature, dependency change, installation, merge, tag or publication.

### Candidate-2 results

Artifact build/source SHA: `33ad146a9d456d4f653da00c2ae298446cdf4f31`.
Build-checkpoint CI passed, run 34566822304. The single incremental locked native
release build passed in 47.94s with the existing unused-import warnings. The
corrected script assembled all archives without an assembly retry or rebuild.

Files under ignored `runtime/dist/candidate-2/`:

| Archive | Bytes | SHA256 |
|---|---:|---|
| webtool-0.1.0-alpha.1-x86_64-unknown-linux-gnu-client.tar.gz | 3901567 | 79bf7f8a6ff73a1e94c33971ea31300017c5822eb449a725ca7a9f43af0b405e |
| webtool-0.1.0-alpha.1-x86_64-unknown-linux-gnu-host.tar.gz | 24340902 | 2443c9920e727db2ac3fca1d650326c9987bd0fab87a0d4be4207447c0788a1b |
| webtool-0.1.0-alpha.1-source.tar.gz | 598601 | b3c2bfed4cfabded3d13ea4fb6495d6a4bff4a61cf0ae356641b7c986df74533 |

Technical packaging readiness: passed. SHA256SUMS verifies all three archives.
Both unpacked binaries report 0.1.0-alpha.1. Packaged doctor and the existing saved
read exit 0 against the installed server. The retained source-mapping warning is
unchanged. All 78 allowlisted Git files, including the corrected packaging script,
match the build checkpoint byte for byte. BUILD-INFO.json is identical in all
archives, and its binary hashes match the unpacked executables. No source file was
changed to manufacture provenance. SOURCE-ACCESS.md has resolved commit/target
references and explicit URLs/checksums for all seven observed MPL packages.

The runtime dependency inventory contains 492 observed registry packages.
LOCKED-SOURCES.json lists all 827 locked registry archives, including unused
optional/other-platform entries. simd_helpers, profiling-procmacros and zune-inflate
were not observed in this build. Their concern records describe their dependency
roles conservatively, not a verified linked-code map. Their recovered notices
remain in the exact project-source archive. The binary packages include notices
for observed dependencies. An initial inspection assertion incorrectly expected
an unobserved dependency's notice in the binary packages; the remaining check was
corrected to follow the actual inventory. No package rebuild was needed.

Distribution readiness for the bounded milestone: supported, with zero unresolved
generated material gaps. Eleven concern records identify files, scope, evidence,
missing obligations and applied remedies. Applicable SearXNG AGPL terms and changed
file notices are retained, not overridden by the search crate's MIT declaration.
libdeflate's applicable MIT attribution is retained in source supplements. Original
port revisions remain unknown; pinned comparison revisions are identified honestly,
not asserted as the port origin. This is not general legal clearance.

The exact project revision is publicly accessible, and selectors registry APIs
confirm both source versions/download paths and lock checksums. All required build
archives and extracted files passed local checksum comparison. Distribution must
keep equivalent source access available as SOURCE-ACCESS.md requires. No publication
point was created, and no future source bundle is promised. Network deployments
must offer source for their actual deployed version, not blindly reuse alpha links.

The host requires GLIBC_2.43 and OpenSSL 3; the CLI requires GLIBC_2.34 and OpenSSL 3.
Rust: 1.98.0, Fedora 1.98.0-1.fc44, native x86_64-unknown-linux-gnu. No portability,
static-linking or reproducible-binary claim. A no-Git source build is documented,
not an additional build proof. Old archives still pass their original checksums.

The restored installed server's PID/start identity and all post-restoration file
hashes, including the database, remain unchanged after candidate-2 checks. No
candidate installation/startup, new ingestion, provider request, suite, benchmark,
merge, release tag or publication occurred. PR #22 may be marked ready on this
bounded evidence; the issue remains open until an authorized merge.

### Restored local use

The user superseded the no-start restriction for the unchanged installed server
only. No webtoold process or port 8420 listener existed. The installed binary was
started from /tmp with the existing absolute config/data paths, appending to the
existing runtime/webtoold.log and replacing the stale runtime/webtoold.pid record.
No signal was sent to the stale PID. Actual PID: 18184, started September 11, 2026
at 05:22:24 UTC. It owns 127.0.0.1:8420 and reports installed build
984376ed71b695c53f720c6ab8896ba310a37d11, not an alpha candidate.

Only the two failed original-candidate steps were retried: doctor and the saved
read both exited 0. The read returned the existing Rust Book "Data Types" document,
ID 04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded, with its retained
source-mapping warning. Connection refusal was not a demonstrated client defect.
The existing database contains 18 documents, seven libraries and one job. Its
file hash differs from the earlier milestone baseline; no byte-unchanged claim
is made across startup. A post-restoration baseline records current hashes and
process identity for candidate-2 preservation checks. Binaries, receipts, helpers,
config and client-settings presence still match the original baseline.

### Corrected packaging scope

The packager accepts --out-dir runtime/dist/candidate-2/ and refuses collisions.
It checks published archive checksums and extracted dependency files against the
committed Cargo.lock, not .cargo-checksum.json or an upstream Git dirtiness flag.
All 543 native resolved registry archives and their extracted files passed this
comparison before the corrected build. Normal features and API/data versions stay.

SOURCE-ACCESS.md supplies exact source pointers, locked download URLs/checksums,
MPL Covered Source locations, no-Git build instructions, AGPL section 6(d) access
and section 13 network-source duties. Separately hosted sources require equivalent
access and continuing availability; this is not a promise of a future bundle.
The source archive contains the corrected packaging script at the binary build
checkpoint. Concerns and remedies are enumerated in packaging/licenses/concerns.json,
not an unconditional publication blocker. Technical and distribution results are
recorded separately above.

## Milestone 11: local alpha candidates

September 11, 2026 UTC. Issue #21 / draft PR #22, chore/alpha-packaging.
PR #20 merged at authorized head ae7d05927245b65bc1e18fdb556d3042c30a474b
with green CI and --match-head-commit. Main merge: 6a99b00. Issue #19 closed.

Build SHA: `ec909cc6da5c7ba6e90f960d0b11070804e18d00`.
Its CI build passed (run 34549817517). Both binaries built once, locked/offline,
normal defaults, native x86_64-unknown-linux-gnu release, in 2m 23s. Existing unused
imports produced warnings. No dependency versions changed; only four workspace
lock entries changed to 0.1.0-alpha.1. API/stored-document versions are unchanged.

### Candidate artifacts and proof

Files under ignored `runtime/dist/`:

| Archive | Bytes | SHA256 |
|---|---:|---|
| webtool-0.1.0-alpha.1-x86_64-unknown-linux-gnu-client.tar.gz | 3846962 | 362c644057ca8c2f2953e1101a5356cd1f0c9e5731d319ce001fbe2ae513dac4 |
| webtool-0.1.0-alpha.1-x86_64-unknown-linux-gnu-host.tar.gz | 24286743 | d5d6120e3bdcca8603505f625a64fd891964c1ce29273ae095c0d3fc5475d3fa |
| webtool-0.1.0-alpha.1-source.tar.gz | 513861 | 2e3e048207e38e89efdd6fcf392da38c2871d266dab437c10966635d822bd5eb |

- All three entries in SHA256SUMS passed after assembly. Fresh unpacked binaries
  both report 0.1.0-alpha.1. All three BUILD-INFO.json files are identical and name
  the actual build SHA. All 61 allowlisted Git source files match that commit byte
  for byte, including Cargo.lock and build/install/package scripts.
- Archives exclude runtime/data, imported MANIFEST.sha256, models, target outputs
  other than the intended binaries, helpers, machine paths and downloaded papers.
  No installed file was replaced. No ingestion, public retrieval or suite ran.
- The first assembly failed because this Cargo cache lacks .cargo-checksum.json.
  Only archive assembly was retried, using committed Cargo.lock checksums and the
  conservative native-target resolved dependency inventory. The successful binary
  build was not repeated. The final script fixes that assumption. The source
  candidate intentionally retains the original build-checkpoint script, which has
  this known assembly defect. Assembly recovery is disclosed in BUILD-INFO.json.
- Both ELF binaries link OpenSSL 3, glibc, libgcc and libm. The CLI requires symbols
  through GLIBC_2.34; the host through GLIBC_2.43. Consult BUILD-INFO.json for all
  symbol versions, interpreter, toolchain and platform. The archived generic notes
  understate the CLI's OpenSSL requirement; both binaries need it. Current ALPHA.md
  corrects that wording. No portability/static/reproducibility claim is made.
- Packaged `--server http://127.0.0.1:8420 doctor` and saved `read` both exited 1
  with connection refused and actionable diagnostics. The attempted saved ID was
  04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded.
  This is a blocked server-dependent proof, not a successful saved read.

### Deployment and publication status

PID 102510 and its listener were already absent before this milestone and remain
absent. No server was stopped, restarted or started. Installed CLI/server binaries,
installation receipts, helpers, client-settings presence, server config and database
hashes all match the pre-packaging baseline. The last installed server binary was
built at 984376ed71b695c53f720c6ab8896ba310a37d11, not the alpha checkpoint.

The original license notice and complete AGPL terms are included, together with
recovered third-party notices. Ten aggregate/generated material-gap entries remain.
See THIRD-PARTY.md for missing notices, SearXNG/libdeflate provenance, dirty xberg
snapshot qualification, MPL Covered Source and AGPL Corresponding Source obligations.
profiling-procmacros also lacks a directly collected notice in this candidate.
The project source archive is not a complete dependency source offer.

PR #22 remains draft because the bounded server proof and publication materials are
incomplete. No merge, release tag, publication or installation occurred. Further
server-dependent proof requires a separately restored existing deployment. Keep
these candidate files unchanged; a corrected distribution needs separate approval
and a new artifact set rather than silently replacing these hashes.

Private proof logs and unpacked files remain under ignored runtime/alpha-proof/.
The final post-build changes fix packaging checksum lookup and documentation only;
they do not change binary implementation or the recorded build checkpoint.

## Milestone 10: readable everyday text CLI

Verified September 10, 2026, feat/readable-cli-output, issue #19 / PR #20.
PR #18 squash-merged at its authorized unchanged head with green CI and
--match-head-commit. Main merge: d75ab5f. Issue #17 closed.

### Presentation and actual proof

- Added a small CLI presentation module for saved/library entries, confirmations,
  jobs and selected extracts. Protocol plain rendering separates headings/prose,
  fenced code, tables, quotes and captions. No retrieval/parser changes.
- Full IDs, URLs and available source/date fields remain untruncated. Jobs expose
  actual state, visited/saved/failed counts, configured limits, saved IDs and errors.
  Warning counts are visible, with details/progress on stderr.
- Tables use an 88-column ASCII grid only for fitting rectangular single-line cells
  with compatible source header flags. Other tables use labeled cells preserving
  row/column spans, header flags, empty cells, zero and multiline values. Non-ASCII
  and tabbed cells use this conservative fallback rather than guessing display width.
  Supplemental-table labels remain. Code is never wrapped or line-prefixed.
- `cargo build --locked -p webtool-cli` passed. Existing unused-anyhow warning remains.
  The only local release build/install was scripts/install-local.sh --client-only,
  which passed in 13.42s. Installed /home/mainpc/.local/bin/webtool matches the release
  output and its valid updated installation receipt.
- Inspected library items for install-shared: title shared.txt, full saved ID, upload
  URL, original retrieval timestamp and warning count. README has a short actual
  before/after excerpt replacing the former default JSON fields.
- Inspected existing finished crawl 71debce3-5c1f-4dcc-85d6-2d6b69e0ed8f. It retains
  state Partial, not Complete: 3 visited, 3 saved, 0 failed, limits 3 pages/depth 1,
  all three full IDs and six warning details on stderr. This is the only existing
  job. No new crawl was started to manufacture a Complete-state example.
- Read saved Rust Book HTML ID
  `04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded`.
  Inspected ordinary prose/headings, 16 code blocks and two tables. All retained
  code strings occur unchanged in both read and code-extract text; no indentation
  repair or wrapping was applied. Tables aligned and labeled actual header rows.
- New document/code JSON parsed equal to old installed-client responses. Table
  JSONL parsed equal to old table JSON and retained exactly one-line framing.
  Machine serializers, stored schemas and Markdown export implementation are unchanged.
- One `read ID | head -n 5` returned pipeline statuses 0/0 without a broken-pipe
  error. Existing extraction warnings remained on stderr.
- No public requests, fixtures, suites, benchmarks, broad lint or command matrix.
  Wider/ragged/merged/empty-cell presentation branches, link/outline rendering and
  mutation confirmations were source-inspected, not separately exercised. No
  library/job mutations or extra documents were created for this milestone.

### Unchanged server and limits

The installed server was not rebuilt, replaced or restarted. PID 102510 and its
process-start identity remained unchanged. One listener remains at 127.0.0.1:8420.
Doctor still reports server build 984376ed71b695c53f720c6ab8896ba310a37d11. The server
binary, server installation receipt, runtime/media-config.toml and SQLite database
hashes match the pre-work snapshot. Helper configuration was not changed or used
for retrieval. Client-only installation preserved the server installation receipt.

No concrete blocker. This is a bounded everyday presentation pass, not broad
format/extraction validation or a published alpha. Markdown display escapes terminal
controls; Markdown export bytes remain unaffected. No new formats, short IDs,
search changes, dependency upgrades or CI changes.

### Commands and retained evidence

All smoke output and before/after comparisons are in ignored runtime/readable-proof/.
The installed CLI contains source checkpoint fa2a68c09a01b03eb7108a64ca28c2f3c61227a5;
later documentation-only commits do not require another installation.

```sh
CLI=/home/mainpc/.local/bin/webtool
ID=04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded
"$CLI" library items install-shared
"$CLI" jobs 71debce3-5c1f-4dcc-85d6-2d6b69e0ed8f
"$CLI" read "$ID"
"$CLI" extract "$ID" code
"$CLI" extract "$ID" tables
"$CLI" --format json read "$ID"
"$CLI" --format jsonl extract "$ID" tables
"$CLI" read "$ID" | head -n 5
```

These use existing saved data. Do not repeat successful proof steps or restart the
server for this presentation change. Prior installation/startup commands remain below.

## Milestone 9: installable client and shared-server setup

Verified September 10, 2026. Issue #17, PR #18, branch feat/install-connect.
PR #16 merged with unchanged authorized head and green build through
`--match-head-commit 93c3870f7ad9e13af76211c66a46c1ea27fb6611`.
Main merge: 1d5924b. Issue #15 closed. No release or tag was published.

### Delivered and verified

- Local `connect` validates an HTTP(S) URL and atomically saves client TOML without
  contacting a server. Existing unrelated settings survived the update. Config
  lives at XDG_CONFIG_HOME/webtool/client.toml or HOME/.config/webtool/client.toml.
- `config show` reports endpoint/path/source. Verified localhost default, saved
  setting, WEBTOOL_SERVER and --server precedence. JSON remains machine-readable.
- Offline connect to port 9 succeeded. Doctor then failed with an actionable
  connection-refused error, selected endpoint and explicit no-server-start message.
- Normal locked debug build passed. One locked optimized build through
  `./scripts/install-local.sh` passed in 1m54s and installed both binaries into
  /home/mainpc/.local/bin. Existing unused-import warnings remain. No suite,
  benchmark, source re-fetch, optional build or installation matrix was run.
- Installer client-only branch selects only webtool-cli; source-inspected, not
  separately built as a second installation matrix. CLI has no engine dependency
  or storage initialization. It reuses already pinned TOML/tempfile crates; lock
  changes add only those two CLI dependency edges. No version/pin/CI changes.
- Installer requires a user-owned bin directory, refuses unrelated existing files,
  and records checksums for later owned-file updates. It does not use sudo, edit
  profiles, download runtime helpers/models, start services or open ports.
- The old runtime PID file was stale and port 8420 had no listener. A fresh process
  check found no webtoold. No stale PID was signaled. Installed webtoold started
  from /tmp with explicit absolute config and data arguments. Its log printed
  the intended paths, loopback address and build commit.
- Existing data/webtool.sqlite3 retained its inode. Document count changed from
  17 to 18 and library count from 6 to 7 after the single upload/library creation.
  Preserved runtime/media-config.toml remained byte-identical, including helpers.
- Client A and B used separate temporary XDG_CONFIG_HOME directories and installed
  CLI commands from /tmp. A created install-shared and uploaded shared.txt. B
  connected independently, listed the shared item, read its text/indentation and
  exported 72 bytes identical to the input.
- Saved ID: `4ed73eb0f9d58b9aab56b01561c1c1b3c593bcbeab9d8a13313f89ee0926fe20`.
  Both doctors reported http://127.0.0.1:8420, source saved client settings and
  build `984376ed71b695c53f720c6ab8896ba310a37d11`.
- Installed binary code matches that implementation checkpoint. Subsequent docs-only
  commits do not require another release build. Health's optional build_commit
  field defaults for older responses; text/JSON doctor reports unknown when absent.
  Dirty code builds are labeled. Backward compatibility/unknown paths were inspected,
  not separately exercised against another server.

No concrete blocker. This proves separate local client configurations sharing one
server, not connectivity from another physical machine. No LAN/firewall changes,
public authenticated-service claim, account/TLS infrastructure or systemd/Docker
management. Relative server paths retain cwd semantics with a data-directory warning;
installed deployments must use explicit absolute paths. Older readers and parser
behavior were not changed or re-fetched. Prior milestone evidence follows below.

### Installed commands and artifacts

Binaries: /home/mainpc/.local/bin/webtool and /home/mainpc/.local/bin/webtoold.
Checksum receipts: sibling .webtool.install-sha256 and .webtoold.install-sha256.
Proof inputs/JSON/exports/configs/log are under ignored runtime/install-proof/.
The working server remains on loopback. Its PID is in runtime/webtoold.pid.

Start only after confirming no existing copy is running:

```sh
cd /tmp
/home/mainpc/.local/bin/webtoold \
  --config /home/mainpc/Projects/webtool/runtime/media-config.toml \
  --data-dir /home/mainpc/Projects/webtool/data
```

The following commands describe the completed proof; do not repeat successful
library creation or uploads merely to reproduce this report:

```sh
cd /tmp
CLI=/home/mainpc/.local/bin/webtool
PROOF=/home/mainpc/Projects/webtool/runtime/install-proof
unset WEBTOOL_SERVER
export XDG_CONFIG_HOME="$PROOF/client-a"
"$CLI" connect http://127.0.0.1:8420
"$CLI" library create install-shared
"$CLI" --format json ingest "$PROOF/shared.txt" --library install-shared >"$PROOF/ingested.json"
ID=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["id"])' "$PROOF/ingested.json")
export XDG_CONFIG_HOME="$PROOF/client-b"
"$CLI" connect http://127.0.0.1:8420
"$CLI" config show
"$CLI" doctor
"$CLI" library items install-shared
"$CLI" read "$ID"
"$CLI" export "$ID" --kind original --output "$PROOF/exported-b.txt"
cmp "$PROOF/shared.txt" "$PROOF/exported-b.txt"
```

Use the saved ID and existing client B config for further local inspection without
uploading again. No global client endpoint was changed. Keep previous binaries and
matching receipts before later updates; never remove existing server data/config
as part of an installation rollback.

## Milestone 8: arXiv abstract HTML and saved-paper citations

Verified September 10, 2026 on feat/arxiv-paper-reading, issue #15, PR #16.
Continued from 4514e349ca283657ea5606ddef78de92f01638ab without another branch,
issue or merge. The user superseded API-first resolution. Earlier official API
requests returned HTTP 500 twice from this host. Those results did not establish
a global outage; no API request was repeated in this continuation.

### Source and identity

One direct local diagnostic request to
https://arxiv.org/abs/cond-mat/0207270v1 returned HTTP 200 with 40,735 HTML bytes.
Raw response, headers and status are retained as runtime/arxiv-abs-local.html,
arxiv-abs-local.headers and arxiv-abs-local.status. The source page selected v1,
while its canonical URL/PDF citation tag were unversioned and its dateline mentioned
latest v3. Selection was established from the article's arxividv "for this version"
row and the unlinked [v1] history marker, not v1's mere occurrence in history.

The reading path now uses only official abstract-page HTML with the existing HTTP
client and scraper. No Atom lookup, browser, proxy, alternate provider or metadata
extraction engine. It checks selected-version/history agreement, requested/final
identity, citation IDs/PDF link, canonical identity, breadcrumbs and full-text
links. Contradictions or absent selection evidence fail. Unversioned canonical
links do not establish a version. Unversioned requests resolve from verified
selection before PDF download; explicit versions are never changed to latest.

Resolver cache identity: arxiv-abstract-html/2. Shared process-wide pacing, existing
Xberg/storage and offline saved-ID citation branch remain. Metadata provenance now
records source_url, status, retrieved_at, metadata_origin and a text/html artifact.
Legacy stored paper metadata remains deserializable; citation origin labels are
not hardcoded as API metadata. PDF bytes remain the original document export.

### Actual workflow

- Normal locked debug build passed after correcting one local variable-name
  compiler error. No dependency/pin/CI/parser changes or suites. Only this project's
  server restarted with runtime/media-config.toml and a port-8420 listener check;
  helpers and stored data were preserved.
- Auto read the SAME cond-mat/0207270v1 URL into existing library papers. Both HTML
  and pinned PDF returned actual HTTP 200. PDF: 186,660 bytes, four reported pages
  and one supplemental table. Plain full text and page locators were inspected.
- Saved ID: `eb1093898329143122e016461a3ab3e1ca6bc0aab584f1229e1b25da454a16bc`.
  Resolved URL: https://arxiv.org/pdf/cond-mat/0207270v1.
- The page-1 body/figure-caption phrase "magnetic moments occupy" was found at its
  page locator and confirmed absent from retained abstract metadata.
- Exported original PDF matched the retained downloaded artifact byte-for-byte.
  SHA-256: 9648e76d23f607423cc32c7f0cfa7af97223fb57f824644ff4820803ce0543f5.
- Product metadata HTML matched the initial local response byte-for-byte, 40,735
  bytes, SHA-256 50a07c9c508bcfaba415d6ae9c9a2d8c0799fb1ef09b69d4183cefabe73e73f7.
- Title: Understanding Paramagnetic Spin Correlations in the Spin-Liquid Pyrochlore
  Tb2Ti2O7. Ordered literal display authors: Ying-Jer Kao, Matthew Enjalran,
  Michel J.P. Gingras. Original citation meta author strings are retained separately;
  names are not rearranged or split into family/given components.
- Selected submission timestamp: 2002-07-10T17:10:30+00:00. The later v3 date
  (2003-03-02) remains only in raw source/dateline metadata. The PDF also prints
  a typeset date in 2019; that was not used to infer a bibliographic date.
- Categories: cond-mat.dis-nn and cond-mat.stat-mech. The page supplies arXiv DOI
  10.48550/arXiv.cond-mat/0207270; it is retained separately as arxiv_doi. No journal
  DOI/reference was present, so those fields were omitted.
- Saved-ID BibTeX and CSL exports inspected: title, ordered literal names, v1 eprint
  and stable abstract URL agree with metadata. BibTeX year=2002; CSL issued date is
  [2002,7,10]. The saved-ID branch reads only storage and the citation formatter;
  it performs no HTTP request. No journal substitution or inferred publisher.
- Xberg warnings for supplemental tables and partial structure stayed visible.
  Mathematical notation remains intact in metadata/source bytes; extracted PDF
  subscripts, columns and mathematical layout are not reconstructed.

No remaining concrete blocker. Only this paper/version was exercised. Other ID
forms and contradictory/missing metadata branches were source-inspected, not a
synthetic fixture or extra corpus. The parser deliberately depends on selected
version markup and fails if that contract changes. No fallback is attempted.

### Working commands

Server remains http://127.0.0.1:8420. Startup, only when no other copy is listening:

```sh
cd /home/mainpc/Projects/webtool
./target/debug/webtoold --config runtime/media-config.toml
```

The papers library and outputs already exist. Successful steps were not repeated:

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
webtool --format json read https://arxiv.org/abs/cond-mat/0207270v1 --library papers --refresh >runtime/arxiv-paper.json
DOC_ID=$(python3 -c 'import json;print(json.load(open("runtime/arxiv-paper.json"))["id"])')
webtool read "$DOC_ID"
webtool find "$DOC_ID" 'magnetic moments occupy'
webtool export "$DOC_ID" --kind original --output runtime/arxiv-original.pdf --force
HASH=$(python3 -c 'import json;print(json.load(open("runtime/arxiv-paper.json"))["source"]["original"]["sha256"])')
cmp "data/objects/$HASH" runtime/arxiv-original.pdf
webtool cite "$DOC_ID" --as bibtex >runtime/arxiv-paper.bib
webtool cite "$DOC_ID" --as csl >runtime/arxiv-paper.csl.json
```

Use the saved ID directly to inspect existing evidence without network retrieval.
No content access implies permission to redistribute PDFs; observe the paper's
license. The previous API-first instructions are superseded, not an active retry
plan. Historical completed milestones follow below.

## Milestone 7: incremental crawl libraries

Verified September 10, 2026. Branch feat/crawl-library-workflow, issue #13, PR #14.
PR #12 was squash-merged only at 8f447a8ed69eacc76e0e262b03e5d1307b557de9 with
unchanged head, green build and --match-head-commit. Main fast-forwarded to
93d5403; issue #11 confirmed closed. Runtime config, helpers and stored data kept.

### Delivered and verified

- Replaced whole-batch collect with incremental next() consumption. Each completed
  usable document attaches to its library and persists job progress before waiting
  for another result. Same-depth batches retain breadth-first semantics. No new
  scheduler, parser, dependencies, browser crawling or resumable frontier.
- Fresh HTTP reads enforce same-origin and robots restrictions on redirect hops,
  rather than reusing an ordinary-read cache entry with different redirect policy.
  Query order/repeated keys remain unchanged; fragments are deduplicated away.
- Failed completed attempts consume budget and increment failed separately from
  extraction warnings. visited counts completed attempts, excluding pending or
  cancelled reads. Zero usable documents after attempted reads is Failed. Job wait
  prints final JSON then exits nonzero for Failed/Interrupted. Saved documents
  survive cancellation/restart; running jobs still recover as Interrupted.
- Normal locked debug build passed. One local site, one crawl, max-pages=3 and
  max-depth=1. The slow child waited 15 seconds. Before it responded, the fast
  child was attached and job state persisted as Running, visited=2, saved=2,
  failed=0. Saved read, doctor, and library search all worked during that interval.
  Evidence timestamp was 14.951 seconds before the delayed response completed.
- Final job: Partial, visited=3, three saved IDs, failed=0, error=null. Warnings
  explicitly separate bounded scope, robots exclusion, depth limit and document
  extraction warnings. All three IDs appear in library crawl-proof. Library search
  found "Cobalt meadow library beacon" both during and after crawling.
- Request log: exactly one each for /robots.txt, /, /fast?b=2&a=1&a=3, and /slow.
  No duplicate requests despite repeated/fragment links; no /blocked or /beyond
  requests. Query order and repeated a parameters stayed intact.
- Post-proof source inspection found that early exclusion checks must also count
  toward the existing 10,000-URL discovery bound. Corrected admission order and
  rebuilt normally. The passed crawl was not rerun. No suite, framework, benchmarks,
  public campaign, or unrelated regression runs. Final server runs this correction.
- Temporary fixture server stopped after proof. Only this project's server was
  restarted, with listener checks and the preserved runtime/media-config.toml.
  PDF, GitHub, captions, Lightpanda, lockfile, pins and single CI build unchanged.

Job: `71debce3-5c1f-4dcc-85d6-2d6b69e0ed8f`.
Saved IDs (root, fast, delayed):

- `28997c0bdcc05670f9bfde4651fd33d80102478fff6fa9eaab589b1753a0e1fe`
- `c597bf9f71bf6156ea6878626a374d97e1cebccd00645e400314840422aa4800`
- `626153858e5486a9c6783b1bdfc9d4bf8e3c589b960478cff397be07e71b0701`

Evidence: runtime/crawl-incremental.json, crawl-final.json, crawl-items.json,
crawl-search.json, crawl-fast.txt, crawl-progress.stderr and crawl-requests.jsonl.
No concrete blocker. All-failed, redirect denial, explicit cancellation and restart
branches were source-inspected, not exercised in an additional campaign. Robots
support remains partial. Frontiers are not persisted. New failed fields default
to zero for historical payloads, not a retroactive count. Batch admission still
waits for siblings; publication no longer does. Attachment and job-update writes
are separate: a crash between them can leave a library item absent from job IDs.

### Commands and local fixture

Server remains http://127.0.0.1:8420. Do not start a second copy:

```sh
cd /home/mainpc/Projects/webtool
./target/debug/webtoold --config runtime/media-config.toml
```

The fixture below is temporary, not a test framework. Run from the checkout.
It refuses to overwrite an existing request log. Before a repeat, move the old
runtime/crawl-requests.jsonl to a new evidence filename and use a fresh library.

```sh
cat >runtime/crawl-site.py <<'PYTHON'
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from pathlib import Path
import json, time

LOG = Path('runtime/crawl-requests.jsonl')
LOG.touch(exist_ok=False)

def record(path, phase):
    with LOG.open('a') as f:
        f.write(json.dumps({'path': path, 'phase': phase, 'time': time.time()}) + '\n')

class Site(BaseHTTPRequestHandler):
    def do_GET(self):
        record(self.path, 'received')
        status = 200
        if self.path == '/robots.txt':
            body = 'User-agent: *\nDisallow: /blocked\n'
            mime = 'text/plain'
        else:
            mime = 'text/html'
            links = ''
            if self.path == '/':
                title = 'Crawl root article'
                phrase = 'This page links to two readable child articles.'
                links = '<a href="/fast?b=2&amp;a=1&amp;a=3">Fast</a> <a href="/fast?b=2&amp;a=1&amp;a=3#repeat">Repeat fast</a> <a href="/slow">Slow</a> <a href="/slow#repeat">Repeat slow</a> <a href="/blocked">Excluded</a> <a href="/blocked">Excluded again</a>'
            elif self.path == '/fast?b=2&a=1&a=3':
                title = 'Fast child article'
                phrase = 'Cobalt meadow library beacon is the distinctive phrase for this completed child.'
                links = '<a href="/beyond">Beyond depth one</a> <a href="/">Root again</a>'
            elif self.path == '/slow':
                time.sleep(15)
                title = 'Delayed child article'
                phrase = 'The delayed child arrives after the fast child is already available in the library.'
                links = '<a href="/beyond">Beyond depth one</a>'
            else:
                status = 404
                title, phrase = 'Unexpected request', 'This path should not be fetched.'
            body = f'<!doctype html><html><head><title>{title}</title></head><body><main><article><h1>{title}</h1><p>{phrase}</p><p>This small technical article provides readable source content for the bounded crawl. The server keeps exact response bytes, converts the selected article, and publishes each accepted document to a shared library. A slow sibling must not delay access to a completed article.</p>{links}</article></main></body></html>'
        data = body.encode()
        self.send_response(status)
        self.send_header('Content-Type', mime + '; charset=utf-8')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
        record(self.path, 'responded')
    def log_message(self, *args):
        pass

ThreadingHTTPServer(('127.0.0.1', 8769), Site).serve_forever()
PYTHON
python3 runtime/crawl-site.py
```

In another terminal (the verified library crawl-proof already exists):

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
# Create this only for a fresh reproduction:
# webtool library create crawl-proof --description 'Incremental three-page crawl'
webtool --format json crawl http://127.0.0.1:8769/ --library crawl-proof --max-pages 3 --max-depth 1 >runtime/crawl-submitted.json
JOB=$(python3 -c 'import json;print(json.load(open("runtime/crawl-submitted.json"))["id"])')
# While /slow remains pending, list the fast child and read its saved ID:
webtool library items crawl-proof
webtool read FAST_DOCUMENT_ID
webtool doctor
webtool jobs "$JOB" --wait
webtool library items crawl-proof
webtool search 'Cobalt meadow library beacon' --library crawl-proof
cat runtime/crawl-requests.jsonl
```

Stop only the fixture with Ctrl-C after completion. Keep webtoold running.
The historical milestones below retain their original proof results.

## Milestone 6: Lightpanda JavaScript reading

Verified September 10, 2026. Branch feat/lightpanda-reading, issue #11, PR #12.
PR #10 squash-merged at authorized 31ed31e2fb451dae70c615ea7d8437f8ac281a30 with
unchanged head, green build, and --match-head-commit. Main fast-forwarded to
179ca9f. Issue #9 remained open after merge and was explicitly closed as completed.

### Helper and interface

Reused /home/mainpc/.local/bin/lightpanda. `version` returned 0.3.6. Release:
https://github.com/lightpanda-io/browser/releases/tag/0.3.6 (release ID 359777982).
Installed SHA-256 matched the official Linux x86_64 release asset:
`e438c0ad44e0f6916c14cf13beb003512c60438d8fd200738d2e596e73f652d6`.
No binary or dependencies installed. Setup for a machine where it is absent:

```sh
mkdir -p "$HOME/.local/bin"
gh release download 0.3.6 --repo lightpanda-io/browser --pattern lightpanda-x86_64-linux --output "$HOME/.local/bin/lightpanda"
echo 'e438c0ad44e0f6916c14cf13beb003512c60438d8fd200738d2e596e73f652d6  '"$HOME/.local/bin/lightpanda" | sha256sum -c -
chmod +x "$HOME/.local/bin/lightpanda"
LIGHTPANDA_DISABLE_TELEMETRY=true "$HOME/.local/bin/lightpanda" version
LIGHTPANDA_DISABLE_TELEMETRY=true "$HOME/.local/bin/lightpanda" fetch --help
```

Interface checked against installed help, https://lightpanda.io/docs/reference/cli/fetch,
and source at release commit f72cba80a82eeeefdd8161688d78de70612b7a47, inspected
in a shallow research clone. No upstream source was copied, built, or executed.
Current docs describe newer flags/defaults than 0.3.6. This adapter uses only the
verified flags and JSON fields (content, dump, http_status, url). It explicitly
requests done quiescence then readyState complete within browser_wait_ms=2000.
The second condition prevents the 0.3.6 waitForAll deadline from silently passing
as quiescence. Neither condition guarantees future/application completeness.
0.3.6 logs some fatal fetch failures yet exits zero; stderr and output validation
are required. No --fail-on-http-error assumption or timed-only success fallback.

### Actual proof

- Normal locked debug build passed. Local fixture passed without a retry.
- HTTP original: 1,496 bytes, identical to authored input. JavaScript inserted
  "Lightpanda quartz lantern arrived" and Rust code after a 100 ms timer. The
  marker was absent from HTTP bytes but present in the 1,963-byte retained DOM.
- Default extraction retained the marker, exact four-space/tab/newline code,
  Rust language, a header cell with colspan=2, and snapshot-relative selectors.
  A DOM base tag resolved next.html to http://127.0.0.1:8768/manual/next.html.
- Saved in library javascript, read as ordinary text, found the marker at its
  HTML selector, exported 1,963 bytes and cmp matched the retained object.
  Export explicitly identified rendered_dom rather than original HTTP bytes.
  Source status was actual 200; final URL was the reported local URL.
  Low extractor-confidence warning stayed visible despite successful checks.
- Public https://quotes.toscrape.com/js/ produced an 8,986-byte DOM containing
  JavaScript-generated quotes. Initial extraction dropped selected container text
  and returned only heading/login. This was a failure, not useful page reading.
- Small correction in the existing source-block converter retains selected direct
  div/span text and uses derived locations. HTML parser revision is now
  rs-trafilatura/0.2.2+source-blocks/3, also included in cache keys. No original
  subtrees, alternate extractor, selector workaround, or substitute page used.
- Rebuilt normally after that correction and retried only the failed public read.
  It passed with ten readable quotes, 52 blocks, actual HTTP 200 and reported final
  URL. Fifty text fragments have explicitly derived locations. Quote/tag grouping
  is coarse; this was not a general HTML cleanup. The local proof was not repeated.
- Capture version lightpanda-json-dom/2 and helper path/wait configuration are in
  cache identity. The DOM, not envelope/diagnostics, is the original artifact.
  HTTP/Auto/GitHub routing, PDF defaults, captions config, stored documents,
  dependency pins, lockfile, and single CI build remain unchanged.
- Only this project's server was restarted, checking port 8420 before starting.
  Temporary fixture server bound loopback port 8768; it was stopped after proof.
  No suites, framework, benchmarks, browser comparison or unrelated regressions.

Local ID: `601901df1661c0695ea15d04ed1d67529e757c088093d7dddfaa2c1393ee0dcd`.
Final public ID: `554249b123992ceb990111014d669600da59a1873dbd3683781b37d949cb1c0c`.
No concrete blocker. Error/missing/timeout branches were source-inspected rather
than a failure campaign. Process-per-request, incomplete web-platform support,
coarse selected inline fragments and application-specific readiness remain limits.
JSON overhead counts toward stdout cap; each subresource is byte-bounded, not the
aggregate page transfer. Diagnostics use the existing 1 MiB stderr bound. No pool.

### Reproduce

The existing ignored runtime/media-config.toml now includes lightpanda_path and
retains yt-dlp and Node settings. To roll back configuration, remove only the active
lightpanda_path line. Do not replace the config or delete stored documents.
Server startup (leave existing listener alone until a guarded restart is needed):

```sh
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
./target/debug/webtoold --config runtime/media-config.toml
```

Fixture creation and loopback serving (temporary output only):

```sh
mkdir -p runtime/lightpanda-site
cat >runtime/lightpanda-site/index.html <<'HTML'
<!doctype html><html><head><title>JavaScript source fidelity</title><base href="/manual/"></head><body><main><article>
<h1>JavaScript source fidelity</h1>
<p>This small technical article describes a browser capture. The initial response provides this introduction. A script adds the measured content after a short timer. The reader must retain the resulting document, preserve source locations, and distinguish that document from the response sent by the local server.</p>
<div id="result"></div>
<script>
setTimeout(() => {
 const host = document.getElementById('result');
 const p = document.createElement('p');
 p.textContent = ['Lightpanda', 'quartz', 'lantern', 'arrived'].join(' ') + '. This paragraph was inserted by JavaScript after navigation. Its presence proves that the capture executed the script rather than reading only the initial page.';
 host.appendChild(p);
 const pre = document.createElement('pre');
 const code = document.createElement('code'); code.className = 'language-rust';
 code.textContent = 'fn quartz() {\n    let answer = 42;\n\tprintln!("{}", answer);\n}\n';
 pre.appendChild(code); host.appendChild(pre);
 const table = document.createElement('table');
 table.innerHTML = '<tr><th colspan="2">Captured values</th></tr><tr><td>quartz</td><td>42</td></tr>';
 host.appendChild(table);
 const link = document.createElement('a'); link.href = 'next.html'; link.textContent = 'Next source page'; host.appendChild(link);
}, 100);
</script></article></main></body></html>
HTML
printf 'User-agent: *\nAllow: /\n' >runtime/lightpanda-site/robots.txt
python3 -m http.server 8768 --bind 127.0.0.1 --directory runtime/lightpanda-site
```

In another terminal, using the existing library javascript:

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
webtool --format json read http://127.0.0.1:8768/index.html --renderer http --refresh >runtime/lightpanda-http.json
webtool --format json read http://127.0.0.1:8768/index.html --renderer lightpanda --library javascript --refresh >runtime/lightpanda-local.json
ID=$(python3 -c 'import json;print(json.load(open("runtime/lightpanda-local.json"))["id"])')
webtool read "$ID"
webtool find "$ID" 'Lightpanda quartz lantern arrived'
webtool export "$ID" --kind original --output runtime/lightpanda-export.html --force
HASH=$(python3 -c 'import json;print(json.load(open("runtime/lightpanda-local.json"))["source"]["original"]["sha256"])')
cmp "data/objects/$HASH" runtime/lightpanda-export.html
webtool read https://quotes.toscrape.com/js/ --renderer lightpanda --refresh
```

Stop the temporary fixture server with Ctrl-C. Keep webtoold running.
Historical milestones below describe their state at the time, not current gaps.

## Milestone 5: GitHub source reading

Verified September 10, 2026. Branch `feat/github-source-reading`, issue #9, PR #10
(not merged). PR #8 was squash-merged at authorized
4500215be8d3449130890783e9f66e8265fb0781 after confirming its unchanged head and
green build, using --match-head-commit. Main fast-forwarded to `3d7df72`;
issue #7 closed. No local changes or runtime data were discarded.

- Normal locked debug build and source-checkpoint CI passed, existing warnings
  only. Dependencies, PDF defaults, captions config, search pins, and CI unchanged.
- Fresh directory read: JCFrags/webtool/tree/main/crates/engine/src resolved to
  `3d7df722944aafe26b6f3b1311649fe46b8b2060`. Nine immediate entries with pinned
  blob/tree links; all listing blocks derived. Original API JSON: 2,107 bytes,
  truncated=false. Visible repository_directory_only warning; no child ingestion.
- Followed its sources.rs link with refresh and saved in library github. Actual
  Rust source, not HTML: 2,312 bytes, 33 lines, Rust language label, exact whitespace.
  Metadata retained requested SHA, repository, path, and resolved commit;
  source.version is that same commit. File read had no warnings.
- Find github_readme returned the source block, lines 1–33. Plain output inspected.
  Export cmp matched `git show COMMIT:crates/engine/src/sources.rs` from the already
  existing checkout. No cloning or application gh subprocess was introduced.
- Startup guard found a stale PID: the old process was gone and port 8420 had no
  listener. After checking this, one server was started with the preserved
  runtime/media-config.toml. No unrelated process was stopped or data deleted.
  No source request failed or needed a retry.

Behavior version: github-source/2 participates in read cache keys. Native routing
is Auto-only without CSS; explicit HTTP/browser remains explicit. Root README
scope warning remains. Files use native readers with real filenames; directory
originals are API JSON and readable listings are derived. Missing references,
paths, public access, rate limits, incomplete trees and unsupported objects have
separate errors. Ref resolution checks at most eight candidate splits; full SHAs
or percent-encoded reference slashes give explicit boundaries. Path traversal is
limited to 16 components. All content requests follow immutable commit/tree/blob
identities, not mutable download URLs. No repository-wide ingestion.

No concrete blocker. Only the requested directory/file workflow was exercised.
Slash-ref ambiguity, encoded paths, missing/rate-limited objects, root README link
supplements, and truncation handling were inspected in source, not separate live
checks. Symlinks/submodules are unsupported. GitHub API response bounds include
base64 overhead; no authenticated quota is configured. README link supplements
handle common inline/reference definitions, not complex CommonMark or unmarked
directory destinations. Highest-value next gap: improve README link interpretation
and directory-target links without changing retained source text.

### Working commands

Server remains at http://127.0.0.1:8420. Startup (do not start another copy):

```sh
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
./target/debug/webtoold --config runtime/media-config.toml
```

Reproduction uses existing library github and replaces only temporary exports:

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
webtool --format json read https://github.com/JCFrags/webtool/tree/main/crates/engine/src --refresh >runtime/github-directory.json
DIR_ID=$(python3 -c 'import json;print(json.load(open("runtime/github-directory.json"))["id"])')
webtool read "$DIR_ID"
FILE_URL=$(python3 -c 'import json;print(next(l["url"] for l in json.load(open("runtime/github-directory.json"))["links"] if l["text"]=="blob: sources.rs"))')
webtool --format json read "$FILE_URL" --library github --refresh >runtime/github-file.json
FILE_ID=$(python3 -c 'import json;print(json.load(open("runtime/github-file.json"))["id"])')
webtool read "$FILE_ID"
webtool find "$FILE_ID" github_readme
webtool export "$FILE_ID" --kind original --output runtime/github-export.rs --force
COMMIT=$(python3 -c 'import json;print(json.load(open("runtime/github-file.json"))["metadata"]["github"]["resolved_commit"])')
git show "$COMMIT:crates/engine/src/sources.rs" >runtime/github-pinned.rs
cmp runtime/github-pinned.rs runtime/github-export.rs
```

These commands reflect the current main commit at proof time; after main changes,
use its reported commit and a symbol in that revision (or use the recorded SHA).
Directory ID: `f05e3e00e5127e6ec7f857a031d67ca955d3a6a834fcf4c092b485f3e37ebc86`.
File ID: `cce49eb3f720cc8d684ff37ecfb0a9c4fb00a2d784f579a3cf76833ca2bbe26a`.
No suites, new test framework, benchmarks, broad lint or unrelated regression
reruns were used. The historical milestones below describe their prior state.

## Milestone 4: YouTube captions

Verified September 10, 2026 UTC. Branch `feat/media-captions`, issue #7, PR #8.
PR #6 was squash-merged at authorized 25059c5d42fef93bdb980817b89c5fe210b187fe
with a green build and --match-head-commit. Main fast-forwarded to `02edc6b`;
issue #5 closed. Runtime data was preserved; PR #8 is not merged.

- Normal locked debug build passed; source checkpoint CI passed. PDF defaults,
  search versions, Cargo.lock, and CI are unchanged. Added only the existing nix
  dependency's resource feature for per-helper file-size limits.
- Official PyPI yt-dlp 2026.08.19 + yt-dlp-ejs 0.8.0 installed locally with uv.
  Existing `/usr/bin/node` v24.18.0 satisfies the official EJS Node >=22 requirement.
  No runtime, ffmpeg, impersonation package, browser, cookies, proxies, models,
  transcription, or audio/video download was used.
- One source: https://youtu.be/jNQXAC9IVRw (“Me at the zoo”, 19 seconds).
  Default auto read selected six provided English cues, saved in library captions.
  Stable video metadata, language/origin, and null (not invented) HTTP status retained.
- Timestamped plain output and original VTT inspected. Find “elephants” returned
  b1, 1200–3360 ms. First cue exactly matches original:
  `All right, so here we are, in front of the\nelephants`.
- Exported 440 bytes equal the verbatim downloaded track retained in data/objects/;
  SHA-256 `5c7fcf32df4558291ee896d133264e6efea2b39ffbadd36f764ce9e8c392ec71`.
- Nonfatal helper warning: no impersonation target available. Caption retrieval
  still succeeded; the warning is preserved, not suppressed. No source blocker.
- Automatic-origin selection, absent-language/blocked-source errors, timeout
  cleanup, and language cache separation are implemented but not separate live
  tests. No provider sweep, test suite, broad lint, benchmark, or smoke retry ran.

Media/native-caption parser revisions are 2. `read --renderer auto` routes only
supported YouTube video URLs to captions; explicit http/CSS remains HTML. Exact
language matching prefers provided over automatic tracks and rejects translated
`tlang` URLs. yt-dlp's own downloader receives selected track/source headers via
temporary load-info-json, not a generic HTTP subtitle fetch. TempDir cleanup,
process groups, one overall deadline, bounded output, and Unix RLIMIT_FSIZE apply.
Signed URLs are redacted from diagnostics and excluded from saved metadata.
Missing helper, blocked source, unavailable captions, and malformed cues are
separate API errors. Metadata alone cannot succeed as a transcript.

### Setup and working commands

Official references checked: [installation](https://github.com/yt-dlp/yt-dlp#installation)
and [EJS](https://github.com/yt-dlp/yt-dlp/wiki/EJS). GitHub rendered reads failed;
the official raw README/wiki sources were read instead. Helper source inspected
from the installed official package, including subtitle header propagation.

```sh
uv tool install 'yt-dlp[default]' --index-url https://pypi.org/simple
yt-dlp --version
node --version
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
# Already running; do not start a second server:
./target/debug/webtoold --config runtime/media-config.toml
```

Ignored runtime/media-config.toml copies config.example.toml and sets
`ytdlp_path = "/home/mainpc/.local/bin/yt-dlp"` and
`ytdlp_js_runtime = "node:/usr/bin/node"`. PID/log remain runtime/webtoold.pid
and runtime/webtoold.log; address http://127.0.0.1:8420. Only this project's old
server was stopped, after checking its executable path.

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
webtool doctor
# Library captions already exists from this proof.
webtool --format json read 'https://youtu.be/jNQXAC9IVRw' --language en --library captions --refresh >runtime/youtube-read.json
DOC_ID=$(python3 -c 'import json;print(json.load(open("runtime/youtube-read.json"))["id"])')
webtool read "$DOC_ID"
webtool find "$DOC_ID" 'elephants'
webtool export "$DOC_ID" --kind original --output runtime/youtube-original.vtt --force
HASH=$(python3 -c 'import json;print(json.load(open("runtime/youtube-read.json"))["source"]["original"]["sha256"])')
cmp "data/objects/$HASH" runtime/youtube-original.vtt
webtool library items captions
```

Saved ID: `1a28a8f63209abdb9912532346d649a1e87c509c68a3a905454e54cce99fc9ef`.
The historical milestones below retain their original outcomes and former setup;
current server configuration is recorded above.

## Milestone 3: usable PDF reading

Verified September 10, 2026 UTC. Checkout `/home/mainpc/Projects/webtool`, branch
`feat/pdf-reading`; issue #5, PR #6 (not merged). PR #4 was squash-merged with
`--match-head-commit 7558b25529e9130697a72d144624784d9579f715` after checking its
head and successful build. Local main fast-forwarded to `de4ad69`, and issue #3
closed. Existing runtime data was preserved.

### Build and delivered behavior

Both commands passed with existing unused-import warnings only:

```sh
cargo build --locked -p webtool-cli -p webtool-server --features webtool-server/documents
cargo build --locked -p webtool-cli -p webtool-server
```

The pinned Xberg 1.1.1 adapter compiled without dependency fixes. No dependency
versions or Cargo.lock changed; the lockfile already selected xberg-native-pdf
1.1.4. Documents are now a default server feature. The CLI remains independent
of the engine. The single CI command is unchanged and now includes documents.

Document parser revision: `xberg/1.1.1+source-blocks/2`.
Page text uses existing Paragraph blocks, with no trimming or Markdown rewrite.
Defaults request plain output and disable upstream quality rewriting. Missing
page numbers remain derived; reported page numbers are retained without boxes.
Supplemental table blocks remain extractable and are explicitly labeled in plain
and Markdown output, rather than silently repeating page text. Cell header roles
and merged spans are not inferred from Xberg's simple matrix. Metadata retains
full upstream documents and output-envelope errors/summary/additional results.

Empty/whitespace-only page objects are skipped with visible diagnostics; they do
not count as successful extraction. No text and no nonempty table cells is an
error. OCR absence is explicit, without inferring that every empty page is a
scan. Partial page-count and upstream warnings are retained. A failed extraction
still leaves retained original bytes under the existing artifact behavior, but
has no successful saved-document handle or saved normalized upstream metadata.
These empty/scan paths were inspected in source, not exercised with more PDFs.

### Actual bounded proof

One public PDF only:
https://sample-files.com/downloads/documents/pdf/fillable-form.pdf

| Check | Result |
|---|---|
| Fresh URL read | HTTP 200, application/pdf, 54,059 bytes |
| Pages | Two Paragraph blocks, reported pages 1 and 2, bbox null |
| Ordinary output | Readable form labels; no whole-page code fences |
| Native text accuracy check | Existing pdftotext independently confirmed page titles/positions; its reference output was inspected |
| Find | “Personal Information” found in b2, page 2 |
| Local ingest of URL-retained original | Same two blocks and exact upstream page strings |
| Original local export | cmp byte-identical to retained input |
| Artifact hash | Both documents match SHA-256 8413d3f961113a06b9ebedd0a3e9963517379cd62d3600f5067d497e3dcfa269 |
| Upstream metadata | Title, author, dates, page dimensions/count, native method, ocr_used=false retained |
| Tables extract | Empty array, matching upstream (this PDF supplied no structured tables) |
| Existing HTML | Previous Rust Book saved document read once successfully |
| doctor | documents enabled; OCR unavailable/not compiled |

Both PDF outputs retained `document_structure_partial` warnings. No hidden
upstream errors appeared. The old HTML kept its existing derived-location warning.
`pdftotext` was already installed and used only for this reference check; it is
not an application dependency or replacement extraction engine.
No cargo test, broad lint, benchmarks, OCR, model downloads, extra feature matrix,
or new test framework was used. No smoke steps needed a retry.

No concrete blocker remains. This is bounded native-text PDF support, not an
accuracy guarantee for arbitrary PDFs. Real table detection, scanned/empty PDF
runtime behavior, complex layouts, form values, encrypted inputs, Office, and
other formats remain unverified. No Office support claim follows from compilation.

### Local server and exact reproduction

The updated server is left running at http://127.0.0.1:8420 with the existing data/.
Only this project's old PID was terminated after verifying its executable path.
PID/log: runtime/webtoold.pid and runtime/webtoold.log. No data was deleted.

Startup (do not start a second copy while it is running):

```sh
cd /home/mainpc/Projects/webtool
./target/debug/webtoold --config config.example.toml
```

The commands below reuse ignored runtime/ and explicitly replace named temporary
exports. The URL read uses refresh; local ingest always parses.

```sh
cd /home/mainpc/Projects/webtool
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
mkdir -p runtime
webtool doctor
webtool --format json read https://sample-files.com/downloads/documents/pdf/fillable-form.pdf --refresh >runtime/pdf-url.json
URL_ID=$(python3 -c 'import json;print(json.load(open("runtime/pdf-url.json"))["id"])')
webtool read "$URL_ID"
webtool find "$URL_ID" 'Personal Information'
webtool export "$URL_ID" --kind original --output runtime/fillable-form.pdf --force
webtool --format json ingest runtime/fillable-form.pdf >runtime/pdf-local.json
LOCAL_ID=$(python3 -c 'import json;print(json.load(open("runtime/pdf-local.json"))["id"])')
webtool read "$LOCAL_ID"
python3 -m json.tool runtime/pdf-local.json
webtool export "$LOCAL_ID" --kind original --output runtime/fillable-form-export.pdf --force
cmp runtime/fillable-form.pdf runtime/fillable-form-export.pdf
webtool --format json extract "$LOCAL_ID" tables
webtool read 04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded
```

Saved URL ID: `bdca71b0310fdf092265cb50d0f2c2cedb6cfe596ad943547a80864c38cc4996`.
Saved local ID: `64ef0a3524cc691e76a2d08d6c8028e44eaac1aa08ca128e753235f48d88fe7b`.

The sections below are historical milestones; their former feature/merge state
is superseded by this section.

## Milestone 2: selected HTML source fidelity

Verified September 10, 2026 UTC. Checkout `/home/mainpc/Projects/webtool`, branch
`fix/html-source-fidelity`; issue #3 and PR #4. Bootstrap PR #2 was squash-merged
with `--match-head-commit a2db698e9761e9c1b0e1bb8869d858bf8d9a6cca` after checking
its unchanged head and successful build. Main advanced to
`24225fe` and issue #1 closed. No runtime data was removed. PR #4 is not merged.

### Changed behavior

- HTML parser revision is `rs-trafilatura/0.2.2+source-blocks/2` (explicit CSS
  uses `explicit-css+source-blocks/2`). Old stored document schemas are unchanged.
- Walk selected list/quote containers in order, emitting prose runs around
  nested blocks instead of flattening them or repeating descendant text.
- Only unique original-element matches restore preformatted text and language
  classes (`language-` or `lang-` on pre/code), or table cell boundaries, header
  flags, rowspan, and colspan. Table row/cell traversal excludes nested rows.
- Never replace an entire selected container with its original subtree. This
  avoids restoring descendants removed by content selection. Ambiguous matches
  and partial container prose retain derived locators and visible warnings.
- Existing plain rendering already preserves code whitespace; it needed no
  change. Plain tables remain tab-separated values, not visual merged-cell
  layouts. JSON retains spans/header flags; Markdown's existing HTML table
  representation supports spans. No document schema or storage changes.
- Search pin, extractor dependency, Cargo.lock, and CI are unchanged.

### Actual checks

| Check | Outcome |
|---|---|
| Normal locked debug CLI/server build | Passed; existing warnings only |
| Authored default-extraction HTML ingest, no selector | Passed: 10 blocks in order |
| Nested code inside li and blockquote | Exact tabs/spaces/newlines; Rust and Python language labels restored |
| Merged table | Rows have 2, 2, 3 cells; Mode rowspan=2, Limits colspan=2; header flags retained |
| Selection and duplication | No navigation in blocks/plain output; each surrounding phrase and code sample appears once |
| Locators | Both code blocks and table have verified HTML locators; four split prose runs honestly derived |
| Plain CLI output | Inspected code indentation and table values; no alternate-screen UI |
| Original fixture export | All 1085 bytes identical by cmp |
| Fresh public Rust Book read | 16 code blocks, two tables, all with verified HTML locators |
| Retained public original comparison | All 16 pre texts match exactly in order; both complete cell matrices/flags/spans match; SHA-256 matches artifact |
| Existing bootstrap saved document | Still readable after server restart |

The public URL was https://doc.rust-lang.org/book/ch03-02-data-types.html, read
with `--refresh` and no selector. The original was 44,687 bytes. Tables were 7×3
and 6×2. Ordinary output was inspected and contains every exact code string.
Ten other blocks have derived source locators; this warning remains visible.
No tests, broad lint, benchmarks, optional builds, or additional public pages ran.
A temporary Python HTMLParser comparison inspected the retained pre/table data;
it is not a new test framework or application dependency.

No blocker remains for this milestone. This is not full HTML fidelity: inline
superscripts in the public chapter still flatten into ordinary text; table
captions, nested table representation, and complete list hierarchy remain outside
this bounded fix. A selected code/table without a unique original match keeps the
cleaned content and a derived locator; no original match is invented.

### Exact local reproduction

The updated server remains running at http://127.0.0.1:8420 using the existing
`data/`. Logs and PID remain in `runtime/webtoold.log` and `runtime/webtoold.pid`.
Startup after stopping the existing server, if needed:

```sh
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
./target/debug/webtoold --config config.example.toml
```

In another terminal, create the single temporary example (not a fixture corpus):

```sh
cd /home/mainpc/Projects/webtool
mkdir -p runtime
cat >runtime/html-fidelity.html <<'HTML'
<!doctype html>
<html lang="en"><head><title>Technical extraction smoke</title></head><body>
<nav><p>EXCLUDED NAVIGATION</p><pre><code>navigation_only();</code></pre></nav>
<main><article><h1>Technical extraction smoke</h1>
<p>This guide documents exact code and table structure for a small shared research reader. The examples preserve indentation, newlines, and source values without executing code.</p>
<ul><li>Before list code.<pre><code class="language-rust">  fn main() {
	println!("Exact nested code");
  }
</code></pre>After list code.</li></ul>
<blockquote>Before quote code.<pre class="language-python"><code>  if ready:
    print("Exact quote code")
</code></pre>After quote code.</blockquote>
<table><thead><tr><th rowspan="2">Mode</th><th colspan="2">Limits</th></tr><tr><th>Min</th><th>Max</th></tr></thead><tbody><tr><th>safe</th><td>0</td><td>17</td></tr></tbody></table>
<p>The table describes safe mode with a minimum of zero and maximum of seventeen. Header cells and merged cells are part of the source, not generated summaries.</p>
</article></main></body></html>
HTML
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
# Ingest always parses; it is not a cached URL read.
webtool --format json ingest runtime/html-fidelity.html >runtime/html-fidelity.json
DOC_ID=$(python3 -c 'import json;print(json.load(open("runtime/html-fidelity.json"))["id"])')
python3 -m json.tool runtime/html-fidelity.json
webtool read "$DOC_ID"
webtool export "$DOC_ID" --kind original --output runtime/html-fidelity-original.html --force
cmp runtime/html-fidelity.html runtime/html-fidelity-original.html
webtool --format json read https://doc.rust-lang.org/book/ch03-02-data-types.html --refresh >runtime/rust-data-types.json
PUBLIC_ID=$(python3 -c 'import json;print(json.load(open("runtime/rust-data-types.json"))["id"])')
webtool read "$PUBLIC_ID"
webtool export "$PUBLIC_ID" --kind original --output runtime/rust-data-types-original.html --force
# Inspect retained source pre/table elements alongside parsed JSON:
webtool --format json extract "$PUBLIC_ID" css --expression 'pre,table'
webtool --format json extract "$PUBLIC_ID" code
webtool --format json extract "$PUBLIC_ID" tables
```

`--force` above only replaces the named temporary exports. All example files and
local result JSON stay under ignored runtime/. Fixture document from this run:
`de34d85d7ef36f8965d19375046332382a6fb5651489490d1c5ec69c0f1b53aa`.
Public document:
`04f9b1ba3433f1c3203cacc1bbb7d51ff0213686dfb1112cfe6e416c85eadded`.
New extraction revision participates in the saved document ID; existing IDs
continue to read the stored result. URL reads need `--refresh` to avoid old cache.

## Historical runnable bootstrap results

The following records milestone 1, before its authorized merge. Current branch,
server revision, and workflow are recorded above.


- Rust 1.98.0 / Cargo 1.98.0 on Fedora 44; debug build.
- `cargo build --locked -p webtool-cli -p webtool-server`: passed with
  existing unused-import and optional-document dead-code warnings.
- A real Cargo.lock was generated and committed. Default HTML and search stay enabled.
- `webtoold` started; `webtool doctor` reached API v1 at http://127.0.0.1:8420.
- Created shared library `bootstrap` and ingested tests/fixtures/source.md.
- Read saved document and found `Exact code` in block b3, source lines 5–7.
- Original export: 87 bytes; `cmp` confirmed byte-for-byte equality.
- Default `read https://example.com`: passed, returned “Example Domain”,
  the domain-purpose paragraph, and “Learn more”, without a selector or browser.
- Live `search 'Rust programming language' --limit 5`: exit 0, five links,
  no warnings. Brave and DuckDuckGo both contributed. Inspected returned URLs:
  https://en.wikipedia.org/wiki/Rust_(programming_language),
  https://rust-lang.org/learn/,
  https://www.geeksforgeeks.org/rust/introduction-to-rust-programming-language/,
  https://rust-lang.org/, https://rust-lang.org/en-US/.
  Snippets are provider output; destination pages were not read by this search.
- Full tests, optional feature builds, lint, benchmarks, coverage, Docker,
  DOI, repository readers, and captions were not run in this milestone.

### Build corrections

metadata-search-engine-rs 0.3.2 has a dependency cycle through search-tui.
Pinning 0.3.1 resolves it without importing a TUI or disabling search.
Caption timestamp replacement required a string replacement argument.
Search futures are collected before bounded stream execution to satisfy
Axum's handler-future lifetime requirements; concurrency remains bounded at four.

### Local operation

Checkout: `/home/mainpc/Projects/webtool`, branch `feat/bootstrap-running-cli`.
The server is left running on loopback only. Data lives in ignored `data/`;
logs, PID, and smoke outputs live in ignored `runtime/`.
One server process per data directory. Do not start a second while it is running.

```sh
cd /home/mainpc/Projects/webtool
cargo build --locked -p webtool-cli -p webtool-server
mkdir -p runtime
nohup ./target/debug/webtoold --config config.example.toml >runtime/webtoold.log 2>&1 </dev/null &
echo $! >runtime/webtoold.pid
export PATH="$PWD/target/debug:$PATH"
export WEBTOOL_SERVER=http://127.0.0.1:8420
webtool doctor
webtool library create bootstrap --description 'Runnable CLI smoke'
webtool --format json ingest tests/fixtures/source.md --library bootstrap --actor JCFrags
DOC_ID=cd6afccebfcf8a897537c855c8e33c70686eb91372878e7ed33133475905f4da
webtool read "$DOC_ID"
webtool find "$DOC_ID" 'Exact code'
webtool export "$DOC_ID" --kind original --output runtime/source-original.md
cmp tests/fixtures/source.md runtime/source-original.md
webtool read https://example.com
webtool search 'Rust programming language' --limit 5
```

Library and export already exist after this run. Use a new library/output name
for a later run, or explicit `--force` only when replacing the export is intended.
To stop, verify the PID in runtime/webtoold.pid belongs to this webtoold and send
SIGTERM. Rollback means stopping this process and using a prior feature commit;
do not delete data or reset main. At bootstrap handoff no merge had been authorized; PR #2 has since been merged as recorded above.

### Remaining blockers and next task

No blocker remains for the default runnable milestone. Optional Xberg documents,
OCR models/native runtimes, fastCRW, Lightpanda, Chromium, and yt-dlp are not
validated or configured; doctor reports them unavailable. They are not implied
working by a successful default build. No provider cycling was needed.

Bootstrap next task (now completed by milestone 2 above): validate default HTML source fidelity on a representative
public documentation page containing code and tables, against retained originals.

## Historical archive checks

The supplied Python SQLite/package logs report 33 passing checks. The supplied
Chromium probes timed out. These historical checks are not Rust test results.
The existing 70 Rust tests were kept but not run or expanded during bootstrap.
`MANIFEST.sha256` describes archive bytes only; it is not a current-tree checksum.
Downloads README(20260910-045733).md and STATUS.md matched the archive exactly.
The older shared-terminal proposal specifies Python; the current direction and
Rust implementation supersede that stack choice.

## Implementation state

### Core source is present

The repository contains the application, not just interfaces or pseudocode.
It includes a CLI, HTTP server, storage, native readers, search orchestration, job execution, and tests.
The default build and small smoke sequence above now run; this does not validate every advertised capability.

### Optional integrations need validation

The Xberg implementation calls the documented Rust API and retains the serialized upstream result.
Page, slide, and sheet references are assigned only from supplied fields.
Tables are exposed as supplemental blocks without invented header roles or spans.

The yt-dlp implementation invokes the configured helper and retrieves an available VTT track.
It does not download video or generate a replacement transcript.

The browser implementation uses bounded subprocesses with output limits and timeouts.
DOM captures retain warnings about unknown status, redirects, and incomplete readiness.
An experimental fastCRW feature provides an additional explicit renderer.

These are source implementations, not working integrations demonstrated in this environment.

## Differences from the architecture proposal

| Proposal | Delivered source |
|---|---|
| Integrate selected fastCRW crawler components | Application-owned bounded crawler, plus an experimental fastCRW renderer adapter |
| Select browser engines automatically by capability | Explicit HTTP, Lightpanda, Chromium, or experimental fastCRW selection |
| Reuse warm browsers | Fresh helper process per browser read |
| Full structured document and figure handling | Page text and supplemental tables, with full upstream metadata retained |
| Broad scholarly and repository reading | DOI export, generic URL reading, and revision-pinned GitHub README retrieval |
| Broad format-specific search | General web metasearch and local keyword search |
| Polished terminal layouts for every result | Plain reading and numbered search, with indented JSON for several structural commands |
| Fully resumable operations | Persisted crawl status and results, not a persisted frontier |

These differences are unfinished work, not evidence that the proposal was fully implemented.

## Deferred validation

Beyond bootstrap, check representative source fidelity before adding integrations.
Full tests and optional integration checks remain manual. Documents, browser
readiness, crawl recovery, captions, and resource measurements need separate work.
Do not expand this milestone into those checks.

## Operational limits

The server assumes one instance per data directory and a trusted network.
All data is shared, with no authentication or per-user quotas.
Request sizes and worker concurrency are bounded, but sustained-load capacity has not been tested.
A disconnected foreground client has no universal reconnectable job record.
Only crawls currently use persisted job records.

The cache is time-based and does not perform conditional HTTP revalidation.
Failed parses can leave an unreferenced original artifact on disk.
Artifact garbage collection and full-library backup export are not implemented.

Use new-format fixtures before extending advertised support.
Keep this document accurate after each actual build and test run.
