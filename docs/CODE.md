# Public code and release documentation

These operations use the same engine, saved documents, and artifact store as
ordinary reads. They do not use an LLM, clone a repository, execute source code,
read GitHub credentials, or access private repositories.

## Repository workflow

1. Discover a small public repository result set.
2. Select a repository and an explicit branch, tag, or commit.
3. Save a bounded repository map. Use the returned map ID for later operations.
4. Search admitted paths, or explicitly select admitted files for literal search.
5. Read a matching file and a nearby test or example at the map's exact commit.

```sh
webtool code discover 'repo:dtolnay/itoa' --limit 1
webtool code map dtolnay/itoa --ref master --depth 2 --max-entries 100
webtool code search MAP_ID 'tests/' --mode paths --limit 5
webtool code search MAP_ID Buffer --mode literal --path src/lib.rs --limit 5
webtool code file MAP_ID tests/test.rs
```

Use the actual branch from discovery or an explicit commit. `master` above is an
example, not an automatic branch fallback. A map request must supply `--ref`.
The repository parameter is `owner/repository`, not a URL. A ref is separate from
the path, so a ref with slashes has no URL ref/path ambiguity.

The existing ordinary GitHub URL reader remains separate. Its bounded ambiguity,
symlink, submodule, and original-byte rules are unchanged.

### Discovery and map coverage

Discovery sends one GitHub repository-search request. It preserves the exact
query, observation time, provider response, reported total, and incomplete state.
It returns at most 20 repositories, with a default of five. There is no automatic
pagination. A provider description is not a fetched code match. Discovery license
metadata is not a revision-specific file license.

Map uses nonrecursive Git-tree requests and bounded breadth-first traversal.
It resolves the explicit ref once, then uses commit/tree/blob identities rather
than requesting a mutable branch for each file. The output distinguishes
`requested_ref`, `resolved_commit`, `root_tree`, and each entry's `object_sha`.
It does not equate a file blob SHA with a repository commit SHA.

| Map option | Default | Accepted range |
|---|---:|---:|
| `--depth` | 2 | 1 to 4 |
| `--max-entries` | 200 | 1 to 1,000 |
| `--max-requests` | 8 | 2 to 20 |
| `--max-bytes` | 2 MiB | 1 KiB to 8 MiB |

Root entries are depth one. Directories at the depth limit are listed but not
visited. All entry types count toward admission. Symlinks and submodules are
visible but never followed. Optional `--path` starts at one directory. Traversal
to that directory consumes the same request and byte budget. Paths have at most
16 components. Dot segments, empty components, control characters, and encoded
path separators are rejected.

`coverage` reports consumed requests and decoded response bytes, incompleteness,
stop reason, and warnings. Each individual response also fits the server's source
byte limit. A partial map retains admitted entries and provider originals.
A failure before the requested directory is established returns an error, not an
empty successful map. A complete bounded directory is not a complete repository.

The saved map's original artifact has role `derived_repository_map`. It is a
manifest, not an original GitHub response or repository file. Its observations
reference separately retained exact GitHub API response artifacts. The manifest
source status is null because it combines several responses. Each observation
keeps its actual HTTP status.

### Explicit search and file reads

`--mode paths` searches admitted path names only. It makes no provider request.
It is a literal substring search, not a glob or symbol index.

`--mode literal` requires one or more exact `--path` values. It fetches only those
admitted regular files. It does not silently scan every mapped file. Matching is
case-sensitive and literal. Every returned match is checked against the saved
UTF-8 original, not the provider description or a normalized parser view.

Matches include the saved document ID, path, blob SHA, pinned URL, one-based line,
and half-open byte range in the complete original file. Offsets are UTF-8 byte
offsets, not character offsets or excerpt offsets. LF defines the line boundary.
Excerpts remain exact file substrings. Terminal output escapes control sequences
without changing originals, JSON, or offsets.

| File/search option | Default | Accepted range |
|---|---:|---:|
| `--max-files` | 2 | 1 to 20 |
| `--max-file-bytes` | 256 KiB | 1 byte to 1 MiB |
| `--max-bytes` | 1 MiB | 1 byte to 4 MiB |
| `--max-requests` | 2 | 1 to 20 |
| Search `--limit` | 5 | 1 to 50 |

