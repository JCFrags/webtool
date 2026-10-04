# Linux installation and operation

Webtool is a client and one shared server. Ordinary research does not need a model.
The server has no authentication. Every connected user can read every library.
Keep it on loopback. Do not publish it as an internet service.

## Support target and prerequisites

Sprint 1 targets Linux x86_64 only. The source build is exercised on Fedora 44.
The OCI build uses Debian 12 (Bookworm) amd64. See [Docker operation](DOCKER.md)
for the contained runtime. Neither result establishes other distributions or
architectures. macOS, Windows, ARM, Alpine/musl and static binaries are not
supported release targets in this sprint.

A native binary archive is tied to its builder. Check `BUILD-INFO.json` before
installation. It lists the loader, needed shared libraries and required glibc,
OpenSSL and GCC symbol versions for **each** binary. A Fedora-built host binary
can require glibc 2.43. Do not treat it as a generic Linux download. Prefer the
Bookworm-built candidate or OCI path for the documented baseline. The minimum
symbol version is a requirement, not proof that every newer distribution works.
The OS must provide its certificate trust store and the reported shared libraries.

A source installation needs Bash, GNU coreutils, a current stable Rust/Cargo,
a C/C++ compiler and linker, pkg-config, CMake, OpenSSL 3 development headers,
and certificate trust. The locked Xberg dependency needs the OpenSSL headers even
though the main HTTP client uses rustls. SQLite is compiled from the locked crate.
No database service is needed. The packager also needs Python 3.11+, `file` and
`readelf`. Python is not an application runtime dependency.

Typical development packages, not a claim of distribution-wide testing:

```sh
# Debian 12 build environment:
sudo apt-get install build-essential pkg-config clang cmake libssl-dev ca-certificates
# Fedora 44 build environment:
sudo dnf install gcc gcc-c++ pkgconf-pkg-config clang cmake openssl-devel ca-certificates
```

Install a stable Rust toolchain through your normal trusted package or rustup
method. The installer does not change system packages or install Rust.

## Install verified binary archives

1. Obtain the client, host and source archives plus `SHA256SUMS` from the same
   candidate/release. A checksum detects corruption only when its manifest comes
   from a trusted source. Do not mix artifacts from different revisions.
2. Run `sha256sum -c SHA256SUMS` beside the archives. Inspect `BUILD-INFO.json`,
   `SOURCE-ACCESS.md`, `THIRD-PARTY.md` and `PUBLICATION-BLOCKERS.txt` after extraction.
3. Extract into fresh directories. Do not overwrite an earlier candidate.
4. Run each required archive's installer as an ordinary user:

```sh
# Inside the extracted client directory:
./install.sh --bin-dir "$HOME/.local/bin"
# Inside the extracted host directory, if this machine hosts the server:
./install.sh --bin-dir "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH" # current shell only
webtool --version
webtoold --version
```

The archive installer checks `BINARY-SHA256SUMS` and needs no Cargo. Retain the
extracted directories and their notices. It installs only the binary present in
that archive and its sibling ownership receipt. It never starts a server, installs
helpers, changes profiles or creates a system service.

For a clean source checkout:

```sh
./scripts/install-local.sh --client-only # client machine
./scripts/install-local.sh               # host machine, both binaries
```

The source installer runs a locked, normal-feature release build in the checkout's
explicit `target` directory. `CARGO_TARGET_DIR` alone does not redirect it. Set
`CARGO_BUILD_JOBS=2` if memory is limited. From an unpacked source archive, use the
no-Git build command in `SOURCE-ACCESS.md`, then copy the binaries to a new
user-owned directory or create the archive checksum manifest before using
`--from-archive`. Do not invent a Git build commit for an unpacked source build.

Both install modes use `.webtool.install-sha256` and `.webtoold.install-sha256`.
They refuse unknown executables, symlinks, or binaries changed since their previous
receipt. Use another `--bin-dir` if a name is occupied. Never delete a receipt to
force replacement. Binary replacement does not restart an existing process.

## Start a new host

Use one explicit configuration file and one data directory, outside the checkout.
The server does not initialize client settings or install optional helpers.

