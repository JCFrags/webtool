# Third-party materials and publication blockers

These are local alpha candidates, not cleared distribution artifacts. The original
project LICENSE notice is unchanged. COPYING supplies complete AGPL v3 terms.
This inventory is not legal clearance.

The packager copies license, copyright and notice files from packages observed in
the native Cargo build. THIRD-PARTY.json records locked registry archive checksums,
source URLs, declared license expressions and copied notice paths. The inventory
includes build-only dependencies, not just code linked into each binary. Rust
standard-library notices are included when available from the installed toolchain.
System shared libraries are identified in BUILD-INFO.json but are not bundled.

packaging/licenses/index.json records recovered supplements and their exact source
URLs or local system origins. Recovered xberg and html-to-markdown attribution
files, Typst NOTICE, and nested hashify LICENSES files must not be omitted merely
because their names differ from LICENSE. Supplemental status describes recovered
text, not approval of a package's complete provenance.

## Bounded remediation status

`packaging/licenses/concerns.json` is the machine-readable review record. Its
records distinguish shipped code, build-time procedural macros, optional modules
and unbundled libraries. The bounded records currently have no unresolved notice
or license-term obligation. This result is not a broad ecosystem audit or legal
clearance.

- mac 0.1.1, mathemascii 0.4.0, simd_helpers 0.1.0 and fxhash 0.2.1 now have
  complete applicable terms and the author or copyright evidence supplied by
  their published source or generic upstream. A missing root filename or a
  non-revision-exact copy of standard terms is not by itself an incompatibility.
- servo_arc 0.3.0 retains its exact published-source copyright/dual-license header
  with complete generic MIT and Apache-2.0 terms.
- profiling-procmacros 1.0.18 is a build-time procedural macro enabled through
  profiling's default feature. It records the same upstream revision and uses the
  same recovered root license files as profiling 1.0.18.
- metadata-search-engine-rs 0.3.1 is compiled in the normal host build through
  webtool-engine's default `web-search` feature. Its image adapters are present in
  that crate but are not exposed by webtool's ordinary web-search interface. Its
  Sogou and Bing image adapters, its historical-internal-API Google image adapter,
  its image model and its image identity logic contain SearXNG-derived portions.
  `SEARXNG-ATTRIBUTION.md` lists the exact affected files and evidence. Those
  portions are treated as AGPL-3.0-or-later despite the crate manifest's MIT-only
  declaration, and complete AGPL terms are included. Complete generic MIT terms
  and the supplied author/license statement cover the remainder without an
  invented copyright statement. Pinned comparison hashes do not claim the
  unknown original port revisions.
- zune-inflate 0.2.54 directly derives decode-table constants, table-building
  logic and CRC data from libdeflate. `LIBDEFLATE-ATTRIBUTION.md` lists the five
  affected files and evidence. libdeflate's applicable MIT copyright and
  permission notice are included. Pinned comparison hashes do not claim the
  unknown original adaptation revision. MIT is one of zune-inflate's declared
  alternatives.
- selectors 0.25.0 and 0.33.0 use MPL-2.0. Complete terms are included.
  `docs/SOURCE-ACCESS.md` defines generated exact registry archive URLs and
  checksums, recipient access, notice retention and availability conditions for
  their Covered Source. Missing source directions are therefore remedied for this
  candidate, subject to final generation and verification at the distribution
  point.
- xberg 1.1.1 records a dirty crate snapshot. The checksum-pinned registry archive
  remains the build-source authority. The dirty marker and the absence of byte
  identity between a Git tree and published crate are not a license conflict.
  Recovered upstream attribution documents remain included.

`docs/SOURCE-ACCESS.md` defines the exact project and locked dependency source
set, no-Git build directions, and the applicable AGPL sections 6(d) and 13 and
MPL section 3.2 access duties. Raw dependency archives remain separately hosted
because bundling them could include excluded test-only models and other payloads.
The distributor must generate and verify the promised URLs and checksums at the
actual distribution point. Modified network deployments must provide source for
the deployed version as described there.

PUBLICATION-BLOCKERS.txt is generated from unresolved concern records and actual
missing term files. Other release proof or publication blockers can still apply.
No Lightpanda, yt-dlp, Node, models or downloaded papers are shipped.