File byte budgets count decoded file contents. API envelope bytes have a separate
bound of twice the selected total file budget plus 8 KiB per allowed file.
The server's source byte limit still applies. A file read stores its exact bytes
before parsing. Binary/non-UTF-8 files are not searchable. Source files are plain
code here, including Markdown, HTML, JSON, and notebooks. They are not rendered,
executed, or converted to an alternate document format.

`code file` only accepts a regular file already admitted by that saved map.
It verifies the API blob identity and decoded size against the saved entry. It
preserves the original bytes and maps the complete code block to source lines.
It does not resolve the branch again. Saved document IDs remain stable for the
same accepted file snapshot. Such a document retains its first accepted retrieval
metadata. Use the selected map's context and the search response for the current
requested ref, rather than treating a saved document's first ref or observation
headers as a fresh provider check. File notices remain verbatim in originals.
The service does not interpret those notices as permission to reuse the file.

Literal search returns one outcome for every selected path when a provider or
budget failure occurs. It retains successful files and matches. A stop prevents
later file requests. Callers must inspect `coverage`, each file's `error`, and
warnings even when the HTTP status is 200. An unadmitted or unsupported selection
fails validation before file requests rather than pretending the file was searched.

`--mode github-code` returns `code_search_unavailable`. GitHub's authenticated code
search is not configured or called. There is no hidden fallback to literal file
search, no token environment lookup, and no inherited `gh` access.

### Deterministic declaration and context navigation

```sh
webtool code search MAP_ID '**/*.rs' --mode path-glob --limit 10
webtool code search MAP_ID 'Buffer|Integer' --mode regex --path src/lib.rs --limit 5
webtool code search MAP_ID Buffer --mode symbols --path src/lib.rs --limit 5
webtool code context MAP_ID FILE_ID 58 --before 3 --after 8
```

Path globs search only admitted names. `*` and `?` do not cross a slash. `**`
crosses directories, and `**/` also permits zero directories. Character classes
and backslash escapes are unsupported. Regex uses bounded Rust regex syntax,
case-sensitive matching, and no zero-width results. Both file modes require exact
selected paths and the existing file/request/byte budgets. Invalid patterns fail
before provider requests.

`symbols` searches declaration names by a literal substring. It uses deterministic
lexical comment/string filtering and line-start declaration patterns for Rust,
Python, JavaScript/TypeScript, and Go. This is not compiler symbol resolution,
reference analysis, embeddings, or a complete declaration index. Macros, Unicode
identifiers, complex declarations, JavaScript regex literals, and language-specific
syntax can exceed this heuristic. Unsupported file extensions fail locally. Match
name ranges, lines, and excerpts still address exact retained UTF-8 bytes.

`context` requires the saved map and saved file IDs. It verifies repository, path,
blob identity, regular Git mode, and retained size against that map. It makes no
provider request and does not resolve the ref again. The returned text is the
exact original substring, with one-based start/end lines and a half-open byte
range. Before/after accept zero through 20 lines each. A context is at most 64 KiB.
It uses the selected map's revision even when the identical file snapshot retains
an earlier requested-ref observation. No adjacent file is fetched automatically.

### Public issues, pull requests, and releases

```sh
webtool code github list JCFrags/webtool --kind issue --state all --page 1 --limit 5
webtool code github list JCFrags/webtool --kind pull-request --state closed --limit 5
webtool code github issue JCFrags/webtool 23 --comments --comment-page 1 --comment-limit 5
webtool code github pr JCFrags/webtool 24
webtool code github release JCFrags/webtool v0.1.0-alpha.1
```

These operations use unauthenticated public REST requests only. They do not inherit
`gh` access, tokens, or accounts. Lists fetch exactly one explicit page of 1 through
20 native items, with page numbers 1 through 1,000. `next_page` is metadata for a
later explicit selection, not automatic pagination. List originals retain native
JSON, including absent/null values. Displayed titles and metadata are discovery,
not accepted object bodies. GitHub's issue list includes PR records. Their actual
kind remains explicit. No filtering/refill request is hidden.

Read selects one issue/PR number or one exact release tag. Its heading and verbatim
Markdown body have JSON-pointer locators in the exact retained API original.
Missing body and explicit null body have different warnings. State, user/order,
labels, timestamps, PR base/head refs and SHAs, release target/asset metadata, and
unknown native fields remain available in saved metadata and original JSON.
Public visibility does not establish a reuse license. Release `target_commitish`
is not resolved to a commit, and release assets are not downloaded.

