# Docker and OCI operation

Webtool's server has no authentication. Every connected user can read all libraries.
Do not publish it to the internet. The example publishes only host `127.0.0.1`.
Inside the container the process binds `0.0.0.0` so that this port mapping works.
That internal bind does **not** make an unrestricted host mapping safe.

## Support boundary

The support target is Linux amd64, a Debian 12 (Bookworm) runtime, and normal
server features. Rootless Podman 5.8.7 can exercise this OCI image on Fedora 44.
A Podman result is not Docker Engine verification. Docker Engine is not installed
in the local acceptance environment. The CI path uses Docker on a Linux runner.
Check the candidate's actual CI and practical receipts before claiming acceptance.
Other architectures, Docker Desktop, ARM emulation and non-Linux hosts are not
exercised targets.

For the Docker loopback boundary, use Engine 28 or newer with the ordinary NAT
bridge and no direct-routing override. Docker documents that older versions can
allow same-layer-2 hosts to reach localhost-published ports. Operator firewall or
network changes can also change reachability. Do not use host networking or
change `127.0.0.1:8420:8420` to `8420:8420`.
See [Docker's port-publishing rules](https://docs.docker.com/engine/network/port-publishing/).
Containers on the same bridge can reach the server. Do not attach untrusted
containers to that network.

## Build and start

Use a clean source checkout. No image or registry publication is automatic.
The Dockerfile pins the Rust compiler version, uses the committed lockfile and
two compiler jobs, and builds **both** `webtool` and `webtoold` from source.
It downloads ordinary build dependencies and base images. No host Cargo tree,
private data, browser, media helper or model is mounted or bundled.
`.dockerignore` is an allowlist of source and required packaging material.

```sh
# From the checkout. This value is source labeling, not manufactured Git provenance.
export WEBTOOL_SOURCE_REVISION="$(git rev-parse HEAD)"
docker compose build
docker compose up
```

`compose.yaml` retains data in the named `webtool-data` volume. Compose prefixes
that name with its project name. It mounts `config.example.toml` read-only at
`/etc/webtool/server.toml`. For a durable deployment, first copy the example to
your own config file, then replace the **config mount only** with its absolute
path. Keep it read-only and restrict host permissions. On SELinux hosts, use a
private `:Z` label only on this task-owned config file, not a whole home directory.
The command fixes `/data` as storage and the container-local bind as `0.0.0.0:8420`.
Those command-line values take precedence over TOML. Edit other settings in the
mounted file. No automatic service or restart policy is installed.

The runtime uses UID/GID 10001, a read-only root filesystem, a writable `/data`
volume and a bounded temporary `/tmp`. The initial named volume receives the
image's `/data` ownership. A host bind mount does not: it must be writable by the
container's mapped user. Prefer the named volume rather than a broad `chmod` or
recursive ownership change. Helpers, if separately configured later, need paths
and dependencies **inside** the container. A host path is not automatically visible.
Media downloads and optional models remain disabled/unconfigured.

The image includes the client. From another terminal:

```sh
docker compose exec webtool webtool --server http://127.0.0.1:8420 --timeout 5 doctor
printf 'First container source.\n' | docker compose exec -T webtool \
  webtool --server http://127.0.0.1:8420 ingest - --name example.txt
# Or connect a verified host client to the loopback port:
webtool --server http://127.0.0.1:8420 --timeout 5 doctor
```

Readiness through `doctor` is necessary but not a content check. Read the saved ID,
export the original, compare the bytes, and restart with the same volume to confirm
persistence. Configured-helper status is not a live provider test.

The Docker build context deliberately excludes `.git`. `doctor` therefore reports
an unknown compiler commit for a context build. The image revision label,
`/usr/share/doc/webtool/BUILD-INFO.json`, source-file hashes and retained source
archive identify the transferred inputs. Verify the label and hashes against the
clean checkout. Do not edit a provenance file to make a no-Git build claim a commit.
If no source revision was supplied, the image is labeled `unknown` and is suitable
only for local development, not a source-exact candidate.

## Minimal rootless Podman path

This is an alternative OCI invocation, not an instruction to install Docker.
Choose an unused host port and task-owned names for diagnostics.

```sh
podman build --build-arg SOURCE_REVISION="$(git rev-parse HEAD)" -t webtool:local .
podman volume create webtool-data
podman run --name webtool --read-only --cap-drop all \
  --security-opt no-new-privileges \
  --tmpfs /tmp:rw,nosuid,nodev,size=256m,mode=1777 \
  -p 127.0.0.1:8420:8420 -v webtool-data:/data \
  webtool:local
# In a second terminal:
podman exec webtool webtool --server http://127.0.0.1:8420 --timeout 5 doctor
```

The image's default config is sufficient for this first use. To configure it,
mount a dedicated config file at `/etc/webtool/server.toml:ro,Z`. Podman named
volumes handle its rootless user mapping. Do not use `:U` on existing research or
host directories as a permission shortcut. Ctrl-C or `podman stop --time 30 webtool`
stops only this container. A restart keeps its data volume.

## Backup, upgrade and rollback

1. Record the image ID/digest, source revision, `doctor`, config path and exact
   volume name. Use `docker compose config` and `docker inspect` to identify them.
2. Finish or explicitly cancel your active jobs. Stop the server with
   `docker compose stop -t 30`. Confirm its process stopped before copying data.
3. Copy the entire stopped volume through its container. Keep a new backup directory:

```sh
backup="$HOME/webtool-container-backup-$(date -u +%Y%m%dT%H%M%SZ)"
mkdir -m 700 "$backup"
container="$(docker compose ps -aq webtool)"
test -n "$container"
docker cp "$container:/data" "$backup/data"
cp config.example.toml "$backup/server.toml" # use your actual mounted config
cp compose.yaml "$backup/compose.yaml"
docker image inspect webtool:local > "$backup/image.json"
# If sqlite3 is installed:
sqlite3 "$backup/data/webtool.sqlite3" 'PRAGMA quick_check; PRAGMA user_version;'
```

`docker cp` after a stop keeps SQLite and `objects/` together. Do not copy only a
live database file. Keep the old image ID or a task-owned tag and its source-access
materials. A tag alone can later point at a different image.

4. Build/load the verified replacement, then use `docker compose up` with the
   same config and volume. Check `doctor`, old library items, a saved read and an
   exact original export. Do not run two servers against the same volume.
5. For rollback, stop the replacement and preserve later research separately.
   Restore the compatible full backup into a **new** volume, with UID/GID 10001
   inside the container, and use the old exact image and config. Do not lower
   SQLite `user_version`. Schema-1 software refuses schema 2. An old image is not
   sufficient when storage has migrated.

`docker compose down` removes containers/networks but keeps named data volumes.
Do not add `--volumes`, run volume pruning, or delete your backup during an upgrade.
For Podman, the same stopped-container `podman cp` sequence works. Keep the
rootless mapped ownership consistent when restoring a volume.

## Source and distribution

The image carries project terms, observed-build third-party notices, exact locked
source indexes, generated `SOURCE-ACCESS.md` and an allowlisted source archive in
`/usr/share/doc/webtool`. It does not bundle raw dependency test-model payloads.
The downloaded OS packages retain their own `/usr/share/doc` license materials.
Base-image and OS-package source obligations are additional to the Rust inventory.

A local build is not publication clearance. Resolve `PUBLICATION-BLOCKERS.txt`,
check actual corresponding-source availability and provide the deployed version's
source directions prominently to all remote users before distribution or remote
operation. Do not rely on an `unknown` revision, mutable tag, or Cargo.lock alone.
See [source access](SOURCE-ACCESS.md), [third-party records](THIRD-PARTY.md) and
[candidate assembly](RELEASING.md). No published alpha tag or asset is replaced.
