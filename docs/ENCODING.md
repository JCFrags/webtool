# HTML character decoding

HTML uses `html-encoding/1` with `encoding_rs 0.8.41`. The service saves the exact
response or upload bytes before decoding. One decoded view supplies content-gate
inspection, main-content selection, source matching, and link discovery. Ordinary
reads, explicit CSS reads, uploads, HTTP crawl reads, and HTML page maps use this
path. No extractor or browser fallback is added.

## Selection rules

For `text/html`, choose the first available rule:

1. A UTF-8, UTF-16LE, or UTF-16BE byte order mark (BOM).
2. The first recognized HTTP `Content-Type` charset parameter.
3. The first recognized eligible HTML meta charset.
4. UTF-8 when the undeclared input is valid UTF-8.
5. Otherwise, windows-1252 with an explicit assumption warning.

HTTP charset parameters use the `mime` parser. HTML meta inspection uses the
existing HTML parser over at most the first 1024 raw bytes. It accepts `charset`
and `http-equiv="content-type"` with a charset-bearing `content` attribute. It
excludes template, noscript, and SVG descendants. Script text and comments are
not declarations. The prescan accepts ASCII-compatible markup only. It does not
inspect UTF-16 meta declarations or later declarations. Duplicate attributes use
the HTML parser's first-attribute behavior. Each declaration source is bounded to
16 entries, and retained labels to 80 characters. Media-type inputs over 8192
bytes are not parsed.

Encoding labels use WHATWG mappings. For example, HTML `iso-8859-1` means
windows-1252. Meta UTF-16 labels become UTF-8, and meta `x-user-defined` becomes
windows-1252, with a warning. A selected label for the replacement encoding fails
instead of falling back. UTF-32 is unsupported. Unknown or malformed declarations
produce warnings and do not select an encoding. Conflicting recognized
declarations produce warnings. These rules are a bounded policy, not full browser
encoding-sniffing conformance. There is no locale or statistical detector.

Captured browser DOM is already UTF-8. The service validates it as UTF-8 and never
reinterprets it using stale HTTP or meta declarations. Conflicts still produce
warnings. A malformed DOM capture fails rather than using replacement decoding.

`application/xhtml+xml` uses a separate, deliberately restricted policy: strict
UTF-8 only. An absent or UTF-8 transport charset, optional UTF-8 BOM, and absent or
UTF-8 XML declaration are supported. Other declarations, non-UTF-8 bytes, or an
incomplete/oversized XML declaration fail explicitly. HTML meta does not select
XHTML encoding. This does not add general XML decoding. JSON and other text formats
retain their existing rules.

## Limits and provenance

The configured `max_bytes` bounds both the input and decoded UTF-8 bytes. Expansion
can therefore reject a response that fits the input limit. Incremental decoding
uses an 8192-byte scratch buffer, checks each output chunk before appending, and
never returns truncated content. Standalone reader functions use a 25 MiB limit.
NUL characters in decoded HTML fail explicitly as unsupported input.

Malformed sequences in declared HTML encodings become U+FFFD with a warning and
`had_errors: true`. All resulting block locations become derived. Such an input
cannot trigger automatic browser recovery. Gate detection still uses its decoded
view. Lossless decoding retains eligible HTML selectors, but those selectors
address elements in the decoded retained artifact, not raw-byte offsets.

`metadata.html_encoding` records the policy version, selected encoding and reason,
BOM, inspected declarations, input/output sizes, output limit, replacement flag,
and decoded UTF-8 SHA-256. Original hashes and original exports still describe the
unaltered bytes. Saved CSS extraction replays the recorded encoding and checks the
decoded hash, without guessing from meta or refetching headers. Legacy saved
snapshots without encoding metadata retain their strict UTF-8 CSS view.

New reads use versioned parser/cache identities. HTML snapshot identity includes
the decoding decision. Existing stored snapshots and their IDs are not rewritten.
When browser recovery succeeds, the initial HTTP artifact and encoding metadata
remain separate from the accepted DOM artifact. HTML maps include the original
artifact, decoding metadata, and warnings in their result.

The decoder does not establish source truth, fix incorrect declarations, or prove
extraction quality. A recognized but incorrect source charset can decode without
errors. Inspect the retained original when fidelity matters.

## Implementation references

- [encoding_rs 0.8.41 Encoding API](https://docs.rs/encoding_rs/0.8.41/encoding_rs/struct.Encoding.html): explicit `for_label`, `for_bom`, and decoder construction.
- [encoding_rs 0.8.41 Decoder API](https://docs.rs/encoding_rs/0.8.41/encoding_rs/struct.Decoder.html): caller-sized incremental UTF-8 output and replacement reporting.
