# Source access for this alpha candidate

This notice accompanies both binaries and the project source archive. It is an
access description, not a promise to produce source later or legal clearance.
Check PUBLICATION-BLOCKERS.txt for unresolved items before distribution.

## Exact project and dependency sources

Project revision: @SOURCE_SHA@

- Browse/download the exact project: https://github.com/JCFrags/webtool/tree/@SOURCE_SHA@
- Git archive: https://github.com/JCFrags/webtool/archive/@SOURCE_SHA@.tar.gz
- The accompanying allowlisted project archive is @SOURCE_ARCHIVE@. Verify it
  against the accompanying SHA256SUMS. BUILD-INFO.json identifies the same revision.
- THIRD-PARTY.json identifies packages observed in the native build, their features,
  build roles, unmodified published-crate source URLs and Cargo.lock SHA256 values.
  LOCKED-SOURCES.json also lists all locked registry archives, including unused
  optional and other-platform entries. A lockfile is an index, not source code.
- Download the required source archives from their listed crates.io URLs without
  credentials or payment. Verify each file with `sha256sum` against its recorded
  checksum before extraction. The published crate bytes, not a possibly dirty
  upstream Git revision, are the build-source authority. Retain source notices.

Project sources plus the required dependency source archives and included build
scripts are the source-access set. Dependency archives are separately hosted, not
bundled here. No model, browser or media-helper payload is in these packages.
General-purpose unmodified build tools and unbundled OS libraries are separate.
Consult BUILD-INFO.json for the toolchain and actual dynamic library requirements.

## MPL Covered Source

The generated table below identifies every observed MPL package's exact archive
URL and SHA256, including selectors. Those unmodified source files remain under
MPL-2.0. Preserve their embedded copyright/license notices when redistributing.
Complete MPL terms are in licenses/supplements/selectors/MPL-2.0.txt. Recipients
may obtain Covered Source by the listed public downloads at no charge. MPL does
not restrict recipients' source rights to the terms used for this executable.

@MPL_SOURCES@

## Build an unpacked project source archive

Unpack outside any existing Git checkout. No .git directory or provenance-file
edit is required to compile:

```sh
cd webtool-0.1.0-alpha.1-source
cargo build --locked --release --target @TARGET@ -p webtool-cli -p webtool-server
./target/@TARGET@/release/webtool --version
./target/@TARGET@/release/webtoold --version
```

Use the Rust version in BUILD-INFO.json, Cargo, a C/C++ compiler/linker, pkg-config
and OpenSSL development headers/libraries. Cargo downloads locked dependencies
as needed. To use a prepopulated Cargo cache, add --offline. Default features
are required; do not add all-features. No helper or model download is needed.
Run the binaries directly, or copy the desired executable into a user-owned bin
directory without overwriting an unrelated file. Configure hosts as ALPHA notes
explain. The local packaging script requires a clean Git checkout, but compilation
and manual installation from this source archive do not. Without Git the server
reports its build commit as unknown; BUILD-INFO.json still identifies the supplied
source. Do not edit generated files to make a newly built binary claim this SHA.

## Distribution and network access conditions

For network binary distribution under AGPL section 6(d), put this notice and clear
source directions next to the binary downloads. Offer equivalent copying access
to the exact project and required dependency sources, without additional charge,
a password or special key. A different source host is allowed, but the distributor
remains responsible for availability for as long as the license requires it.
Recheck all required source downloads at the actual distribution point. If a host
no longer provides equivalent access, distribution cannot rely on that host.
These instructions are not a three-year written offer under section 6(b).

For a modified AGPL server that supports remote interaction, prominently offer
all remote users access to the corresponding source of the deployed version
under section 13. Give these directions through the service's user-facing access
instructions and keep the exact source available at no charge. This candidate's
notice does not automatically provide access for a different installed build or
for later modifications. A private local check does not establish a public offer.

MPL section 3.2 likewise requires reasonable, timely Covered Source access at no
more than distribution cost and preservation of source notices. No distribution
or public server deployment has been performed by this packaging operation.
