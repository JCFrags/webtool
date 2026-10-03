# Video discovery and supplied captions

Video operations use the server's configured yt-dlp helper without an API key.
They are explicit, separate from ordinary web search, and make no LLM calls.
They do not download audio/video, transcribe, translate, enrich each search result,
or process playlists. Source access and permitted media use remain separate gates.
Public visibility does not establish permission for every use. Provider terms and
unofficial extraction can restrict this path.

## CLI

Quote the complete search query. The CLI sends that string unchanged:

```sh
webtool video search 'Rust async cancellation' --limit 5
webtool video tracks 'https://www.youtube.com/watch?v=VIDEO_ID_11'
webtool video captions 'https://www.youtube.com/watch?v=VIDEO_ID_11' --language en --choice provided
webtool video captions 'https://www.youtube.com/watch?v=VIDEO_ID_11' --language en --choice automatic --refresh
```

Replace `VIDEO_ID_11` with one valid 11-character video ID. Watch and youtu.be URLs
are supported. Playlist and tracking parameters are removed. Other URL forms are
not silently converted. Global `--format json` exposes the full typed response.
Text and Markdown output escape terminal control sequences only for presentation.
Warnings go to stderr.

Search accepts 1 to 20 hits, default 5, and a nonempty query of at most 4096 bytes
without NUL. It uses one service-built `ytsearchN:QUERY` argument and flat metadata.
It does not fully extract individual results or fetch more hits to replace omitted
entries. Results stay in provider order. `provider_rank` is their original one-based
position, so omitted entries can leave gaps. IDs and canonical watch URLs must agree
with supplied identities. Unsupported or inconsistent entries produce a warning.
No guessed titles, durations, dates, counts, or channel values replace absent data.
Zero duration/count values remain zero. Search descriptions are discovery text,
not transcript evidence. Search and track inventory are not cached or saved as
caption documents.

Track inventory lists one selectable untranslated VTT track for each exact language
and origin. It exposes no signed track URLs, headers, or raw helper metadata. Empty
inventory includes `media_captions_unavailable`. Inventory is a point-in-time list,
not a guarantee that later retrieval will succeed. Other formats and translated
tracks are not selectable in this slice.

`--choice` accepts:

- `provided-first`: Prefer a supplied `subtitles` track, then a provider-generated
  `automatic_captions` track in the same exact language. This is the default.
- `provided`: Require a supplied `subtitles` track. This label does not prove human
  authorship.
- `automatic`: Require the provider's automatic track. The document includes a
  warning that YouTube, not webtool, generated those captions.

Language codes are literal and case-sensitive. `en`, `en-US`, and `EN` are distinct.
A missing exact language/origin fails. There is no substitution, translation, ASR,
or generated text. Existing `read` auto/captions routing and `media` retain their
provided-first default. Explicit HTTP reads remain HTML reads.

The caption document retains the byte-identical downloaded VTT.
Cue locations use integer milliseconds. Stable metadata records video identity,
helper version when supplied, language, track format/name, and actual origin.
Source HTTP status remains null when the helper does not supply it. The helper
receives pruned metadata through `--load-info-json` to keep source headers and
exactly one track. The generic HTTP reader does not fetch the expiring track URL.

Explicit provided/automatic choices have distinct cache keys. `--refresh` retrieves
again. Language and actual origin distinguish new provenance snapshots even when
two tracks have identical bytes. Caption parser revision is
`yt-dlp+native-captions/3`. Old saved documents and IDs are not rewritten. Repeated
identical accepted caption snapshots retain their ID. Non-caption cache identities
stay unchanged.

## HTTP contract

All operations are POST JSON and appear in `/openapi.json`:

| Path | Request | Response |
|---|---|---|
| `/v1/video/search` | `{"query":"literal query","limit":5}` | `VideoSearchResponse` |
| `/v1/video/tracks` | `{"url":"https://youtu.be/VIDEO_ID_11"}` | `CaptionTracksResponse` |
| `/v1/video/captions` | `{"url":"https://youtu.be/VIDEO_ID_11","language":"en","choice":"provided","refresh":false,"library":null}` | `ReadResponse` |

JSON choice values are `provided_first`, `provided`, and `automatic`. Omitted choice
uses `provided_first`. Omitted language uses `en`. A caption read can attach the
saved document to one existing shared library. Search/track metadata fields use
null for unavailable optional values. Helper version and retrieval time identify
the observation, not a live-readiness certification.

The server returns its bounded `Problem` envelope. Known validation failures use
400. Missing helper/captions use 422. Denial or inconsistent identity uses 502.
Recognized rate limits use 429. Helper output limits use 413 and deadlines use 504.
Error messages do not echo URLs, local paths, request text, or helper diagnostics.
Successful warnings retain bounded URL-redacted helper diagnostics. See
[API.md](API.md) for the common boundary and its limits.

## Helper limits and remaining work

All video operations, including ordinary caption reads, share one service-owned
semaphore across Engine clones. Existing `parse_concurrency` sets its active limit.
`helper_timeout_seconds` covers admission, metadata, caption retrieval, and parsing.
The existing helper runner caps stdout with `max_bytes`, stderr with 1 MiB, and
kills the helper process group on cancellation or timeout. A killed child can
remain defunct until the next helper request reaps it. Bare PID existence does
not establish that a helper still runs. Each operation has a private temporary
directory. Signed metadata and transient files are removed when the operation ends. Unix file-size limits also bound caption files.

Arguments are service-built, not arbitrary client arguments. Calls disable user
configuration, plugins, remote components, cache, proxy use, media download, and
playlists. Retries are zero. Denial, rate limit, login, or token requirements do
not trigger cookies, token plugins, proxy/client changes, another provider, or a
helper installation. Existing explicit JavaScript runtime configuration is used.

The runner collects bounded output. It is not streaming media progress or resumable
staging. Media transfer jobs, playlist traversal, download cancellation/resume,
rights/retention policy, and their CLI/API interfaces are not implemented here.
They require a separate permitted-source and job-interface milestone.

Implementation sources checked against yt-dlp `2026.08.19`:

- [Release README](https://raw.githubusercontent.com/yt-dlp/yt-dlp/2026.08.19/README.md):
  flat extraction, JSON output, and fixed helper options.
- [YouTube search extractor](https://raw.githubusercontent.com/yt-dlp/yt-dlp/2026.08.19/yt_dlp/extractor/youtube/_search.py):
  bounded `ytsearchN` video-only discovery.

## Verification scope

The normal locked CLI/server build and two focused media checks passed. An isolated
synthetic CLI/server flow verified literal queries, provider order, unknown values,
URL-free inventory, exact language/origin choice, default provided-first reads,
cache separation, and distinct origin IDs for identical 127-byte VTT originals.
Cue locations remained integer milliseconds. The flow also checked shared admission,
known errors, output limits, deadlines, and generated schema references.

No public metadata, caption, or media request was used for this proof. Synthetic
checks do not establish current YouTube availability, provider permission,
extraction accuracy, or installed activation.
