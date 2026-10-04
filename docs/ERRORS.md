# Typed backend errors

The HTTP and ordered-batch boundary uses one engine error contract. It does not
classify failures by message text. See [API.md](API.md) for the two-field JSON
`Problem` envelope and request-extractor errors.

## Producer contract

`webtool_engine::error` exposes `ErrorKind`, `EngineError`, `ErrorCategory`,
`StatusIntent`, and `PublicError`. A producer selects the known failure kind:

```rust
use webtool_engine::error::ErrorKind;

let error = ErrorKind::InvalidUrl.error();
let with_detail = ErrorKind::InvalidUrl.context("private diagnostic detail");
let with_cause = ErrorKind::SourceRequestFailed.with_source(error);
```

Use `with_source` when wrapping an existing error. `anyhow::Context` can also
carry typed intent without discarding its underlying error. An outer
`EngineError` selects the public intent even when its cause has another intent.
Do not infer a new kind from `Display`, a prefix, or a formatted cause chain.
A plain string that equals an old validation message is still untyped.

`EngineError::from_anyhow` and `error::kind` use downcasts. They recognize common
engine errors, existing `CodeError`, `ScholarlyError`, `PmcError`, and
`ExternalCodeError` values, and the reader's existing `MissingContent` marker.
That marker still selects automatic read recovery inside the engine. The HTTP
boundary does not initiate recovery. Unknown values select `Internal`.

`PublicError` contains only a status intent and static code/message strings.
`EngineError` retains its private diagnostic cause and uses a fixed public
`Display`. Cause text can contain source URLs, paths, helper output, or secrets.
Do not return or log a cause chain at a public boundary. A typed diagnostic used
as an `anyhow` context remains accessible through its typed downcast, not
necessarily through `anyhow`'s formatted chain.

Categories distinguish invalid input, missing resources, conflicts, size limits,
unavailable capabilities, rate limits, upstream failures, unavailable service,
timeouts, parsing, storage, and internal failures. Categories are not a second
HTTP policy. For example, `StorageFault` is storage intent but returns 500
`internal_error`. `SourceHttpStatus(u16)` retains the actual upstream status but
returns 502 `upstream_error`, not the upstream status as this service's status.
Fixed messages for supplied source 403, 404, and 429 name that source status.
Other supplied statuses currently use the generic unsuccessful-status message,
while their actual integer remains in the typed internal error.

## Stable common codes

These codes retain their prior known HTTP status unless marked as new. Several
validation kinds share `invalid_request` but have operation-specific fixed
messages. Use the status and code for client control, not the message.

| Status | Codes |
|---|---|
| 400 | `invalid_request` |
| 404 | `not_found` |
| 409 | `crawl_not_resumable`, `media_resume_unsupported` |
| 413 | `size_limit` |
| 422 | `capability_unavailable`, `read_content_blocked`, `read_content_unavailable`, `encoding_unsupported`, `encoding_invalid` |
| 502 | `upstream_error` |
| 503 | `queue_full` |
| 504 | `timeout` |

New typed reader failures return 422 `parse_invalid` for invalid supplied text
or structure and 422 `parse_failed` for an extraction failure or unusable result.
They replace previously unclassified reader failures that returned 500. Original
bytes remain retained before parsing. This does not make a failed parse a saved
document or add an original-artifact endpoint for failed requests.

New crawl-only kinds are 422 `crawl_policy_refused`, 422
`crawl_metadata_unsupported`, and 502 `crawl_result_rejected`. They preserve the
existing refusal policies. New persisted crawl failure text is fixed and safe.
Job states, warning codes, saved attachments, charging, and cancellation are not
changed. New automatic-recovery failure warnings and metadata also use fixed
error text, not private cause chains. Existing historical diagnostics are not
rewritten.

Unknown faults and storage failures return 500 `internal_error`. Database
corruption or an absent object file is not a missing saved resource. Only an
explicit absent document, library, or job selects 404 `not_found`.

## Stable service codes

