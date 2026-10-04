# Web search

Web search supports DuckDuckGo, Brave, Startpage, and Yahoo HTML adapters, plus
optional Brave Search API and operator-configured SearXNG JSON adapters.
The default remains DuckDuckGo plus Brave HTML. Search snippets are provider summaries,
not evidence fetched from destination pages. Ordinary search does not fetch results,
rewrite queries, call a language model, or use a fallback provider chain.

Web search excludes recognized paid and sponsored placements on the server, before
results reach text, Markdown, JSON or JSONL clients. This policy is always on.
It does not remove user-saved documents from library search.

## Shared clients and budgets

Each service owns a pooled HTML search client and a separate pooled JSON client.
The JSON client has no shared HTML cookies and follows no redirects. All engine
clones and connected clients share per-provider admission, pacing, and global
request capacity across both client types.
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
does not reserve a future start. The application has no retry or backoff loop.
The JSON client explicitly disables transport retries. The pinned HTML-client
builder still permits reqwest's default retries for protocol negative
acknowledgments, so an exact single-wire-request guarantee does not apply to
HTML search.

Global deadlines accept 100–60000 ms. Provider budgets accept 100–20000 ms, within
the pinned client's 20-second socket ceiling. Global concurrency accepts 1–16 and
provider concurrency accepts 1–4. Intervals accept 0–60000 ms. Search response limits
accept 1024–33554432 bytes. The body limit applies after decompression. Oversized
advertised lengths are rejected when available. The top-level
`request_timeout_seconds` no longer sets web-search timeouts. These async deadlines
bound waiting and network work, but do not preempt a synchronous, byte-bounded
HTML or JSON parse already in progress.

The response retains completed providers when another provider expires. Each failed
or empty provider has a warning, including when every provider fails. Requests stop
at the first applicable provider or global deadline. Search returns one response,
not a stream. It still waits for unfinished providers up to those budgets.

## Provider order and ranking

`search_engines` order determines warning order, provider provenance order, and which
provider supplies a duplicate URL's title and snippet. Network completion order does
not choose this text. Providers run concurrently subject to shared limits. The order
is not a quality preference, serial execution guarantee, or fallback sequence.
Duplicate configured provider names are rejected. Selecting both `brave` and
`brave_api` is also rejected because they use the same upstream index. Replace one
with the other rather than spending two requests for duplicate ranking votes.
SearXNG is one already-merged ranked source. Its `engines`, `positions`, and `score`
fields do not add votes or direct-provider labels. Its upstream engines can overlap
other selected providers, so these source votes are not proof of independent indexes.

Reciprocal-rank fusion adds `1 / (60 + rank)` for each provider's first occurrence of
a valid URL. One provider cannot vote twice for that URL, even in repeated merge
rows. Ties use the normalized URL. URL fragments are removed, while ordinary query
parameters remain. No semantic reranker or automatic query rewriting is used.

Language, date, domain, and result-type filters are not request fields in the
current search API. Query text is sent unchanged. Provider support for syntax
inside that text is not a strict local filter guarantee.

## Optional JSON providers

Only providers listed in top-level `search_engines` run. Adding optional sections
does not enable a provider. All existing TOML files and HTML defaults remain valid.
These adapters need no new dependency or mandatory service. They do not fetch
result pages, cache responses, run an answer model, or request extra pages.

### Brave Search API

1. Replace `brave` with `brave_api` in `search_engines`.
2. Set `[search.brave_api] api_key_env = "BRAVE_SEARCH_API_KEY"`.
3. Supply that variable to the **server process** through protected service
   configuration. Do not put the key in TOML, CLI arguments, client settings, logs,
   or source control. Restart the service safely when its environment changes.

The configuration stores only the environment variable name. It never stores or
serializes the resolved key. Only a selected `brave_api` request reads the variable.
A missing, empty, non-Unicode, or invalid header value returns
`provider_unconfigured` while other providers can still return results. Config and
health inspection do not contact Brave or prove key/plan readiness.

The fixed endpoint is `https://api.search.brave.com/res/v1/web/search`. The key goes
only in a sensitive `X-Subscription-Token` header. No redirect is followed, including
same-origin redirects. There is no endpoint override outside test builds.

Requests set `result_filter=web`, `spellcheck=false`, and `text_decorations=false`.
Only `web.results` are admitted, with absent or `search_result` row types and absent
or `search` collection types. Ads, videos, answers, discussions, rich callbacks,
and other collections are not converted. A documented search envelope without a
web collection can be empty. A missing or malformed envelope is an error, not an
empty success. A row with a malformed title/URL is skipped with a partial warning.

