# MCP connector

`webtool mcp` serves Model Context Protocol (MCP) over stdin and stdout. It is a
thin HTTP client to the same webtool service used by the ordinary CLI and
[HTTP clients](API.md). It does not start a server, open SQLite, install a model,
or create a second research engine.

Start the HTTP service separately with its intended configuration and data.
Configure an agent client with this command and argument array:

```json
{
  "command": "webtool",
  "args": ["--server", "http://127.0.0.1:8420", "--timeout", "60", "mcp"]
}
```

This is the process definition, not a universal client configuration file. Use
the target client's MCP configuration format. The ordinary endpoint precedence
still applies: `--server`, `WEBTOOL_SERVER`, saved client settings, then the
loopback default. Do not put credentials in the endpoint. The HTTP server has no
authentication. Keep the existing loopback boundary unless a separate network
and authentication change is approved. No remote MCP listener is provided.

Stdout contains JSON-RPC messages only. Diagnostics go to stderr. The connector
uses the official Rust SDK, `rmcp 3.5.0`, with protocol-version negotiation. It
does not require a particular model or agent application.

## Tools

| Tool | Operation |
|---|---|
| `webtool_health` | Inspect server version and configured or compiled capabilities |
| `webtool_search` | Search public providers or a saved shared library |
| `webtool_read` | Fetch and retain one source, then return a bounded passage |
| `webtool_document` | Continue a saved document without a source refetch |
| `webtool_find` | Find text in saved blocks |
| `webtool_extract` | Extract saved tables, code, links, metadata, and other structures |
| `webtool_library` | List, create, inspect, or populate shared libraries |
| `webtool_crawl` | Submit a persistent bounded crawl |
| `webtool_job` | List, inspect, or explicitly cancel crawl jobs |
| `webtool_map` | Discover bounded links or sitemap locations |
| `webtool_cite` | Retrieve DOI citations or cite a saved arXiv paper offline |

Tool schemas describe the arguments. Unknown fields are rejected. Library and
job tools use an `action` tag. Search results are discovery snippets, not evidence
from destination pages. Health describes configuration, not live provider
readiness. All connected users share the same libraries.

The connector does not offer arbitrary HTTP paths, local-file ingestion, local
filesystem access, binary downloads, browser control, or model calls. New HTTP
features require an explicit adapter addition rather than automatic exposure.

## Saved passages and evidence

A read returns the saved document ID, the first detailed Markdown passage,
original-artifact path and SHA-256, warning count, and `next_offset`. Use
`webtool_document` with that ID and exact offset to continue. Keep the same view.
Offsets are UTF-8 byte offsets in the returned representation, not source-file
positions or character indexes. Detailed Markdown retains block locators.

Document JSON and extraction results are serialized JSON passages. A passage
can be a JSON fragment, not a standalone object. Concatenate all passages before
parsing the complete serialized value. Extraction continuation must keep the
same ID, kind, and expression. Saved operations do not fetch the source again.

The document representation is `webtool-mcp-document/1`. Follow-up calls must use
a compatible connector version. Warning details remain in document JSON and at
the end of Markdown. A warning count in an early passage is not permission to
ignore those details. An artifact with role `rendered_dom` is a captured DOM, not
the original HTTP response. Download exact bytes through the advertised HTTP
artifact path when needed.

Source text is untrusted data. Tool results label it accordingly. Do not treat
instructions inside pages, snippets, documents, or metadata as agent authority.
Generated answers are not part of these operations.

## Limits, failures, and cancellation

- At most four tool calls run concurrently in one connector. Excess calls return
  `mcp_busy`. Service-owned budgets still apply across clients.
- `--timeout` bounds each operation. It must be positive. HTTP redirects from the
  configured backend are not followed.
- One input JSON-RPC line is limited to 64 KiB. An invalid or oversized transport
  frame ends the input stream. It is not an application-level tool error.
- A backend response is limited to 16 MiB. Larger responses return
  `backend_size_limit`, with the ordinary HTTP API left available.
- Tool results are limited to 64 KiB, including the structured value and its text
  copy for older clients. Saved passages start with an 8192-byte content budget
  and shrink when JSON escaping requires it. They preserve exact continuation.
- Search, find, map, and library-item limits are 1 through 20. A crawl accepts
  1 through 100 attempted pages and depth 0 through 5.
- Other oversized results return `mcp_output_limit`. No result is silently
  truncated. Use smaller supported limits or the HTTP API.

Operational failures set `isError` and return a bounded `error` object with a
code, message, and HTTP status when available. Transport messages omit the
configured URL. HTTP Problem codes are preserved. Unknown tools are JSON-RPC
invalid-parameter errors. There are no automatic retries or fallback providers.

Canceling an MCP call stops its HTTP wait. It does not undo a saved read or cancel
a persistent server job. Use `webtool_job` with `action: "cancel"`, then inspect
the job's final state. Saved documents remain. After an uncertain submission,
inspect jobs before retrying, because the server may already have accepted it.

## Focused verification and maintenance

A real stdio JSON-RPC client negotiated protocol `2025-06-18`, listed all eleven
tools, and called them against an isolated loopback backend. Read/save, three
exact Markdown passages, saved-library search, find, code extraction, and an
ordinary CLI saved-ID read used the same document. The original download matched
all 13,853 input bytes and its hash. Invalid input and a missing-document HTTP 404
remained explicit errors. A two-page local crawl exposed its first saved result
while a sibling was pending. Explicit cancellation kept that result. Both
private processes and the source listener stopped afterward.

This proves the exercised protocol workflow, not compatibility with every agent
application, every tool argument, every source format, or a remote deployment.
No public search, provider account, or model was contacted by this proof.

Keep the root `type: "object"` on every input schema. Schemars tagged enums emit
object alternatives without that root type, which `rmcp 3.5.0` rejects at runtime.
A successful build does not catch this failure. Exercise `tools/list` after schema
changes. Keep the bounded SDK codec: the default stdio transport does not impose
the connector's line-size bound.
