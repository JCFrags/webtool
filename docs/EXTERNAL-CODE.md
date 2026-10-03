# Optional code and documentation indexes

Sourcegraph and Context7 are explicit optional adapters. Both remain unconfigured
by default. They use the ordinary engine, saved documents, artifact store, and
operation/network/parsing capacity. They do not change first-party GitHub or
docs.rs operations, invoke a local LLM, or create another index or database.

Index snippets are not fetched source evidence. These adapters do not automatically
read source links, clone repositories, infer an installed version, or choose a
substitute provider. Public visibility and index metadata are not reuse licenses.
Rights remain unknown unless separately established.

## Server configuration and readiness

Add only the selected provider to the server's TOML configuration. These example
endpoints are explicit operator choices, not defaults or tested service health:

```toml
[external_code]
timeout_ms = 10000
max_bytes = 1048576

[external_code.sourcegraph]
endpoint = "https://sourcegraph.example.org"
credential_env = "WEBTOOL_SOURCEGRAPH_TOKEN"
timeout_ms = 6000
interval_ms = 1000
max_requests_per_minute = 20

[external_code.context7]
endpoint = "https://context7.com"
credential_env = "WEBTOOL_CONTEXT7_KEY"
timeout_ms = 6000
interval_ms = 1000
max_requests_per_minute = 20
```

Use a protected server environment mechanism for actual credentials. Do not put
credentials in TOML, CLI arguments, URLs, client requests, or ordinary logs.
`credential_env` is only an environment variable name. The server resolves its
value for the selected provider. Sourcegraph can omit the reference for an
instance that permits anonymous access. A configured but missing/invalid reference
fails locally. Context7 requires a reference and value. It has no keyless fallback.
The adapter accepts nonempty ASCII token values up to 8 KiB using letters, digits,
`-_.~/+=`. Other credential formats are unavailable rather than silently changed.

Endpoint validation permits HTTPS or literal loopback HTTP for diagnostics. It
rejects URL credentials, query strings, and fragments. A base path is supported.
The transport does not follow redirects, use environment proxies, send a Referer,
retry requests, rotate keys, or contact another provider. Authorization headers
use Sourcegraph's `token` scheme and Context7's `Bearer` scheme. Header values are
marked sensitive. Error bodies are not retained or returned. A success body that
contains the configured token verbatim is refused before artifact storage.

```sh
webtool external status
```

Status reports only configuration booleans, local credential readiness, shared
request counts, and cooldown seconds. It makes no provider request. It does not
expose endpoints, environment names, or credential values. `locally_ready` does not
establish account access, provider health, library availability, or index coverage.

| Setting | Default | Accepted range |
|---|---:|---:|
| Whole `timeout_ms` | 10,000 | 500 to 30,000 ms |
| `max_bytes` | 1 MiB | 1 KiB to 4 MiB |
| Provider `timeout_ms` | 6,000 | 100 to 20,000 ms |
| Provider `interval_ms` | 1,000 | 100 to 60,000 ms |
| Provider `max_requests_per_minute` | 20 | 1 to 60 |
| Operation result `--limit` | 5 | 1 to 20 |

The whole deadline includes operation admission, provider queueing, pacing, body
reading, parsing, and storage. The server's general request timeout and source-byte
limit can reduce the selected budgets. Each provider has one serialized gate shared
by engine clones and clients. The local rolling-minute ceiling is not the provider's
allowance. HTTP 401, 402, 403, or 429, or a supplied zero `ratelimit-remaining`, starts
a shared cooldown. Numeric or HTTP-date `Retry-After` and Unix `ratelimit-reset`
can extend the minimum 60 seconds, up to one day. The state is in-process. It is
not coordinated across service restarts, machines, or first-party adapters.

## Sourcegraph indexed search

Select an actual configured instance. No global Sourcegraph endpoint or public
repository coverage is assumed. The adapter uses `/.api/search/stream` with `v=V3`,
`cm=false`, and a bounded `display` and `count`.