```sh
config_dir="$HOME/.config/webtool"
data_dir="$HOME/.local/share/webtool"
mkdir -p "$config_dir" "$data_dir"
chmod 700 "$config_dir" "$data_dir"
# From the source checkout or extracted host archive. Refuse an existing config:
test ! -e "$config_dir/server.toml" && cp config.example.toml "$config_dir/server.toml"
chmod 600 "$config_dir/server.toml"
# Inspect/edit server.toml. Keep bind = "127.0.0.1:8420".
cd /tmp
"$HOME/.local/bin/webtoold" \
  --config "$config_dir/server.toml" --data-dir "$data_dir"
```

Run the server in a terminal first. Use another terminal for the client:

```sh
webtool connect http://127.0.0.1:8420
webtool config show
webtool --timeout 5 doctor
webtool library create research
printf 'First retained source.\n' > /tmp/webtool-example.txt
webtool ingest /tmp/webtool-example.txt --library research
webtool library items research
# Use the saved ID from ingest, without fetching the source again:
webtool read DOCUMENT_ID
webtool export DOCUMENT_ID --kind original --output /tmp/webtool-original.txt
```

Client settings use `$XDG_CONFIG_HOME/webtool/client.toml` or
`$HOME/.config/webtool/client.toml`. `connect` saves the endpoint without contacting
it. Precedence is `--server`, `WEBTOOL_SERVER`, saved setting, then loopback.
Use `--server` and an isolated HOME/config directory for diagnostic scenarios.
Clients upload files over HTTP. They never open the server database.

Keep one server per data directory. Config, data and helper paths should be
absolute. Relative paths are resolved from the working directory, not the config
file. The server retains `webtool.sqlite3`, its SQLite sidecars and `objects/`
beside it. Keep the entire data directory together. Helpers belong to the server
operator and are optional. A client or container image does not supply browser,
caption, OCR or model readiness. Media downloads stay disabled by default.

An explicit trusted-network deployment is a separate operator decision. It needs
access restrictions and the deployed version's source-access directions. A
contributor name is attribution, not a login. Do not change the loopback examples
to `0.0.0.0` just to connect another machine. No service manager is required here.

## Upgrade, backup and rollback

1. Record `webtool doctor`, the endpoint, build revision and binary hashes. Check
   jobs. Wait for or cancel only work you own before a planned stop.
2. Stop the correct server gracefully with Ctrl-C in its terminal. If another
   launch method owns it, verify its executable, arguments and process start time
   before signaling it. Confirm it exited and its listener closed.
3. While the server is stopped, copy the **entire** data directory, config, old
   binaries and both install receipts into a new backup directory. Keep originals
   with SQLite. A bare copy of a live SQLite file is not a consistent backup.
4. Validate the stopped database or its backup with SQLite `PRAGMA quick_check`.
   Keep the backup unchanged before starting a new binary over that data.
5. Install the verified replacement through the same ownership-receipt installer.
   Start it with the same absolute config/data arguments. Run `doctor`, a saved
   read and an original export. Check existing libraries and jobs.

Example stopped-server backup, with a new destination:

```sh
backup="$HOME/webtool-backup-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -m 700 "$backup"
cp -a "$HOME/.local/share/webtool" "$backup/data"
cp -a "$HOME/.config/webtool/server.toml" "$backup/server.toml"
cp -a "$HOME/.local/bin/webtool" "$HOME/.local/bin/webtoold" \
  "$HOME/.local/bin/.webtool.install-sha256" \
  "$HOME/.local/bin/.webtoold.install-sha256" "$backup/"
# If sqlite3 is installed:
sqlite3 "$backup/data/webtool.sqlite3" 'PRAGMA quick_check; PRAGMA user_version;'
```

Keep the backup path with your launch command. Do not assume an older binary can
read a newer schema. Schema-1 binaries refuse schema 2. Rollback means stopping
the replacement and restoring the compatible database **and originals**, config,
old binary pair and matching receipts. Never lower SQLite `user_version` to evade
compatibility checks. Before rollback, preserve any new research separately.
Prefer a new restore directory so neither the backup nor later data is overwritten.
See [crawl storage](CRAWL.md) for explicit resume and migration limits.
