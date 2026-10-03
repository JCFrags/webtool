# Explicit historical reads

`webtool archive` uses the Wayback capture index and HTTP replay. It never changes
an ordinary live read into an archive read. It creates no capture, follows no
redirect, runs no browser, and fetches no linked page or subresource.

## Select a capture

```sh
webtool archive lookup https://example.org/article \
  --at 20200101120000 --within-days 30 --limit 3
webtool --format json archive lookup https://example.org/article \
  --at 20200101120000 --within-days 30
webtool archive read https://example.org/article --timestamp CAPTURE_TIMESTAMP
```

These are syntax examples, not known captures. Replace `CAPTURE_TIMESTAMP` with
one timestamp returned for the exact original URL. Timestamps use UTC and exactly
14 digits, `YYYYMMDDhhmmss`.

Lookup selects candidates at or before `--at`, within the inclusive lookback
window. `--within-days` accepts 0 to 3660. Zero selects the exact second.
`--limit` accepts 1 to 3. Results show the requested interval, actual capture time,
original capture status, media type, and replay URL. Newest candidates appear
first. This is not a complete timeline or a publication date.

The CDX request uses `matchType=exact`, date bounds, selected columns, and a bounded
negative result limit. One extra row detects possible truncation. CDX can combine
scheme or `www` variants. The reader excludes those rows rather than substituting
another original URL. It does not make extra requests to fill the result limit.
`has_more` means the bounded index response contained more rows, including excluded
variants. It does not count all matching captures.

An empty result has `archive_no_capture`. It does not prove that a page was never
archived. Denial, malformed responses, timeouts, and rate limits remain errors.
No fallback to another archive or ordinary live page occurs.

## Read and retain historical evidence

The read operation takes one exact original URL and capture timestamp. It checks
that pair in the index. Known non-200 capture statuses remain unavailable. Some
CDX revisit records omit original status. Those can be read only after the usual
replay and identity checks, with original status left null and an explicit warning.
Replay HTTP 200 does not establish original HTTP status. No nearby capture is
selected automatically.

The replay request uses `/web/TIMESTAMPid_/ORIGINAL_URL`. All HTTP redirects are
refused, including redirects within Wayback, to another date, or to the live web.
A successful HTTP replay must report a matching `Memento-Datetime`. An original
Link relation, when supplied, must match the selected URL. A missing original
Link relation is not independent confirmation of that URL. Exact index identity
and the requested replay URL remain its evidence.

Supported content is HTML, XHTML, plain text, and Markdown. HTML uses the same
bounded decoding and content selection as live HTTP reads. Empty content, access
or challenge pages, and application shells are not accepted as full text. Other
formats fail explicitly. There is no JavaScript recovery or archive access bypass.

Saved documents retain:

- Exact received replay body, local SHA-256, byte size, and `wayback_replay` role.
- Original URL, selected and confirmed capture timestamps, replay URL, and local
  retrieval time in `metadata.archive`.
- Capture-index status separately from replay HTTP status. `source.status` is
  replay HTTP status. `source.version` identifies the capture timestamp.
- The bounded raw capture-index artifact and its retrieval time.
- Provider CDX digest separately from the local replay-body hash.
- Parser/resolver identity, readable source blocks, and historical warnings.

Replay bytes are retained after HTTP transfer decoding. They are not a WARC record
or a guaranteed byte-identical publisher response. The `id_` route avoids the
ordinary replay toolbar mode but is not a source-cleanliness guarantee. Historical
identity participates in document and cache identity. Identical replay bodies at
different capture timestamps produce different saved documents. Existing live or
historical snapshots are not rewritten.

Links resolve against the captured original URL. Those destinations were not read
or verified as historical sources. An ordinary `read` of such a link is a live
request. A capture date does not establish that all linked assets existed then.

Use ordinary saved-ID reading, `find`, `extract`, libraries, and original export
for accepted captures. `archive read --library NAME` attaches the document to an
existing shared library. `--refresh` checks the same exact capture, not a newer one.

Archive availability does not grant redistribution permission. Item-specific
rights remain unknown unless separate evidence establishes them. Do not use this
route to bypass a live source's access controls.

## Shared bounds and errors

All Engine clones share one archive request at a time and a minimum three-second
interval between request starts. This is a conservative local policy, not a
verified provider quota. Archive capacity is separate from ordinary network
capacity. The operation admission limit still applies.

One operation has at most 30 seconds, including admission, pacing, requests, and
parsing. A smaller `request_timeout_seconds` reduces that bound. Index responses
are capped at 64 KiB or `max_bytes`, whichever is smaller. Replay bodies use
`max_bytes`. Both transfer limits apply after decompression. HTML decoded-text
limits also apply. Synchronous byte-bounded parsing cannot be preempted by an
async deadline already in progress.

Index responses have a shared ten-minute, 64-entry memory cache. Accepted capture
reads use the existing saved-document cache for one day. Cache responses keep
original retrieval times and report `cached=true`. Refresh does not bypass pacing
or a provider cooldown.

HTTP 429 and 503 start a shared cooldown from `Retry-After` seconds or an HTTP
date. A missing or invalid value uses 60 seconds. A huge value prevents requests
rather than overflowing into an immediate retry. No automatic retry occurs.
The client explicitly disables reqwest's default safe protocol-error retries, so
one admitted request does not hide transport retries.
The server uses its configured descriptive `webtool/` User-Agent. Configure any
additional agent identification required by the provider before such use. No key,
cookie, proxy rotation, or paid account is needed or added.

HTTP endpoints use the shared OpenAPI and safe Problem error boundary:

- `POST /v1/archive/lookup`: `ArchiveLookupRequest` and `ArchiveLookupResponse`.
- `POST /v1/archive/read`: `ArchiveReadRequest` and ordinary `ReadResponse`.
- Known archive errors distinguish invalid request, absent exact capture,
  redirect refusal, identity mismatch, unavailable content/format, size limit,
  deadline, cooldown, and upstream failure. They do not expose provider bodies
  or requested URLs in errors.

## Bounded verification

The locked CLI/server build and two focused archive checks passed. The local
fixture retained exact bytes and qualifications, distinct IDs for identical bodies
at different captures, saved-library access, pacing, and redirect/cooldown refusals.
A public CLI read of `https://example.com/` at `20200101114919` confirmed the exact
Memento timestamp and retained a 1,256-byte replay body. Saved-ID find and original
export matched its SHA-256. The CDX revisit status stayed null, with an explicit
warning, while replay HTTP status was 200. This is one source/capture check, not
general archive availability or publisher-byte equivalence.

## Provider references

Checked October 2, 2026:

- [CDX API](https://raw.githubusercontent.com/internetarchive/wayback/master/wayback-cdx-server/README.md)
  documents exact scope, columns, inclusive dates, JSON, and negative limits.
  Its visible changelist is from 2013. Documentation is not live availability proof.
- [Availability API](https://archive.org/help/wayback_api.php) returns a closest
  capture, not an at-or-before guarantee. This implementation uses CDX instead.
- [Automated access](https://archive.org/developers/bots.html) describes User-Agent,
  caching, and cooldown requirements. No fixed numeric CDX quota was verified.
