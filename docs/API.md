# HTTP API

The initial API prefix is `/v1`.
The server is intended for a trusted group and has no authentication.
The CLI is the first client, but any HTTP client can submit the same messages.
Keep the server on loopback unless a separate network and authentication boundary
is approved. No API key, account, CORS policy, or remote exposure is added here.

## Generated contract

`GET /openapi.json` serves OpenAPI 3.1 JSON from the registered Axum handlers and
Rust protocol types. Route registration and schema collection use
[utoipa-axum 0.3](https://docs.rs/utoipa-axum/0.3.0/utoipa_axum/) with
[Utoipa 6](https://docs.rs/utoipa/6.0.0/utoipa/). The released adapter supports
Axum 0.8. The protocol's optional `openapi` feature is enabled by the server, not
required by a standalone CLI build.

The contract includes all operations below, unique operation IDs, request bodies,
path/query parameters, successful responses, and shared `Problem` error schemas.
`/health` remains an undocumented alias for `/v1/health`. Axum also handles `HEAD`
for GET routes. There is no separate API backend or generated SDK.

`Content` is a `oneOf` with a required `type` tag. `Locator` is a `oneOf` with a
required `kind` tag. Each variant describes its own fields. `Document.metadata`
and `ExtractResponse.data` remain unrestricted JSON, including arrays and null.
Generation alone does not prove extraction fidelity or universal client-generator
compatibility. Clients must inspect warnings, source status, and artifact roles.

| Method | Path | Purpose |
|---|---|---|
| GET | `/v1/health` | Version and configured or compiled capabilities |
| POST | `/v1/read` | Retrieve and save one source |
| POST | `/v1/ingest` | Upload a file as multipart data |
| POST | `/v1/search` | Search public providers or saved documents |
| GET | `/v1/documents` | List saved documents |
| GET | `/v1/documents/{id}` | Retrieve the document JSON |
| GET | `/v1/documents/{id}/original` | Download original bytes |
| POST | `/v1/documents/{id}/find` | Literal or regex matching |
| POST | `/v1/documents/{id}/extract` | Structural extraction |
| GET, POST | `/v1/documents/{id}/annotations` | Read or add notes and tags |
| GET, POST | `/v1/libraries` | List or create shared libraries |
| GET, POST | `/v1/libraries/{name}/items` | List or add document references |
| POST | `/v1/crawl` | Submit a bounded crawl job |
| GET | `/v1/jobs` | List recent jobs and active work |
| GET | `/v1/jobs/{id}` | Read persistent status and results |
| POST | `/v1/jobs/{id}/cancel` | Request cancellation |
| POST | `/v1/jobs/{id}/resume` | Explicitly resume a durable crawl |
| POST | `/v1/map` | List page links or sitemap locations |
| POST | `/v1/media` | Retrieve metadata and existing captions |
| POST | `/v1/cite` | Retrieve DOI bibliography metadata |

## Examples

Read request:

```json
{"url":"https://example.com","refresh":false,"renderer":"http","library":"research","selector":"main"}
```

Search request:

```json
{"query":"Rust async cancellation","limit":10}
```

Saved-library search:

```json
{"query":"evidence","limit":10,"library":"*"}
```

Find request:

```json
{"query":"Exact code","regex":false,"ignore_case":false,"limit":100}
```

Extract request:

```json
{"kind":"json_pointer","expression":"/results/0/value"}
```

Crawl request:

```json
{"url":"https://example.com","max_pages":20,"max_depth":2,"library":"research","actor":"Alice"}
```

Caption routing: `/v1/read` accepts `language` (default `en`) and renderer
`auto` (default), `captions`, or the existing explicit renderers. Auto routes
YouTube watch/youtu.be URLs to captions unless a CSS selector is supplied.
Explicit `http` still reads HTML. `/v1/media` uses the same stored caption path.
Language is part of the cache key; source status is null when yt-dlp supplies none.

## Contract details

`read` returns `document` and `cached` fields.
`search` returns provider-labeled results and warnings.
`find` offsets are UTF-8 byte offsets into the returned block text.
They are not Unicode character indexes or original-file offsets.

`extract` supports `tables`, `links`, `code`, `images`, `metadata`, `outline`, `json_pointer`, and `css`.
A missing JSON pointer has `found: false` rather than masquerading as a retrieved null.

`ingest` accepts exactly one `file` part with `multipart/form-data`.
Optional parts are `name`, `library`, `actor`, and `selector`.
A name is a filename, not a URL or server filesystem path. If omitted, the file
part's filename is used, or `upload.txt` if that header is also absent.
Unknown parts, a missing file, and multiple files return 400.

File bytes must fit the server's configured `max_bytes`. Each decoded text part
is limited to 8192 bytes. The total request body limit is `max_bytes + 256 KiB`,
including multipart boundaries and headers. JSON request bodies have this same
total limit. Body and upload size rejections return 413 with a `Problem` body.
The exact limit is deployment configuration, not a universal OpenAPI constant.
Send one existing `library` name to attach the saved document to that library.

`original` returns an attachment using `application/octet-stream`, not JSON or
base64. This prevents the download endpoint from displaying saved HTML as an
application page. `source.original` in the document supplies the source media
type, byte count, SHA-256, and artifact role. A `rendered_dom` artifact is a
retained DOM snapshot, not original HTTP response bytes. Its download header is
`Content-Disposition: attachment; filename=rendered-dom.html`. Other originals
use `Content-Disposition: attachment`.

`cite` accepts `doi` or its compatibility alias `document_id`, plus `format`.
Do not send both identifiers. A saved arXiv document ID uses retained metadata
without a network request. DOI input retrieves bibliography metadata. Formats are
`bibtex`, `ris`, and `csl`; saved arXiv papers support `bibtex` and `csl` only.

`map` returns a tagged `page_links`, `urlset`, or `sitemapindex` value. It does not
expand nested sitemaps. Crawl requests can opt in to bounded expansion with
`sitemaps: ["https://example.com/sitemap.xml"]`, `discover_sitemaps: true`, or both.
These fields default to an empty list and false.

Job cancellation returns the state at request time and `cancel_requested`. Poll
for final cancellation. An already terminal job returns `cancel_requested: false`.
Queued/running jobs become interrupted after server restart. No saved job is
silently scheduled. Resume accepts `{}` or `{"max_pages":40}` and retains the
frontier, completed attachments, and past attempt charges. The optional limit is
a new total budget, up to 500. Only durable interrupted/cancelled/partial jobs with
pending or interrupted work and remaining budget can resume. Historical jobs with
no frontier, active workers, unsupported states, or exhausted work return 409
`crawl_not_resumable`. Completed failures and exclusions are not retried.

New jobs include optional `progress` with page and sitemap candidate, attempt,
pending/active, exclusion, and interruption counts. `visited` still counts
completed page attempts. Interrupted attempts consume budget but do not increment
`visited`. See [CRAWL.md](CRAWL.md) for transaction/crash semantics, scheduling,
robots policy, and sitemap bounds. Successful operations return 200, including
ingest, library creation, crawl submission, and accepted resume.

## Error boundary

HTTP application and extractor failures return `application/json` with the shared
`Problem` shape:

```json
{"code":"invalid_json","message":"The request body is not valid JSON."}
```

| Status | Typical code | Meaning |
|---|---|---|
| 400 | `invalid_json`, `invalid_query`, `invalid_path`, `invalid_multipart`, `invalid_request` | Malformed input or a recognized validation failure |
| 404 | `not_found` | Unknown route or saved resource |
| 405 | `method_not_allowed` | Wrong method for a registered route |
| 409 | `crawl_not_resumable` | No supported saved work within the current state/budget |
| 413 | `size_limit` | Request, file, metadata, or source byte limit |
| 415 | `unsupported_media_type` | A JSON endpoint needs a JSON content type |
| 422 | `invalid_request`, `capability_unavailable` | JSON field types do not match, or a known capability is unavailable |
| 429 | Provider-specific code | Recognized upstream rate limit |
| 500 | `internal_error` | Internal or unclassified engine failure |
| 502 | Provider/helper-specific code or `upstream_error` | Recognized upstream failure |
| 503 | `queue_full` | Crawl queue cannot accept another job |
| 504 | `timeout` or helper-specific code | Recognized operation deadline |

Messages from this boundary are fixed, bounded text. They do not echo request
bodies, unknown field names, URLs, local paths, or helper stderr. Use `code` and
HTTP status for control flow, not message matching. This is the existing two-field
JSON envelope, not an RFC 9457 `application/problem+json` implementation. Transport
failures before Axum can form a response are outside this envelope. `HEAD` responses
have no body.

The engine still returns `anyhow` errors. `crates/server/src/error.rs` contains an
explicit compatibility adapter for known provider/helper prefixes and exact
validation messages. Existing arXiv, GitHub, browser, caption, and citation codes
remain where recognized. Broad substring guesses no longer turn unknown faults
into 400, 404, or 422. Unknown failures return 500, even when an engine failure
might later prove to be invalid input. A typed engine error migration is still
required. The HTTP boundary does not redact historical documents, engine warnings,
job errors, or arbitrary metadata returned by successful operations.

## Small client examples

Both examples use this backend, retain no credentials, and need no third-party
packages. They read or ingest one source, fetch the saved document, download its
original, and check its byte count and SHA-256. Ingest saves data in the selected
server. Use a diagnostic server when the input is only a test.

Python 3:

```sh
python3 examples/http_client.py --server http://127.0.0.1:8420 --file note.txt
python3 examples/http_client.py --server http://127.0.0.1:8420 --url https://example.com
```

Node.js 24 or later can run the TypeScript file directly with built-in type
stripping. No npm install or compilation step is needed:

```sh
node examples/http_client.mts --server http://127.0.0.1:8420 --file note.txt
node examples/http_client.mts --server http://127.0.0.1:8420 --url https://example.com
```

The clients also expose `read`, `ingest`, `document`, and `original` methods and a
`ProblemError` with status and code. The TypeScript types are intentionally small
and do not validate every document variant at runtime. These examples buffer one
file in memory. They are not streaming clients or generated SDKs.

## Verification scope

Focused server checks cover extractor errors, total/per-file byte limits, known
versus unknown engine failures, route coverage, operation IDs, resolved schema
references, binary original responses, and tagged block shapes from an actual
ingest response. Keep `Problem` explicitly registered: the shared `IntoResponses`
derive references it but does not collect its schema. Use the binary string schema
for original bytes, not Utoipa's default integer-array schema for `[u8]`.
On October 2, 2026, an isolated loopback HTTP proof exercised Python read/save
from a local Markdown source and TypeScript multipart ingest/save. Both fetched
the saved document and verified the exact 146-byte original by byte count and
SHA-256. The four returned blocks matched the generated content and locator tags.
All 19 documented paths and 22 unique operation IDs were present, and all schema
references resolved. Live malformed JSON/query/multipart, 415, 422, 404, total JSON
body limit, and file limit outcomes returned the expected `Problem` envelopes.
Both clients handled a 404 through their error classes. The isolated services
stopped afterward. The locked CLI/server build and 2 unit plus 8 API tests passed.

This local proof did not contact a public provider, test every source format or
helper, or establish third-party OpenAPI generator compatibility.

`webtool_server::api_router()` exposes the collected Axum routes and contract.
`webtool_server::router(engine)` adds the specification endpoint and error/limit
layers. A future adapter can mount beside that router and use the same engine;
this slice does not implement MCP or another operation backend.
