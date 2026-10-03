# Explicit single-media jobs

This optional route uses the common jobs table, token registry, and object store.
It does not start a second scheduler or database. Ordinary reading, captions,
search, crawl frontiers, and saved documents keep their existing behavior.

## Configuration and permission

Media preview and transfer are disabled when `media_download` is absent. An
operator must explicitly supply all allocation fields and existing absolute
helper paths. No application-selected production allocation is provided:

```toml
# Example only: tiny private diagnostics, not a deployment recommendation.
ytdlp_path = "/absolute/path/yt-dlp"
ytdlp_js_runtime = "node:/absolute/path/node"
[media_download]
ffmpeg_path = "/absolute/path/ffmpeg"
ffprobe_path = "/absolute/path/ffprobe"
max_job_bytes = 8388608
max_duration_seconds = 30
max_width = 320
max_height = 240
storage_bytes = 134217728
staging_bytes = 67108864
free_space_reserve_bytes = 1048576
timeout_seconds = 60
```

Existing executable paths do not prove helper compatibility or source access.
The service checks the configured ffmpeg and ffprobe versions during each job.
It does not install helpers, runtimes, models, plugins, or remote components.
The existing explicit yt-dlp JavaScript runtime setting remains in use.

Public visibility and a request do not establish content rights or permission
for the access method. Content copyright permission and provider access terms are
separate. No acknowledgment is treated as proof of either. This unofficial
extractor route makes no claim of general YouTube terms compliance. It does not
use the official YouTube API or a fallback provider. Use only permitted sources
and access methods. Denial does not trigger cookies, login, token plugins, proxy
switching, client changes, or another provider.

## Preview, select, and poll

```sh
webtool video formats 'https://youtu.be/VIDEO_ID_11'
webtool --format json video formats 'https://youtu.be/VIDEO_ID_11'
webtool video download 'https://youtu.be/VIDEO_ID_11' \
  --video-id VIDEO_ID_11 \
  --video-format VIDEO_FORMAT --video-identity VIDEO_IDENTITY \
  --audio-format AUDIO_FORMAT --audio-identity AUDIO_IDENTITY \
  --max-bytes 8388608 --max-duration-seconds 30 --max-width 320 --max-height 240
webtool jobs JOB_ID --wait
webtool jobs JOB_ID --cancel
webtool video export JOB_ID ARTIFACT_ID --output local-media.mkv
```

Replace the placeholders with one valid 11-character video ID and the exact
format IDs and identity hashes returned by the preview. Preview reports source
codecs, container, dimensions, audio language, and exact or estimated sizes when
supplied. Unknown fields remain null. Preview is an observation, not a guarantee
of future availability or rights. No signed stream URLs or raw metadata are
returned. Each job extracts fresh metadata and checks the video identity and
selected-format fingerprints before transfer. Fingerprints cover format ID,
container, protocol, codecs, dimensions, and audio language, not expiring URLs.

The caller must supply finite byte and duration limits within operator ceilings.
Width and height are actual pixels, including portrait video. Optional caller
caps cannot exceed the operator caps. Video dimensions and duration must be known
before transfer. Missing dimensions cannot bypass a hard cap. The service selects
only the requested format IDs. It does not silently substitute lower quality.

For native audio, replace the video selection with:

```sh
webtool video download 'https://youtu.be/VIDEO_ID_11' --video-id VIDEO_ID_11 \
  --audio-only --audio-format AUDIO_FORMAT --audio-identity AUDIO_IDENTITY \
  --max-bytes 8388608 --max-duration-seconds 30 --wait
```

Native audio retains the fetched audio-only stream and its source codec/container.
It is not MP3 conversion, ASR, or the creator's original master. Video either uses
one source stream that already contains audio/video, or one video-only plus one
audio-only stream. Both merge inputs are downloaded separately with fixup disabled.
The service invokes its configured ffmpeg with fixed local stream-copy arguments,
then validates the Matroska output. The audio input uses the supported native
container demuxer list. Opus requires the Ogg demuxer, not only Matroska/WebM.
It does not depend on `--keep-video` or on
unverified helper input-retention behavior. Source inputs remain retained artifacts.
The derived result links to both input hashes. A transferred video is not complete
before required merge and actual output validation succeed.

This first slice supports direct HTTP(S) source formats in a finite container and
codec set. It does not support live/upcoming broadcasts, playlists, channels,
fragmented manifests, format expressions, arbitrary helper arguments, conversion,
transcription, or compatible media resume. An explicit live, upcoming, or unknown
live-status marker is rejected even when `is_live` is false. A true `is_live`
flag also rejects a conflicting nonlive marker. `jobs JOB_ID --resume` reports
`media_resume_unsupported`. Submit a new preview-checked job instead. No transfer
resumes automatically at startup.

## Common HTTP and job contract

