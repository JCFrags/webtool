# Search ad exclusion

Web search excludes recognized paid and sponsored placements on the server, before
results reach text, Markdown, JSON or JSONL clients. This policy is always on.
It does not remove user-saved documents from library search.

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
owns this small card-admission parser and reuses the pinned HTTP client. It keeps
the existing provider endpoints, query parameters, Yahoo settings, header timeout,
client socket timeout, configured outer deadline and four-provider concurrency.
Responses also obey the existing configured byte limit. Dependency pins are unchanged.

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

No matches, excluded ads or changed provider markup can produce `provider_empty`.
Network errors and rate limits remain visible as `provider_error`. Neither case
causes an unfiltered fallback, browser retry or extra provider request. Search
snippets remain provider summaries, not evidence fetched from destination pages.

Use the normal locked build and focused `search::` library checks when changing
this boundary. Check one ordinary search through the CLI. Do not replace focused
verification with a broad commercial-query campaign. See [STATUS.md](STATUS.md)
for the exact exercised scope and remaining live-provider limits.
