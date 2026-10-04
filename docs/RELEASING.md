# Linux candidate assembly

This workflow produces private artifacts for practical acceptance. It does not
approve a release, push a branch, publish an image, replace the published
`v0.1.0-alpha.1` assets, or satisfy the Sprint 1 parity gate by itself.
See [the sprint gates](SPRINTS.md). Keep a new candidate directory for each revision.

## Choose the actual build target

The native packager supports the current Linux Rust host only. It does not cross
compile. Its Fedora 44 host output can require glibc 2.43 and is **not** the generic
Linux artifact. Prefer the source-built Debian 12 amd64 OCI output for the Sprint 1
baseline. Record the actual Rust version, builder and runtime image digests,
platform, normal feature set, ELF dependencies and glibc/OpenSSL/GCC symbols.
Do not relabel a host binary as a Bookworm build.

Use a clean, committed checkout and an isolated output directory. Untracked files
are not swept into archives. Build inputs and source archives use the explicit
allowlist in `scripts/package.py`. It includes `migrations/`, which is required to
compile storage. It excludes local data, configs, secrets, runtime evidence,
helpers, models, Cargo caches, historical development records and `.git`.

## Native candidate

```sh
CARGO_BUILD_JOBS=2 ./scripts/package-local.sh --out-dir /absolute/private/candidate
```

This normal locked/offline release build uses the checkout's explicit target
folder. It remaps source paths, checks cached registry archives against
Cargo.lock, and compares extracted build inputs to those archives. Populate an
ordinary trusted Cargo cache first if dependencies are missing. Do not change the
lockfile or install optional helpers to make packaging pass.

When worktrees share a Cargo target, hold the external build lock, refresh only
the current worktree's existing crate roots/build scripts, and retain private
copies before releasing it. See project instructions. The lock alone does not
make workspace cache artifacts source-exact.

## Reuse one source-built OCI build

Avoid a second full release build. The Dockerfile's `build` stage writes
`/out/BUILD-EVIDENCE.json`, both binaries, compiler artifacts, metadata and generated
notices. The receipt hashes source files and exported material. A no-Git context
build reports an unknown compiler commit; the receipt identifies its actual inputs.

```sh
revision="$(git rev-parse HEAD)"
podman build --target build --build-arg SOURCE_REVISION="$revision" \
  -t webtool-build:private .
# Copy from a created, never-started container. Use task-owned names and remove
# only the recorded container after its copy completes.
podman create --name webtool-build-copy --entrypoint /bin/true webtool-build:private
podman cp webtool-build-copy:/out /absolute/private/build-evidence
podman rm webtool-build-copy
./scripts/package-local.sh --reuse-build /absolute/private/build-evidence \
  --out-dir /absolute/private/candidate
# Finish the runtime image from the same cached source build, not another build tree:
podman build --build-arg SOURCE_REVISION="$revision" -t webtool:private .
```

Docker accepts the corresponding `build`, `create`, `cp` and `rm` commands. Its
Engine must actually be exercised before claiming Docker verification. These
steps use no host Cargo mount. Never mount a host build tree and call it a clean
container build. Build dependencies and base images may be downloaded normally.

`--reuse-build` runs no Cargo command. It refuses a different source commit,
changed allowlisted source file, changed binary or changed exported build file.
It uses the **recorded builder** toolchain and linkage, not the host's glibc.
Retain the full private receipt directory. Do not hand-edit a receipt to bless an
unrelated binary. An image with `SOURCE_REVISION=unknown` cannot become a
source-exact candidate through this path.

## Inspect and exercise

The three archives are client, host and source. Each binary archive includes the
same ownership-receipt installer, `BINARY-SHA256SUMS`, build information, terms,
third-party inventory and source directions. Run `sha256sum -c SHA256SUMS`.
Inspect exact filenames, tar members, generated placeholders, outgoing paths and
metadata before any online publication. Use only public project identity.

Use a fresh HOME/bin/config/data directory outside the checkout, not an existing
workstation installation. Install the extracted client and host without Cargo.
Start a private loopback server on an unused port. Exercise `connect`, `doctor`,
file upload, shared-library use, saved read and byte-identical original export.
Restart once with the same storage, and check the saved ID again. Preserve a
consistent stopped-server data backup. Exercise the source-built OCI image with
its nonroot user, read-only root, persistent volume and host-loopback publication.

All temporary checks need an enforced finite lifetime, absolute work/close
deadlines, a close reserve, ownership records, bounded calls and verified
shutdown/reap. `scripts/smoke_cli.py` provides the bounded native CLI/server check.
Do not run a diagnostic on an installed service's port or data. Do not replace an
expired scenario process just to finish the same check.

Source compilation from an unpacked archive needs no Git. It must include every
required migration and crate input. A metadata/build check is useful but is not a
claim of reproducible binary identity or full offline restoration. The archive's
BUILD-INFO.json continues to identify the supplied snapshot. A newly compiled
no-Git binary correctly reports its compiler commit as unknown.

## Licensing and publication gate

`THIRD-PARTY.json` follows compiler-artifact inventory, including build-only
packages. It is not a per-binary linkage map. `LOCKED-SOURCES.json` additionally
lists optional/other-platform registry entries. Collect source headers,
attribution and nested notice material, not only files named LICENSE.
Preserve the existing supplement and concern records. Do not bundle raw test-only
models or treat a dirty upstream Git snapshot as the locked crate's authority.

`PUBLICATION-BLOCKERS.txt` reports unresolved material gaps. Zero gaps is not legal
clearance or product acceptance. Before actual distribution, verify all promised
project/dependency downloads and their checksums at the distribution point.
Generated source URLs for a private unpushed commit are **not yet publicly
available**. Serve corresponding source alongside binaries or publish accepted
source through the approved gate first. A local private source archive does not
create a public AGPL offer. Modified network service users need prominent access
to the deployed version's corresponding source. MPL Covered Source must remain
available under its terms. See [SOURCE-ACCESS.md](SOURCE-ACCESS.md).

For OCI distribution, also retain and meet base-image and OS-package source/license
conditions. Their materials are separate from the Rust compiler inventory. A
runtime image ID and `COPYING` alone do not establish those conditions.

Report separately: private artifact checksums/source identity, clean-user Linux
use, actual OCI use, Docker Engine/architecture limits, remote integration, and
installed activation. This sprint's private assembly does not authorize the last
two stages. Worker completion is not parent or Ketch-parity acceptance.