One request asks for at most 20 results. A higher webtool limit adds
`provider_limit`; it does not trigger pagination. Queries over 600 characters or
75 whitespace-separated words return `provider_unsupported` without a request.
Country, language, and safe-search parameters retain Brave's provider defaults.
Account setup, authorized key use, spending limits, and applicable storage rights
remain operator prerequisites. Local synthetic checks do not prove live readiness.

### SearXNG

1. Add `searxng` to `search_engines` only when you have selected an instance.
2. Set `[search.searxng] endpoint` to its complete `/search` or `/` URL.
3. Enable JSON output in that instance's `settings.yml` search formats.

No public instance is selected automatically. The endpoint is server configuration,
not a client request field. It must be HTTP(S) without embedded credentials, query
parameters, or a fragment. Use HTTPS for remote services. HTTP remains available
for trusted local services but transmits queries without encryption. Authenticated
SearXNG gateways are not supported in this slice. A missing endpoint returns
`provider_unconfigured`. A disabled JSON format can return HTTP 403, reported as
`provider_blocked`, not retried or bypassed.

The adapter sends `q` unchanged with `format=json` and `categories=general`. It
accepts only `results` rows with `template=default.html` and a general or empty
category, title, and HTTP(S) URL. The `content` field becomes a discovery snippet.
Other templates, categories, answers, infoboxes, corrections, and suggestions are
not used. Missing `results`, malformed JSON, and error envelopes are errors.
Malformed rows and `unresponsive_engines` produce `provider_partial` when useful
results survive, or `provider_error` when none survive. Only failure counts are
returned, not upstream error strings. Endpoint and upstream readiness, access
rules, and instance settings still require an authorized live check.

For both JSON adapters, explicit boolean `is_ad`, `is_sponsored`, `sponsored`, or
`promoted` flags exclude rows before limits, as do the existing paid-URL checks.
These flags are defensive policy, not guaranteed fields in every upstream schema.
No current schema guarantees that all concealed advertising is marked.

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
- `provider_error`: other network/status failures, refused JSON redirects, response
  byte limits, missing/malformed JSON structures, or parser errors.
- `provider_unconfigured`: an optional provider lacks its endpoint or usable server
  key reference/value. No request is sent and no fallback is substituted.
- `provider_unsupported`: the literal query exceeds a provider's supported bounds.
- `provider_limit`: the requested limit exceeds the Brave API single-page maximum.
- `provider_partial`: usable JSON results survive malformed rows or reported
  SearXNG upstream failures. Failure counts remain visible.

Warnings name the provider and remain in configured order. Errors omit request
URLs, queries, credentials, raw provider bodies, and JSON field values. No outcome
causes an unfiltered fallback, browser retry, provider rotation, or extra
application-level provider request. The HTML transport-retry limit above still
applies. These checks cannot guarantee absolute ad-free results or stable access
to zero-key HTML interfaces.

Use the normal locked build and focused `search::` and `config::tests::search_`
library checks when changing this boundary. The local-response checks cover a fast
result beside a slow provider, blocked/error/empty states, byte limits, shared
admission and pacing, cancellation during pacing, and deterministic merge order.
Existing parser checks cover paid cards and redirects for all four HTML formats.
Two local JSON-provider checks use synthetic responses and a test-owned synthetic
key. They cover selected organic fields, paid rows, shared budgets, one SearXNG
ranking vote, partial/missing/malformed/blocked outcomes, refused redirects, and
credential/header non-disclosure. They do not establish real provider readiness.
No live key or arbitrary public SearXNG instance is needed for these checks.

Primary adapter references, checked October 2, 2026:

- [Brave Web Search reference](https://api-dashboard.search.brave.com/api-reference/web/search/get)
  defines the endpoint, request bounds, spellcheck default, and web result fields.
- [Brave authentication](https://api-dashboard.search.brave.com/documentation/guides/authentication)
  defines the subscription-token header and credential handling.
- [SearXNG Search API](https://docs.searxng.org/dev/search_api.html)
  defines endpoints, JSON enablement, and provider-dependent syntax/filters.
- SearXNG source at
  [`931fd9787b1517d88af2876175d8c31b03e11671`](https://github.com/searxng/searxng/tree/931fd9787b1517d88af2876175d8c31b03e11671)
  defines `get_json_response` in `searx/webutils.py` and the standard result fields
  in `searx/result_types/_base.py`. This inspected source is not an installed instance.

Check one ordinary search through an isolated CLI/server when live verification is
approved. Do not replace focused verification with a broad commercial-query campaign.
See [STATUS.md](STATUS.md) for historical installed observations, not proof that a
new search service revision is active.
