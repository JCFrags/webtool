# Durable bounded crawling

Crawling is explicit, HTTP-only, and application-owned. The CLI, HTTP API, and
shared libraries use the same engine and SQLite store. There is no background
public crawl, rendered crawler, distributed queue, or complete-site archive.

## Commands

```sh
webtool crawl https://example.com/ --max-pages 20 --max-depth 2 --library research
webtool jobs JOB_ID
webtool jobs JOB_ID --wait
webtool jobs JOB_ID --cancel
webtool jobs JOB_ID --resume --max-pages 40 --wait
```

Create the library first. Cancellation preserves saved documents. Poll the job
for the final cancellation state. Stopping a CLI wait does not cancel the job.

`POST /v1/jobs/{id}/resume` accepts `{}` or `{"max_pages":40}`. Resume is allowed
only for a durable interrupted, cancelled, or partial job with work left within
its budget. The optional value is a new **total** page-attempt budget, not an
additional allowance. It can keep or increase the previous limit, up to 500.
Resume returns 409 `crawl_not_resumable` for an active worker, an unsupported
state, a historical job without a frontier, or no remaining work/budget.
Invalid limits return 400. Resume does not retry completed failures or exclusions,
reset counts, change origin/depth/library, or replace the job ID.

## Persistence and crash semantics

SQLite schema version 2 adds `crawl_runs` and `crawl_frontier`. Existing documents,
artifacts, libraries, annotations, job payloads, and snapshot IDs are not rewritten.
Historical job JSON defaults to no progress and has no recoverable frontier.
Before an installation upgrade, retain a consistent pre-upgrade database copy
and the original artifact directory for rollback. Older schema-1 binaries refuse
a schema-2 database. Do not lower `user_version` to bypass this check.
New jobs retain URL admission order, kind, depth, state, attempt charges,
interruption count, exclusion/error reason, and saved document reference.

Each started page or sitemap attempt is charged and checkpointed **before**
pacing, queueing, or network access. A successful page checkpoint commits its
frontier state, newly admitted URLs, job counts/IDs, and library attachment in one
transaction. Library attachment and job document IDs are deduplicated. Original
bytes and the parsed document are saved through the ordinary reader first. A
crash before the crawl checkpoint can therefore leave an unattached saved
snapshot. Resume does not claim exactly-once network access or eliminate these
unattached snapshots. Identical accepted snapshots retain their existing IDs.

On startup, queued and running jobs become `interrupted`. Active entries become
interrupted attempts. No job is silently scheduled, and no robots, sitemap, or
page request starts until explicit resume. Resume requeues interrupted entries
without refunding their earlier charges. Another interruption consumes another
attempt if that entry starts again. Pending entries consume no charge until they
start. Cancellation uses the same interruption policy for active work.

`visited` counts completed page entries, successful or failed. `failed` counts
completed failed page entries, not extraction warnings or interrupted attempts.
`document_ids` lists usable saved snapshots. A job with attempted page completions
but no usable documents is failed. Policy/discovery/extraction losses remain
warnings and make a usable result partial. Resume retains previous warnings.

`progress` reports page candidates, total charged attempts, pending/active entries,
excluded entries, cumulative interrupted attempts, and separate sitemap candidate,
attempt, pending, active, and interruption counts. It is absent on old jobs. Counts
are checkpoint observations, not instantaneous network activity. Detailed reasons
remain in the frontier. Public job warnings are capped at 256 plus a truncation
warning. There is no public frontier-edit or failed-request retry API.

## Scheduling and scope

- Page budget: 1 to 500 started attempts, including failures and interruptions.
- Link depth: 0 to 8. The start URL and sitemap page seeds have depth zero.
- Page candidates: at most 10,000, including excluded URLs. Link scanning also
  stops after 10,000 links per saved document. Discovered oversized/invalid URLs
  are skipped with a warning. Page URLs are limited to 8192 bytes.
- Origin: the same scheme, host, and effective port as the start URL. Page and
  sitemap redirects stay on that origin, reject credentials, and check robots
  at each hop. URL fragments are removed. Query values and order are retained.
- Request bytes and deadlines: the existing configured reader limits apply to
  each page. There is no separate total crawl-byte limit.

Workers refill vacant slots at the current depth after each checkpoint instead
of waiting for the slowest sibling in a fixed batch. The next depth starts only
when all active work at the current depth finishes. Each usable completion is
attached and visible before the worker waits again. The existing network, parse,
operation, job, and crawl capacity limits provide backpressure.

Request-start pacing is shared across jobs on an origin, with a minimum delay of
0.5 seconds and any accepted larger `Crawl-delay`. Robots and sitemap metadata also
use network capacity and pacing. Redirect hops do not get separate pacing slots.
There is no shared robots cache, `Retry-After` scheduler, or per-origin active
request limit beyond existing worker capacity. Restart does not retain pacing
reservations. These limits are not an RFC conformance or server-load guarantee.

## Robots policy

