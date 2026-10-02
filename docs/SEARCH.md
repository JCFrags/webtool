# Web search

Web search uses the configured DuckDuckGo, Brave, Startpage, or Yahoo HTML adapters.
The default remains DuckDuckGo plus Brave. Search snippets are provider summaries,
not evidence fetched from destination pages. Ordinary search does not fetch results,
rewrite queries, call a language model, or use a fallback provider chain.

Web search excludes recognized paid and sponsored placements on the server, before
results reach text, Markdown, JSON or JSONL clients. This policy is always on.
It does not remove user-saved documents from library search.

## Shared clients and budgets

Each service owns one pooled search HTTP client. All engine clones and connected
clients share its per-provider admission, pacing, and global request capacity.
These search limits are separate from ordinary reading and crawling capacity.
Multiple server processes do not share these limits. Web search is not cached.

Existing TOML files remain valid. The optional `[search]` section sets:

| Setting | Default | Meaning |
|---|---:|---|
| `timeout_ms` | 8000 | Whole-search deadline, including admission and pacing waits |
| `concurrency` | 4 | Active provider requests across all searches in this service |
| `max_bytes` | 4194304 | Per-provider response limit, also capped by top-level `max_bytes` |

Each `[search.providers.NAME]` table can set:

| Setting | Default | Meaning |
|---|---:|---|
| `timeout_ms` | 6000 | Request and body budget after admission, within the whole-search deadline |
| `concurrency` | 1 | Concurrent admitted operations for this provider, including pacing waits |
| `interval_ms` | 1000 | Minimum interval between request starts, across all searches |

Only names in `search_engines` are called. Missing provider tables and fields use
the defaults above. Zero `interval_ms` explicitly disables pacing. A provider does
not hold global request capacity while it waits for its interval. A canceled waiter
does not reserve a future start. No retries or backoff requests occur.

Global deadlines accept 100–60000 ms. Provider budgets accept 100–20000 ms, within
the pinned client's 20-second socket ceiling. Global concurrency accepts 1–16 and
provider concurrency accepts 1–4. Intervals accept 0–60000 ms. Search response limits
accept 1024–33554432 bytes. The body limit applies after decompression. Oversized
advertised lengths are rejected when available. The top-level
`request_timeout_seconds` no longer sets web-search timeouts. These async deadlines
bound waiting and network work, but do not preempt a synchronous, byte-bounded HTML
parse already in progress.

The response retains completed providers when another provider expires. Each failed
or empty provider has a warning, including when every provider fails. Requests stop
at the first applicable provider or global deadline. Search returns one response,
not a stream. It still waits for unfinished providers up to those budgets.

## Provider order and ranking

`search_engines` order determines warning order, provider provenance order, and which
provider supplies a duplicate URL's title and snippet. Network completion order does
not choose this text. Providers run concurrently subject to shared limits. The order
is not a quality preference, serial execution guarantee, or fallback sequence.
Duplicate configured provider names are rejected.

Reciprocal-rank fusion adds `1 / (60 + rank)` for each provider's first occurrence of
a valid URL. One provider cannot vote twice for that URL, even in repeated merge
rows. Ties use the normalized URL. URL fragments are removed, while ordinary query
parameters remain. No semantic reranker or automatic query rewriting is used.

Brave Search API and an operator-configured SearXNG endpoint are planned follow-up
adapters, not implemented providers. No key, account, or additional service is
required by this slice. Language, date, domain, and result-type filters are not
request fields in the current search API. Query text is sent unchanged. Provider
support for syntax inside that text is not a strict local filter guarantee.

## Admission boundary

`crates/engine/src/search/organic.rs` selects the four existing result formats and
checks each card before extracting its title, URL and snippet. It rejects paid
structural markers, sponsored ancestors, `rel="sponsored"`, explicit paid labels,
and standalone English ad badges outside titles and snippets. DuckDuckGo also
requires the affirmative `web-result` class. There is no broad-selector fallback.

