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

## Unresolved before publication

- metadata-search-engine-rs 0.3.1 declares MIT, but its changelog says the Sogou
  engine was ported from SearXNG. The copied/adapted scope, upstream revision and
  applicable attribution/source obligations are not identified. Resolve this with
  the upstream author rather than assuming the MIT metadata overrides lineage.
- mac 0.1.1, mathemascii 0.4.0, simd_helpers 0.1.0,
  metadata-search-engine-rs 0.3.1 and fxhash 0.2.1 lack recovered revision-exact
  root license materials. Metadata alone does not replace missing notices.
  fxhash retains a Rust Project copyright/dual-license header in its source.
- selectors 0.25.0 and 0.33.0 use MPL-2.0. Complete terms are included, but
  recipients still need a final, tested notice and location for corresponding
  Covered Source under MPL section 3.2. servo_arc 0.3.0 has generic dual-license
  terms and its exact copyright header, not recovered revision-exact root files.
- xberg 1.1.1 records a dirty crate snapshot. Its recovered attribution documents
  match its recorded Git revision, not necessarily every byte of the published
  crate. The checksum-pinned registry crate remains the build-source authority.
- zune-inflate describes libdeflate lineage without identifying the exact revision
  and adapted scope. Recovered license texts do not resolve that provenance gap.
- The project source archive is not a complete dependency Corresponding Source
  bundle. Registry source URLs and checksums document retrieval locations, not a
  verified complete source offer. Raw dependency archives are not bundled because
  they can contain excluded test-only models and other payloads. Resolve source
  availability and any required notices before binary distribution. Modified AGPL
  server deployments must also meet applicable network-source requirements.

PUBLICATION-BLOCKERS.txt adds missing materials detected in the actual build.
Do not infer that all obligations are resolved when one listed file is recovered.
No Lightpanda, yt-dlp, Node, models or downloaded papers are shipped.
