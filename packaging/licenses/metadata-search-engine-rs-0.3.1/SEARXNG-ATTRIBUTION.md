# SearXNG-derived portions

The checksum-pinned `metadata-search-engine-rs` 0.3.1 crate declares MIT in its
Cargo manifest. Its changelog dates version 0.3.1 to 2026-08-07.

**Changed-file notice:** The five published-crate files listed below are Rust
adaptations that differ from SearXNG source. SearXNG's corresponding source files
use `SPDX-License-Identifier: AGPL-3.0-or-later`. Treat the adapted portions under
AGPL-3.0-or-later, not under the crate's MIT declaration. The accompanying
`LICENSE-AGPL-3.0` contains the complete AGPL version 3 terms.

Upstream project: <https://github.com/searxng/searxng>

Pinned upstream comparison sources retrieved on 2026-09-10:

- SearXNG revision `931fd9787b1517d88af2876175d8c31b03e11671`:
  - `searx/engines/sogou_images.py`, SHA-256
    `2e666571188bb65c67b6fef07791daba3fbb09e221d13793b7c0a13332157df2`
  - `searx/engines/bing_images.py`, SHA-256
    `3af250dc08bfbd27d9823f70c25298451e05ee1d9e53d8ea1fd705638dc1480b`
  - `searx/result_types/image.py`, SHA-256
    `9c9661ddb6c7f88c8ca28369d2bf9a4878855bc50acd55f7efa7808f69a54a20`
  - `searx/result_types/_base.py`, SHA-256
    `323a6aaf4811014fbf0fec14a90238db46acaf6836fa7ebbbbc3b8fbf71f02c0`
- Historical SearXNG revision `13eec44b65d19dedddfd85755ed2adab2a22187c`:
  - `searx/engines/google_images.py`, SHA-256
    `b9158479982c31383718fc9c764fab9f4a42901409913791336f2dc2ba34e382`

These are the observed comparison snapshots, not claims about the unknown
SearXNG revision from which the crate publisher originally ported each file.
The crate does not identify those original upstream revisions.

Affected published-crate files:

- `src/engines/sogou_images.rs`: request construction, embedded
  `window.__INITIAL_STATE__` extraction, field mapping, and result construction
  are a Rust adaptation of SearXNG's Sogou Images engine. The crate changelog
  expressly calls this file "ported from SearXNG's `sogou_images.py`."
- `src/engines/bing_images.rs`: `/images/async` request parameters, result
  selectors, inline `m` JSON fields, and image-result mapping follow SearXNG's
  Bing Images engine.
- `src/engines/google_images.rs`: Google Go Android user agent, unescaped
  `async=_fmt:json` request, `ischj.metadata` parsing, image field mapping, and
  author handling follow SearXNG's historical internal-JSON Google Images
  engine.
- `src/models.rs`: the `ImageResult` fields and documented meanings mirror the
  SearXNG image-result type and legacy image result dictionaries.
- `src/aggregator.rs`: image identity uses normalized page URL plus `img_src`,
  expressly mirroring SearXNG's `template|url|img_src` result hash.

The Rust implementations include modifications. Distribution of these portions
must therefore preserve the SearXNG notices, identify the changed files, apply
AGPL-3.0-or-later to the adapted portions, and provide the applicable source as
required by the AGPL. The remaining crate files retain their published MIT
terms unless another notice states otherwise.