An optional single issue-conversation comment page is a separate saved document,
with its own original, locators, and next-page metadata. PR reviews, inline review
comments, timelines, checks, and changed-file lists are not included. Parent and
comment reads are separate mutable observations, not a transaction-consistent
historical conversation. A comment transport/budget failure retains the accepted
body and exposes `coverage.stopped`. No retry, extra page, or HTML fallback is used.
An object plus comments consumes at most two requests and 4 MiB. A discovery page
consumes one request and 2 MiB. The general source-byte limit and deadline can
reduce these bounds. Issue/PR/release snapshots are saved observations, not
immutable Git objects or complete-repository evidence.

### Pinned revision comparison

```sh
webtool code compare BASE_MAP_ID HEAD_MAP_ID
webtool code compare BASE_MAP_ID HEAD_MAP_ID --provider --page 1 --limit 5
```

The maps must describe the same public repository and starting directory. Offline
comparison makes no provider request. It reports exact object/mode changes among
admitted paths. Missing entries in an incomplete map are `not_admitted_in_base` or
`not_admitted_in_head`, not confirmed additions/removals. It does not infer renames
or compare file bodies. The saved original is a derived comparison manifest that
references both saved maps, their requested refs, exact commits, and coverage.

`--provider` opts in to one GitHub comparison request using those two immutable
commit SHAs. It does not resolve mutable refs again. The caller selects one commit
page and result count with the same 1 through 1,000 and 1 through 20 bounds. At
most 4 MiB of response JSON is admitted. Its separately saved document retains
native status, merge-base, commit metadata, and file patches with JSON-pointer
locators. Patches are provider excerpts, not independently verified complete code
files. GitHub's comparison uses merge-base semantics, which can differ from a
direct base-to-head tree comparison. Changed files occur only on page one and
are capped upstream at 300. Missing patches and later/remaining pages keep
coverage incomplete. A provider request failure keeps the accepted offline map
comparison and exposes the stop reason. No asset or source link is followed.

### Provider behavior

The GitHub adapter uses REST API version `2022-11-28` and a descriptive service
User-Agent. The transport has no authorization header and does not follow GitHub
redirects. Repository moves need an explicit updated repository selection.

Clients share a server-owned serialized provider gate. Repository search spaces
the next GitHub request by six seconds. Other GitHub requests have at least one
second between starts. docs.rs has a separate one-second gate. A rate-limit or
access-denial response stops the operation without retry or alternate provider.
The client also disables reqwest's default safe protocol-error retries. Coverage
therefore does not hide transport retries within one admitted request.
The gate blocks later requests for at least 60 seconds, or a longer supplied
numeric `Retry-After`/GitHub reset delay, capped at one day. This is conservative
local pacing, not a promise that provider limits cannot be reached. Rate state is
in-process and shared by these operations, not all machines or the separate
ordinary GitHub URL reader. API responses keep supplied rate headers.

The operation deadline is the server's configured request timeout and includes
queueing and pacing. Code/docs operations use the engine's ordinary operation,
network, and parsing capacity. There is no separate database, job queue, or index.

## Exact docs.rs releases

```sh
webtool docs hex 0.4.3 hex/fn.encode.html
webtool docs hex 0.4.3 src/lib.rs --source
```

The first path is after `https://docs.rs/hex/0.4.3/`. The source path is after
`https://docs.rs/crate/hex/0.4.3/source/`. Crate, exact release, path, and page/source
choice are explicit. `latest`, `newest`, ranges, and wildcards are rejected.
The operation does not infer an installed dependency version or substitute a
current release when the requested version or documentation build is unavailable.

The reader checks docs.rs's `crate-metadata` JSON against the requested crate and
release. It permits only a same-path trailing-slash redirect at that same release.
It rejects another release, crate, host, source route, or page before another
request. At most three responses and 4 MiB of response bytes are admitted, further
limited by the server's source byte limit. There is no browser recovery.

A documentation page retains its exact HTML and selects `main` explicitly through
the existing HTML reader. Page links can include other releases or external sites.
They are source links, not promises that every link is pinned. Select an explicit
release for each later read. The service does not automatically follow those links.

A source read selects `#source-code pre code` from the retained HTML. Its decoded
code text has an HTML locator. It is not a raw crate archive file, and its display
line numbers are not original HTML line numbers. HTML entities and syntax spans
are the documented transformation. This path preserves the source text supplied
by docs.rs without confusing line-number chrome with source code.