```sh
webtool external sourcegraph search 'Buffer' \
  --repo github.com/dtolnay/itoa --ref COMMIT_SHA --path src/lib.rs --limit 5
webtool external sourcegraph search 'tests/' \
  --mode path --repo github.com/dtolnay/itoa --ref COMMIT_SHA --limit 5
```

Replace `COMMIT_SHA` with the selected 40-character commit, or use one explicit
simple branch/tag ref. `--ref` requires an exact repository name. Revision lists,
time queries, and complex revision expressions are unsupported. Literal content
and literal path-substring searches are supported. User text is quoted or escaped
inside structured provider filters, not accepted as an arbitrary search expression.
The exact repository, optional path/language, and result limit are explicit.
Forks and archived repositories are excluded. Regex and symbol modes return
`external_unsupported` before a provider request. There is no silent mode fallback.

The response retains admitted native hit objects, commit when supplied,
`repoLastFetched`, provider line coordinates, progress/skips, filters, alerts,
completion signals, the observation URL/time/status, and original stream bytes.
Missing and explicit null native fields stay distinct. Index lines use the
provider's zero-based line number. Provider offsets are not original-file byte
ranges. Every displayed block has a derived locator.

A returned immutable commit must match an explicitly requested commit. Unknown
commits are visible and cannot be verified. An admitted hit from a different exact
repository or commit fails identity checks. Unsupported/malformed result types
are omitted with warnings. Result limits, skipped repositories, alerts, malformed
or missing completion signals, and transport stops keep incompleteness visible.
A usable partial response retains complete admitted events from the bounded stream.
Byte/deadline stops also report `coverage.stopped`. A failure without usable hits
returns an error. A complete stream does not prove complete or fresh index coverage.

### Explicit retained-file verification

Search returns an index search ID and zero-based hit numbers. To accept a file,
use the first-party workflow in [CODE.md](CODE.md) to save a map at that hit's commit
and read the admitted file. Then select all three saved IDs explicitly:

```sh
webtool code map dtolnay/itoa --ref COMMIT_SHA --depth 2 --max-entries 100
webtool code file MAP_ID src/lib.rs
webtool external sourcegraph verify SEARCH_ID 0 MAP_ID FILE_ID
```

Verification makes zero network requests. It supports content-line hits for
`github.com/owner/repository` only. It requires the ordinary saved map/file parsers,
regular Git mode, same repository/path, hit/map commit, map/file blob identity,
and retained size. It compares each full index line with exact retained UTF-8 file
bytes. LF defines lines. A CR in a CRLF terminator is excluded from the comparison.
The response supplies one-based file lines and half-open original byte ranges.
A mismatch fails instead of turning a stale snippet into evidence.

The selected map supplies the commit/blob context when an identical saved file
retains its first accepted retrieval metadata. This comparison does not independently
recompute a Git blob SHA-1, establish file rights, read a missing file, support
non-GitHub hosts or binary files, or validate the whole provider index. It does not
use an undocumented raw-file or unstable GraphQL fallback.

## Context7 library and context selection

Both v2 requests send `fast=true`. Current documentation says this skips LLM
reranking. The service does not invoke reranking or apply provider rules. This is
not a guarantee that the upstream index was prepared without an LLM. Queries leave
the service and can be retained by Context7. Do not send private code or secrets
without separate permission for that transfer.

```sh
webtool external context7 libraries 'example-library' 'example API usage' --limit 5
webtool external context7 context DISCOVERY_ID /owner/library 'example API usage' \
  --version v1.2.3 --limit 5
webtool external context7 context DISCOVERY_ID /owner/library 'tracked API usage' \
  --tracked --limit 5
```

Use an ID admitted by saved discovery from the same configured endpoint, not a
library guessed from its title. The selected library must report `state=finalized`.
Discovery preserves native versions, branch, index update date, state, unknown
fields, and `searchFilterApplied`. Coverage is always incomplete because one
query-selected library response does not establish index totals or exclusions.

