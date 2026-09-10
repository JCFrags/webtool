#!/usr/bin/env bash
# Local source install only. No sudo, service changes, helpers, or profile edits.
set -euo pipefail
usage() { echo "Usage: $0 [--client-only] [--bin-dir /absolute/user/bin]"; }
client_only=false
bin_dir="${HOME:?HOME must be set}/.local/bin"
while (($#)); do
    case "$1" in
        --client-only) client_only=true; shift ;;
        --bin-dir) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; bin_dir=$2; shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done
[[ $EUID != 0 ]] || { echo 'Run as an ordinary user, without sudo.' >&2; exit 1; }
[[ $bin_dir = /* ]] || { echo '--bin-dir must be absolute.' >&2; exit 1; }
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
mkdir -p -- "$bin_dir"
[[ -d $bin_dir && -O $bin_dir && -w $bin_dir ]] || { echo 'Bin directory must be owned and writable by this user.' >&2; exit 1; }
bin_dir=$(cd -- "$bin_dir" && pwd -P)
names=(webtool)
packages=(-p webtool-cli)
if ! $client_only; then names+=(webtoold); packages+=(-p webtool-server); fi
# Receipts allow updates only when the installed binary still matches our last
# install. Unknown files, symlinks, and independently modified binaries refuse.
check_destination() {
    local name=$1 dest receipt
    dest="$bin_dir/$name"
    receipt="$bin_dir/.$name.install-sha256"
    if [[ -e $receipt || -L $receipt ]]; then
        [[ -f $receipt && -O $receipt && ! -L $receipt ]] || { echo "Refusing unrelated receipt: $receipt" >&2; exit 1; }
    fi
    if [[ -e $dest || -L $dest ]]; then
        [[ -f $dest && -O $dest && ! -L $dest && -f $receipt ]] || { echo "Refusing unrelated executable: $dest" >&2; exit 1; }
        [[ $(sha256sum -- "$dest" | cut -d ' ' -f1) = "$(cat -- "$receipt")" ]] || { echo "Refusing modified executable: $dest" >&2; exit 1; }
    fi
}
for name in "${names[@]}"; do check_destination "$name"; done
cd -- "$root"
# Explicit target directory keeps outputs predictable even with an inherited
# CARGO_TARGET_DIR. Cargo may fetch locked Rust dependencies, never runtime helpers.
cargo build --locked --release --target-dir "$root/target" "${packages[@]}"
tmp=''
trap '[[ -z $tmp ]] || rm -f -- "$tmp"' EXIT
for name in "${names[@]}"; do
    check_destination "$name"
    tmp=$(mktemp "$bin_dir/.$name.XXXXXX")
    cp -- "$root/target/release/$name" "$tmp"
    chmod 755 "$tmp"
    hash=$(sha256sum -- "$tmp" | cut -d ' ' -f1)
    mv -fT -- "$tmp" "$bin_dir/$name"
    tmp=$(mktemp "$bin_dir/.$name.receipt.XXXXXX")
    printf '%s\n' "$hash" >"$tmp"
    mv -fT -- "$tmp" "$bin_dir/.$name.install-sha256"
    tmp=''
    printf 'Installed %s\n' "$bin_dir/$name"
done
printf 'Use absolute binary paths or add %s to PATH yourself. No server was started.\n' "$bin_dir"