Metadata includes requested/resolved release, URL, route, identity evidence, and
provider artifacts. Source repository commit, build target, and features remain
unknown unless separately established. Public documentation and code visibility
are not reuse licenses. Reading an example does not prove that it builds with the
user's compiler, target, or selected features.

## HTTP routes

All nine routes use the same generated OpenAPI contract and safe `Problem` error
boundary. JSON request fields match `crates/protocol/src/code.rs`.

| Route | Purpose |
|---|---|
| `POST /v1/code/discover` | One public repository discovery page |
| `POST /v1/code/map` | Save a bounded map at an explicit ref |
| `POST /v1/code/search` | Explicit admitted-path or selected-file search |
| `POST /v1/code/file` | Read one admitted file at the saved commit/blob |
| `POST /v1/docs/read` | Read one explicit docs.rs release page/source |
| `POST /v1/code/github/list` | One explicit public issue/PR/release discovery page |
| `POST /v1/code/github/read` | One object body and optional separate comment page |
| `POST /v1/code/compare` | Offline map comparison and opt-in exact-SHA provider page |
| `POST /v1/code/context` | Exact nearby lines in a saved map/file selection |

Domain errors have fixed, bounded public text. Typical codes include
`code_invalid_request`, `code_search_unavailable`, `code_source_unavailable`,
`code_access_denied`, `code_rate_limited`, `code_identity_mismatch`, `code_limit`,
`code_unsupported`, `code_upstream_error`, and `code_timeout`. Unknown storage or
programming failures remain safe internal errors. This new typed boundary does
not migrate all existing engine errors.

## Remaining work

Explicit optional Context7 and Sourcegraph adapters are described in
[EXTERNAL-CODE.md](EXTERNAL-CODE.md). Both default to unconfigured. Their indexed
snippets do not replace this first-party workflow or establish exact publisher
source. The adapters have synthetic proof, not live authenticated acceptance.
Compiler-resolved symbols/references, complete-repository ingestion, authenticated
code search, PR review/timeline APIs, asset downloads, and exact crate archive
source are not implemented. Deterministic declaration search has the lexical
limits above. Public issue/PR/release snapshots and explicit provider patches do
not establish complete history, immutable conversation state, or file rights.
No universal index coverage, extraction accuracy, redistribution permission, or
performance advantage is claimed.

## Build and verification

Use the normal locked CLI/server build and focused checks. If several worktrees
share one Cargo target, serialize Cargo and copy each candidate into private
storage before releasing the lock. Before Cargo starts, update only the current
worktree's existing protocol/engine/server/CLI crate-root and `build.rs` mtimes.
Shared dependency files can use relative workspace paths and otherwise reuse a
sibling worktree's protocol artifacts. This procedure changes no source content
and does not delete caches or other worktrees. Do not run a practical service from
the mutable shared target.

The bounded candidate workflow passed on October 3, 2026 UTC:

- Repository discovery selected `dtolnay/itoa`. Its requested `master` ref resolved
  to `1577ed901354d0d7448ac162328f9dbf5183124c`. The map admitted 21 entries in
  seven API requests and reported unvisited depth-limited directories.
- Path search made no provider request. Literal search returned five `Buffer`
  matches from `src/lib.rs` and exposed its match limit. One nearby
  `tests/test.rs` read was the second and final file request. Saved originals,
  SHA-256, byte ranges, line numbers, blob identities, and pinned links matched.
- `hex 0.4.3`'s `hex/fn.encode.html` page and `src/lib.rs` source route each used
  one request. Both retained exact HTML and release identity. Source text matched
  the selected original code element, including notices and entity decoding.
- Authenticated code search returned unavailable without a provider request.
  `latest` was rejected locally. An unadmitted file returned explicit 404.
  Five new routes and all OpenAPI component references were checked over HTTP.
- The normal locked CLI/server build, three new focused engine checks, and the
  existing focused API schema check passed. The isolated service stopped.

These checks used one repository and one crate release, not an index/provider
campaign or source-fidelity corpus. There was no live rate-limit injection,
credential use, missing-build campaign, or Context7/Sourcegraph call. The native
resolver's wider coverage and every repository/markup shape are not established
by this scenario. A build alone is not acceptance of those untested behaviors.