Choose either one literal listed version or the explicit tracked index. Missing
selection, `latest`/`main`/similar aliases, unlisted versions, unadmitted libraries,
and pending libraries fail without another request or substitution. A listed label
is sent as `/owner/library/version`. It is not a source commit or installed-version
assertion. Tracked selection sends the base library ID and makes no release claim.

The JSON context response retains admitted native `codeSnippets`/`infoSnippets`,
source URLs, `isDynamic`, `sourceFile`, and unknown/absent/null metadata. Code and
information share one local result cap, with code first. Source URLs are exposed
only as unfetched links. The original response retains untrusted provider rules.
Those rules are not applied, copied into instructions, or used to select sources.
An optional returned `libraryId` must match the requested ID.

The documented context schema does not require a resolved library/version,
commit, build target, or feature attestation. Returned snippets may use mutable
URLs or a dynamic source-code index. They are derived, incomplete third-party
context, not complete or revision-exact publisher documentation. Read selected
first-party sources separately before accepting source evidence.

## HTTP and errors

The five routes share generated OpenAPI types from
`crates/protocol/src/external_code.rs` and the safe `Problem` boundary:

| Route | Purpose |
|---|---|
| `GET /v1/external/providers` | Local readiness and shared budget state |
| `POST /v1/external/sourcegraph/search` | Explicit bounded instance index search |
| `POST /v1/external/sourcegraph/verify` | Compare one saved hit/map/file selection |
| `POST /v1/external/context7/libraries` | Save bounded library discovery |
| `POST /v1/external/context7/context` | Read explicit listed/tracked index context |

Fixed error codes distinguish unconfigured, invalid, unsupported, unavailable,
access denied, rate limited, redirect refused, identity mismatch, byte limit,
deadline, upstream failure, and credential-response refusal. Unknown storage or
programming failures remain safe internal errors. Inspect `coverage`, stop reasons,
and warnings even when an operation returns HTTP 200.

## Bounded verification and contract evidence

On October 3, 2026 UTC, the normal locked CLI/server build and three focused engine
checks passed. Two existing safe-error checks also passed. One loopback synthetic
HTTP fixture exercised actual private CLI/server copies and all five routes and
OpenAPI references. Its 24 provider requests covered readiness without probes,
structured search, native coverage and unknowns, selection/identity errors, partial
SSE byte/deadline retention, privacy and redirect refusal, shared denial/rate
cooldowns, concurrent-client pacing, and the rolling-minute ceiling. Five diagnostic
services exited zero and their listeners stopped.

The file/map verification used clearly synthetic documents seeded offline into
isolated diagnostic data. It established selected-map identity and exact retained
byte comparison, not live first-party retrieval. No real key, account, public
provider request, authenticated live integration, purchase, install, or deployment
was exercised. Provider availability, full schema variation, Unicode search
interpretation, and real index/version/source fidelity remain unverified.

Primary provider documentation was read on the same UTC date:

- [Sourcegraph streaming search API](https://sourcegraph.com/docs/api/stream-api)
  defines V3 events and the stream route. Its legacy anonymous example does not
  establish current global endpoint health or coverage.
- [Sourcegraph query language](https://sourcegraph.com/docs/code-search/queries/language)
  describes quoted patterns, repository filters, and explicit refs.
- [Sourcegraph API policy](https://sourcegraph.com/docs/api) describes the new
  versioned API from 7.0 and warns that the GraphQL debug API lacks compatibility
  guarantees. This adapter does not depend on GraphQL.
- [Context7 API guide](https://context7.com/docs/api-guide) says an API key is
  required. Some rate/OpenAPI text also describes keyless access. This adapter
  deliberately requires a configured server key and never attempts keyless access.
- [Context7 library search](https://context7.com/docs/api-reference/search/search-for-libraries)
  and [context API](https://context7.com/docs/api-reference/context/get-documentation-context)
  define the v2 fields and `fast=true` behavior. Version selection is a request,
  not a cryptographic publisher pin.
- [Context7 data privacy](https://context7.com/docs/security/data-privacy)
  describes query retention and external reranking providers. The upstream privacy
  and preparation process is not validated by the synthetic fixture.
