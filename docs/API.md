# HTTP API

The initial API prefix is `/v1`.
The server is intended for a trusted group and has no authentication.
The CLI is the first client, but any HTTP client can submit the same messages.

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

## Contract details

`read` returns `document` and `cached` fields.
`search` returns provider-labeled results and warnings.
`find` offsets are UTF-8 byte offsets into the returned block text.
They are not Unicode character indexes or original-file offsets.

`extract` supports `tables`, `links`, `code`, `images`, `metadata`, `outline`, `json_pointer`, and `css`.
A missing JSON pointer has `found: false` rather than masquerading as a retrieved null.

`ingest` accepts exactly one `file` part.
Optional parts are `name`, `library`, `actor`, and `selector`.
A name is a filename, not a server filesystem path.

`original` returns an attachment using `application/octet-stream`.
This prevents the download endpoint from displaying saved HTML as an application page.

Application errors usually return `code` and `message`.
Framework-level body and JSON rejections still need complete normalization.
API error classification currently uses message matching and should become typed before a stable release.

The endpoint definitions are implemented in source but have not been executed in this environment.