| Route | Operation |
|---|---|
| `POST /v1/video/formats` | `MediaFormatsRequest` to `MediaFormatsResponse` |
| `POST /v1/video/download` | `MediaDownloadRequest` to the common `Job` |
| `GET /v1/jobs` and `GET /v1/jobs/{id}` | Common persistent polling |
| `POST /v1/jobs/{id}/cancel` | Common cancellation |
| `GET /v1/jobs/{id}/artifacts/{artifact}` | Exact accepted attachment scoped to the job |

A media request includes `kind: "media"`, selected video ID, one `selection`, and
finite caller constraints. Selection mode is `video` with `video` and optional
`audio`, or `native_audio` with `audio`. Each format selection contains `id` and
`identity`. No caller-selected server filesystem path is accepted.

`Job.request` is a typed crawl/media enum. Old crawl JSON retains its untagged
shape and default decoding. A present media discriminator must decode as media
or fail. It never falls back to a crawl after a malformed discriminator. Crawl
frontier transactions, attempt charges, attachments, and explicit resume remain
unchanged. New media progress/result uses optional `Job.media`. Existing JobState
values remain unchanged. Media stages are queued, metadata, transfer, merge,
validation, publication, and complete. Speed, ETA, and exact transfer totals stay
unknown when unavailable. Progress writes are bounded to about one per second,
plus bounded stage/final updates. Media has one shared slot across Engine clones,
separate from read, caption, and crawl capacity.

Artifact export verifies membership, regular file type, size, and streamed SHA-256
before returning an attachment. Export never returns a server path. The CLI streams
to a private temporary file beside the client-local destination and publishes only
an exact declared byte count. Existing destinations require explicit `--force`.
Hashes identify retained bytes, not a claim of creator-upload identity.

## Budgets, lifecycle, and honest limits

`max_bytes` counts all retained source inputs plus final output. Known or estimated
sizes can reject excessive selections before transfer. Actual retained totals are
checked before publication. The generic HTML/caption `max_bytes` is unchanged.

Admission conservatively counts all existing object-store bytes, not only media,
plus queued/active media reservations for staging, private bounded metadata, and
object copies. Accepted originals and derivatives are therefore counted, including
objects outside the 1000-job display cap. Retained or abandoned staging is counted
as well. Active staging can be counted again against its reservation. This can
reject work earlier than strictly necessary. Free-space reserve checks apply at
admission, during 100 ms sampling, and before publication. Ordinary object-store
writes are not placed under a new global disk quota.

Per-file `RLIMIT_FSIZE` and sampled directory monitoring are defenses, not a hard
aggregate filesystem quota. Multiple files can exceed a sampled limit before the
next check and group termination. Do not describe these limits as strict aggregate
write enforcement. No mount or quota change is made. Helper stdout, stderr, line
length, file count, and whole operation time are also bounded. The deadline
includes queued time and streamed object-copy time. Output line limits are 4096
bytes for transfer and 4 MiB for bounded metadata/probe JSON. Stderr is discarded,
with finite 1 MiB total and 8192-byte line bounds. No raw helper diagnostics or
signed metadata are persisted in jobs or object artifacts.

The runner uses private UUID directories under `media-staging`, generated names,
fixed argument arrays, no shell, null stdin, and explicit process groups. It kills
the group on cancellation, deadline, limit, and after helper completion, then waits
and reaps the direct child and joins pipe tasks before cleanup. ffprobe accepts only
local file protocols and a finite demuxer list. It checks actual containers,
codecs, required streams, dimensions, and duration. Missing ffmpeg/probe fails,
not a hidden quality downgrade.

Cancellation and final publication share the common submission lock. If cancellation
is accepted before publication owns that lock, it cannot publish a late complete
result. A cancellation after completion returns `cancel_requested: false`. During
streamed final object copies, cancellation waits for the publication decision.

Graceful shutdown rejects new job admission, cancels tokens, and waits for workers
and cleanup. Startup marks unfinished jobs interrupted without a hidden network
request. Recovery removes only verified service-owned staging after old writers
stop. A live/ambiguous owner or descendant group retains staging with a warning.
Compatible resume and automatic retry are unsupported. Accepted objects, libraries,
other files, and user paths are never removed by this cleanup. Publication failure
can leave immutable unreferenced objects. These bytes remain counted and are not
automatically deleted.

## Verification boundary

Use the normal locked CLI/server build before relevant focused existing checks.
Follow the worktree root-mtime, external-lock, two-job, and private-binary-copy
procedure in `AGENTS.md`. Bounded private proof used a no-network synthetic
metadata/transfer helper and actual ffmpeg/ffprobe 8.1.2. The generated 64x48 VP8
and Opus inputs produced a 1.008-second Matroska result. Both inputs were retained,
and client exports matched the retained hashes. Native audio, common progress,
cancellation with stopped/reaped helpers and staging cleanup, dimension rejection
before transfer, startup interruption without helper retry, and unsupported resume
were exercised. These are individual fixture results, not current public YouTube
compatibility, provider permission, production migration, installed activation, or
a large cancellation/crash campaign.