| Service | Status and codes |
|---|---|
| Archive | 400 `archive_invalid_request`; 404 `archive_unavailable`; 413 `archive_size_limit`; 422 `archive_content_unavailable`, `archive_format_unsupported`; 429 `archive_rate_limited`; 502 `archive_redirect_refused`, `archive_identity_mismatch`, `archive_upstream_failed`; 504 `archive_timeout` |
| Native arXiv | 400 `arxiv_invalid_identifier`; 404 `arxiv_unavailable`; 422 `arxiv_reuse_not_established`, `citation_unavailable`; 429 `arxiv_rate_limited`; 502 `arxiv_identity_mismatch`, `arxiv_metadata_failed`, `arxiv_full_text_failed` |
| Native GitHub | 404 `github_reference_not_found`, `github_repository_unavailable`, `github_path_not_found`; 422 `github_reference_ambiguous`, `github_resolution_limit`, `github_unsupported_object`, `github_content_unavailable`; 429 `github_rate_limited`; 502 `github_access_denied`, `github_listing_incomplete`, `github_api_error` |
| Browser | 413 `browser_size_limit`; 422 `browser_helper_missing`, `browser_empty_output`; 502 `browser_navigation_failed`, `browser_invalid_output`; 504 `browser_timeout` |
| Video and captions | 400 `video_invalid_query`, `video_invalid_limit`; 413 `media_size_limit`; 422 `media_helper_missing`, `media_captions_unavailable`, `media_captions_malformed`; 429 `media_rate_limited`; 502 `media_identity_mismatch`, `media_source_blocked`, `media_helper_failed`; 504 `media_helper_timeout` |
| Media jobs | 400 `media_download_invalid`; 404 `media_artifact_not_found`; 409 `media_resume_unsupported`; 413 `media_budget_exceeded`; 422 `media_download_disabled`, `media_download_helpers_missing`, `media_format_unavailable`; 502 `media_validation_failed`; 504 `media_deadline` |
| Code and docs.rs | 400 `code_invalid_request`; 404 `code_source_unavailable`; 413 `code_limit`; 422 `code_search_unavailable`, `code_unsupported`; 429 `code_rate_limited`; 502 `code_access_denied`, `code_identity_mismatch`, `code_upstream_error`; 504 `code_timeout` |
| Scholarly | 400 `scholarly_invalid_request`; 404 `scholarly_not_found`; 413 `scholarly_size_limit`; 429 `scholarly_rate_limited`; 502 `scholarly_access_denied`, `scholarly_provider_failed`, `scholarly_invalid_metadata`; 504 `scholarly_timeout` |
| PMC | 400 `pmc_invalid_request`; 404 `pmc_version_unavailable`, `pmc_unavailable`; 409 `pmc_source_changed`; 413 `pmc_size_limit`; 422 `pmc_identity_incomplete`, `pmc_reuse_not_established`, `pmc_body_unavailable`; 502 `pmc_identity_mismatch`, `pmc_invalid_metadata`, `pmc_invalid_xml`. See [PMC.md](PMC.md) for source policies. |
| External indexes | 400 `external_invalid_request`; 404 `external_unavailable`; 413 `external_limit`; 422 `external_provider_unconfigured`, `external_unsupported`; 429 `external_rate_limited`; 502 `external_access_denied`, `external_redirect_refused`, `external_identity_mismatch`, `external_upstream_error`, `external_credential_response_refused`; 504 `external_timeout` |

`MediaOutputLimit` and `MediaBudgetExceeded` remain different internal kinds but
share 413 `media_budget_exceeded`. `MediaStagingUnsafe` and
`MediaValidationFailed` share 502 `media_validation_failed`. Job-safe diagnostics
can distinguish these kinds without scanning prefixes. `MediaCancelled` is an
internal typed worker signal with 409 `media_cancelled`; cancellation still uses
the existing job state and endpoint.

## Remaining boundaries

The following paths can still produce untyped internal errors. If they reach
HTTP or batch, they return 500 `internal_error`, without cause text:

- Semaphore closure, blocking-task join failure or panic, deadline overflow,
  and serialization of generated identities or metadata.
- Per-request invalid operator `document_config`, optional fastCRW renderer
  configuration or construction, and Chromium profile-directory creation.
- Video/caption temporary-directory or file I/O, and some media staging,
  process-supervision, and filesystem operations not given a more specific kind.
- Unexpected internal invariant failures, including unavailable helper pipes.

Configuration load/validation and client construction also retain internal
`anyhow` errors. They normally occur before the service starts, not as HTTP
responses. Storage operations wrap SQLite, filesystem, checksum, and stored
payload faults as `StorageFault`. No string-based compatibility adapter remains
at HTTP or batch.

Search-provider failures remain explicit per-provider warnings in a successful
`SearchResponse`. A provider failure must not hide another provider's results.
This migration does not turn those partial outcomes into HTTP errors or redact
all successful-operation metadata and warning text.

Native helpers do not always supply structured failures. `HelperFailure`
distinguishes missing executable, I/O, deadline, output limit, and unsuccessful
exit before browser/media mapping. Lightpanda's native timeout tag and yt-dlp's
native rate/access diagnostics still require bounded protocol-specific text
inspection. This is a helper-protocol limit, not engine error-message matching
or a fallback/retry policy change.

Some existing enum-only code and scholarly helpers discard low-level causes
when they select their domain enum. Their public classification is typed, but
this migration does not claim complete diagnostic preservation in those legacy
helpers. Newly migrated common fetch, parser, archive, storage, and helper paths
retain their existing underlying causes.
