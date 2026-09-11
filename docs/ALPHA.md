# webtool 0.1.0-alpha.1 candidate

An early, usable text CLI and shared research server. This is not a production
service or a complete V1 release. Candidate archives are for local validation;
no release has been approved or published.

## Unpack and run

Use `sha256sum -c SHA256SUMS` beside the three archives before unpacking them.
Unpack each archive into a fresh directory with `tar -xzf ARCHIVE.tar.gz`.
Keep LICENSE, COPYING, THIRD-PARTY.md, the license directory and BUILD-INFO.json
with each binary. No install, service startup or shell-profile changes are automatic.

Client archive:

```sh
cd webtool-0.1.0-alpha.1-TARGET-client
./webtool --version
./webtool connect http://SERVER:8420
./webtool config show
./webtool doctor
./webtool library list
```

Replace TARGET with the target triple in the archive name and SERVER with the
trusted host address. The client requires no browser, document or media helpers.
`connect` only saves local settings. It does not require an online server.
Endpoint precedence is --server, WEBTOOL_SERVER, saved settings, then localhost.
Settings use XDG_CONFIG_HOME/webtool/client.toml or HOME/.config/webtool/client.toml.

Host archive, for a separately approved deployment only:

```sh
./webtoold --version
# Edit a copy of config.example.toml at an absolute path before startup.
# Select the intended data directory. Never start over an existing server.
./webtoold --config /absolute/path/server.toml --data-dir /absolute/path/data
```

Paths are working-directory-relative unless absolute. Existing deployments must
select their existing data directory explicitly. Configure optional helper paths
only on the host. Loopback is the default. Explicit `--bind 0.0.0.0:8420` permits
trusted-LAN binding but does not change firewalls. Every library is shared. There
is no authentication or TLS setup; do not expose this service to the public internet.

## Platform requirements

Only the current native Linux architecture is packaged. Consult BUILD-INFO.json
for the exact Rust toolchain, target, build commit, build platform, ELF interpreter,
needed shared-library names and required symbol versions for each binary.
These binaries are dynamically linked. Both candidate binaries need the reported
OpenSSL 3, glibc, loader and GCC runtime libraries.
Runtime shared-library packages and certificate trust must be supplied by the OS.
No static-linking, broad Linux portability or reproducible-binary claim is made.
No other distribution, architecture or physical-machine LAN connection was tested.

## Previously verified workflows

Built in: web search; source-preserving HTML/code/tables; native PDF text; pinned
GitHub files and immediate directories; crawl-to-library; version-pinned arXiv
papers and offline saved-paper citations; file uploads, shared-server clients and
original exports. Everyday CLI output was checked against saved sources.

Separately installed and configured helpers: YouTube captions use yt-dlp with a
supported JavaScript runtime; JavaScript page capture uses Lightpanda. No yt-dlp,
Node, Lightpanda, browsers, models or downloaded papers are bundled. Earlier
bounded checks passed with those helpers; this packaging step does not retest them.

Known limits: OCR and Office-format breadth remain unverified. Chromium and fastCRW
remain experimental/unverified. Complex mathematical/layout reconstruction is not
implemented. Browser readiness does not guarantee complete application content.
Search/caption/source providers may fail or block access. This is not an archive
of all GitHub files or a complete-site crawler. Independent local client configs
were tested, not connectivity from another physical machine.

## Source and licensing

The original AGPL notice is retained in LICENSE; complete AGPL v3 terms are in
COPYING. No warranty. Third-party licenses remain applicable. Read THIRD-PARTY.md,
THIRD-PARTY.json and PUBLICATION-BLOCKERS.txt before any distribution.

The source archive contains an allowlisted exact project Git snapshot, Cargo.lock,
build/install/package scripts and license materials. Dependency source URLs and
checksums are recorded in THIRD-PARTY.json, including build-only dependencies.
Dependency source archives are not bundled: some contain test-only model payloads
or other materials outside this package scope. Complete corresponding-source
availability remains a publication condition, not an implied complete source offer.
The archive omits private working state and development history documents.
BUILD-INFO.json identifies the actual build checkpoint; later documentation-only
commits need not change the binaries.

To build the project sources, use the recorded Rust toolchain, a C toolchain and
required native development libraries, then `cargo build --locked --release
-p webtool-cli -p webtool-server`. Cargo normally obtains locked dependencies from
the registry at the URLs/checksums in THIRD-PARTY.json. No offline restoration
or second source-build proof was run. A source checkout without Git reports an
unknown server build commit rather than inventing one.

Before distribution, resolve recorded notice gaps and provide recipients with the
matching source archive alongside the binaries. Remote users of a modified AGPL
server must be offered its corresponding source as required by section 13. The
candidate manifest and repository link are not a waiver of those obligations.