The parser checks URLs before and after decoding the normal DuckDuckGo and Yahoo
organic wrappers. It rejects DuckDuckGo ad redirects, Bing `/aclick`, known Google
ad-delivery domains, paid-click identifiers and explicit paid `utm_medium` values.
The merge boundary repeats the URL check. Ad URLs are dropped, not rewritten to
hide their origin. Ordinary query parameters remain intact.

Admission checks precede provider result limits and reciprocal-rank merging.
Excluded placements cannot fill the limit or contribute a ranking vote. An
independent organic result for the same merchant or URL remains eligible.
Merchant sites are not blocked as a class. Words about advertising in titles or
snippets are not a reason to remove a result.

## Why the parser is local

Pinned `metadata-search-engine-rs/0.3.1` returns only title, URL, snippet and engine.
Its parser discards paid DOM context. A filter on that flat result cannot detect
a direct merchant link inside a sponsored container. The application therefore
owns this small card-admission parser and reuses one pinned HTTP client for the
service lifetime. It keeps the existing provider endpoints, query parameters,
Yahoo settings, headers, and client socket ceiling. The service budgets above replace
the former hard-coded header timeout and per-search concurrency. Dependency pins
are unchanged.

Do not restore the flat upstream adapters without an equivalent pre-conversion
paid-result check. Keep checks before URL unwrapping and result limits, not just
in CLI presentation. Web search is not cached, so no saved-ID migration is needed.

## Evidence and limits

- [SearXNG DuckDuckGo selection](https://github.com/searxng/searxng/blob/931fd9787b1517d88af2876175d8c31b03e11671/searx/engines/duckduckgo.py#L493-L501)
  explicitly selects `web-result` and excludes `result--ad result--ad--small`.
- [DuckDuckGo ad-link examples](https://serpapi.com/duckduckgo-light-ads) show `/y.js`,
  ad parameters and nested Bing `/aclick` URLs. These are documented third-party
  examples, not a live-response guarantee.
- [SearXNG Startpage comments](https://github.com/searxng/searxng/blob/931fd9787b1517d88af2876175d8c31b03e11671/searx/engines/startpage.py#L148-L151)
  describe `#sponsored` ancestry. Its current parser uses structured data instead,
  so this comment does not verify today's Startpage HTML.

Generic structural and badge checks are application policy, not a claim that all
providers use these exact markers. Unknown future markup, concealed promotion,
and advertising inside destination pages cannot be ruled out. A paid-click URL
can also appear in an organic placement; this strict policy still excludes it.

Warnings distinguish these outcomes without changing the response schema:

- `provider_empty`: no parsed organic results. This can mean no matches, excluded
  ads, or unrecognized markup. It does not prove the provider has no matches.
- `provider_blocked`: HTTP 401, 403, 429, or 451, or a recognized challenge form
  without parsed organic results. The HTTP status remains in the warning. Structural
  challenge detection is limited. Unknown challenge pages can still appear empty.
- `provider_timeout`: the provider budget, global deadline, or socket timeout expired.
  Global-deadline warnings explicitly include admission and pacing waits.
- `provider_error`: other network/status failures, response byte limits, or parser errors.

Warnings name the provider and remain in configured order. HTTP errors omit request
URLs and queries. No outcome causes an unfiltered fallback, browser retry, provider
rotation, or extra provider request. These checks cannot guarantee absolute ad-free
results or stable access to zero-key HTML interfaces.

Use the normal locked build and focused `search::` and `config::tests::search_`
library checks when changing this boundary. The local-response checks cover a fast
result beside a slow provider, blocked/error/empty states, byte limits, shared
admission and pacing, cancellation during pacing, and deterministic merge order.
Existing parser checks cover paid cards and redirects for all four HTML formats.
Check one ordinary search through an isolated CLI/server when live verification is
approved. Do not replace focused verification with a broad commercial-query campaign.
See [STATUS.md](STATUS.md) for historical installed observations, not proof that a
new search service revision is active.