The product token is `webtool`, matched case-insensitively. Matching groups are
combined. The wildcard group applies only when no exact product-token group
exists. An empty allow/disallow line still separates consecutive groups. Sitemap
records do not terminate a group.

Path matching is case-sensitive and includes the query. Unreserved ASCII percent
escapes are decoded before comparison. Reserved escapes remain distinct and use
uppercase hex. Raw UTF-8 becomes percent-encoded octets. Rules use anchored prefix
matching, `*`, and a trailing `$`. The longest normalized octet pattern wins, with
allow winning an equal-length tie. `/robots.txt` is implicitly allowed. Rules
outside a group and unsupported records do not grant additional permission.

This is a stricter bounded policy, **not complete RFC 9309 conformance**:

- Robots redirects are limited to ten hops on the original origin. RFC 9309
  recommends following at least five redirects across authorities. A refused
  cross-origin redirect stops the job rather than assuming a missing file.
- Only HTTP 200 is parsed. 404/410 mean no rules. 401/403 deny all. Other statuses,
  network errors, invalid UTF-8, matcher limits, compressed metadata, and size
  failures stop the job. No stale-cache or long-term unavailability policy exists.
- Robots bodies use identity encoding and a cap of the smaller of configured
  `max_bytes` and 512 KiB. Non-UTF-8 encoding recovery is unsupported.
- `Crawl-delay` is a nonstandard extension. Valid finite nonnegative values raise
  the minimum delay. Values over 60 seconds stop the job, not shorten the delay.

Robots rules are not access authorization. This feature does not bypass login,
paywall, consent, challenge, or network boundaries. The existing HTTP URL validator
is not a private-network egress security boundary. Broader server exposure still
needs a separate approved network/security design.

## Sitemap discovery

```sh
webtool crawl https://example.com/ --sitemap https://example.com/sitemap.xml --wait
webtool crawl https://example.com/ --discover-sitemaps --wait
```

Use up to eight explicit roots, or opt in to `Sitemap:` records from robots.txt,
or both. There are no guessed `/sitemap.xml` requests. Sitemap URLs, advertised
roots, child indices, and page seeds must stay on the crawl origin and pass robots.
Page locations must also stay under the directory of their resolved sitemap.
Cross-host sitemap delegation from robots.txt is unsupported, even though the
sitemap protocol permits it with ownership evidence.

Discovery runs before page scheduling. It accepts UTF-8 XML `urlset` and
`sitemapindex`, with the standard namespace or no namespace. Only direct `loc`
children of the corresponding `url`/`sitemap` entries are used. Foreign extension
locations are not page evidence. DTDs/external entities, feeds, and text-list
sitemaps are unsupported. `lastmod`, priority, and change frequency are not used as
proof of source currency or permission.

Operational bounds are deliberately below the sitemap protocol's 50 MB and
50,000-entry limits:

- At most 32 sitemap candidates and 32 started sitemap attempts per job, including
  exclusions and interrupted attempts. Resume does not reset this separate budget.
- At most three child-index levels after a root. Cycles and duplicate URLs do not
  start another request. URL identity retains meaningful queries.
- Per body: the smaller of configured `max_bytes` and 1 MiB. Automatic metadata
  decompression is disabled and requests ask for identity encoding. Compressed
  responses and gzip sitemap files are refused, not expanded.
- Per XML input: scan at most 32 index entries or 10,000 URL entries. Locations
  must be shorter than 2048 bytes. Page seeds also occupy the page candidate limit.

Each metadata completion and admission is checkpointed. Sitemap failure does not
hide usable pages from other sitemap inputs or the start page. Exclusions,
cycles, invalid entries, unsupported input, and reached limits remain warnings.
`map` remains a single-source listing operation and does not expand nested maps.

## Source and verification notes

Policy references: [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309.html) and the
[Sitemap protocol](https://www.sitemaps.org/protocol.html). These describe protocol
requirements, not proof of this implementation's conformance.

For diagnostic builds, use private binary copies and isolated loopback data.
A private local graph exercised CLI/HTTP reads and library search while a slow
crawl sibling was pending, cancellation, a hard server restart, explicit resume,
retained attempt charges, and attachment deduplication. It ended with seven
completed page attempts, five saved snapshots, two failures, nine charged
attempts, and two interrupted attempts. Completed pages were fetched once. The
interrupted page was fetched three times, each after explicit scheduling.
Nested/cyclic sitemap checks reached 32 candidates and eight metadata attempts.
They refused a depth-four child, out-of-origin/directory locations, robots-denied
URLs, oversized and compressed inputs, and a cross-origin robots redirect.
These are bounded local observations, not public-site or RFC-conformance tests.
No kill-at-every-checkpoint campaign, concurrent-server access, or production data
migration was tested.

When worktrees share a Cargo target directory, hold a common build lock and force
this worktree's crate roots and existing `build.rs` files to a fresh modification
time before Cargo. Cargo's relative dependency paths can otherwise reuse another
worktree's workspace artifact. Do not touch another worktree, delete dependency
artifacts, or run mutable shared-target binaries as candidates.
