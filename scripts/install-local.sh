#!/usr/bin/env bash
# User-owned source or extracted archive install. No service or helper changes.
set -euo pipefail
usage() { echo "Usage: $0 [--client-only] [--bin-dir /absolute/user/bin] [--from-archive /absolute/extracted/archive]"; }
client_only=false
archive_dir=''
declare -A archive_hashes=()
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
# The packager places this same script beside each archive's binary and manifest.
if [[ -f $script_dir/BINARY-SHA256SUMS ]]; then archive_dir=$script_dir; fi
bin_dir="${HOME:?HOME must be set}/.local/bin"
while (($#)); do
    case "$1" in
        --client-only) client_only=true; shift ;;
        --bin-dir) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; bin_dir=$2; shift 2 ;;
        --from-archive) [[ $# -ge 2 ]] || { usage >&2; exit 2; }; archive_dir=$2; shift 2 ;;
        --help|-h) usage; exit 0 ;;
        *) usage >&2; exit 2 ;;
    esac
done
[[ $EUID != 0 ]] || { echo 'Run as an ordinary user, without sudo.' >&2; exit 1; }
[[ $(uname -s) = Linux ]] || { echo 'This installer supports Linux only.' >&2; exit 1; }
[[ $bin_dir = /* ]] || { echo '--bin-dir must be absolute.' >&2; exit 1; }
root=$(cd -- "$script_dir/.." && pwd -P)
if [[ -n $archive_dir ]]; then
    [[ $archive_dir = /* ]] || { echo '--from-archive must be absolute.' >&2; exit 1; }
    archive_dir=$(cd -- "$archive_dir" && pwd -P)
    [[ -f $archive_dir/BINARY-SHA256SUMS && ! -L $archive_dir/BINARY-SHA256SUMS ]] || { echo 'Archive binary checksum manifest is missing or symlinked.' >&2; exit 1; }
fi
mkdir -p -- "$bin_dir"
[[ -d $bin_dir && -O $bin_dir && -w $bin_dir ]] || { echo 'Bin directory must be owned and writable by this user.' >&2; exit 1; }
bin_dir=$(cd -- "$bin_dir" && pwd -P)
names=(webtool)
packages=(-p webtool-cli)
if ! $client_only; then names+=(webtoold); packages+=(-p webtool-server); fi
if [[ -n $archive_dir ]]; then
    names=()
    # Accept only executable names, never paths or options from a supplied file.
    while IFS= read -r line; do
        [[ $line =~ ^([0-9a-f]{64})\ \ (webtool|webtoold)$ ]] || { echo 'Invalid archive binary checksum manifest.' >&2; exit 1; }
        name=${BASH_REMATCH[2]}
        expected_hash=${BASH_REMATCH[1]}
        if $client_only && [[ $name != webtool ]]; then continue; fi
        [[ ! " ${names[*]} " = *" $name "* ]] || { echo 'Duplicate archive binary checksum.' >&2; exit 1; }
        binary="$archive_dir/$name"
        [[ -f $binary && ! -L $binary ]] || { echo "Missing or symlinked archive binary: $name" >&2; exit 1; }
        [[ $(sha256sum -- "$binary" | cut -d ' ' -f1) = "$expected_hash" ]] || { echo "Archive binary checksum mismatch: $name" >&2; exit 1; }
        archive_hashes[$name]=$expected_hash
        names+=("$name")
    done <"$archive_dir/BINARY-SHA256SUMS"
    ((${#names[@]})) || { echo 'Archive has no selected binary.' >&2; exit 1; }
fi
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
if [[ -z $archive_dir ]]; then
    cargo build --locked --release --target-dir "$root/target" "${packages[@]}"
    source_dir="$root/target/release"
else
    source_dir=$archive_dir
fi
tmp=''
trap '[[ -z $tmp ]] || rm -f -- "$tmp"' EXIT
for name in "${names[@]}"; do
    check_destination "$name"
    tmp=$(mktemp "$bin_dir/.$name.XXXXXX")
    cp -- "$source_dir/$name" "$tmp"
    chmod 755 "$tmp"
    hash=$(sha256sum -- "$tmp" | cut -d ' ' -f1)
    if [[ -n $archive_dir && $hash != "${archive_hashes[$name]}" ]]; then
        echo "Archive binary changed during installation: $name" >&2; exit 1
    fi
    mv -fT -- "$tmp" "$bin_dir/$name"
    tmp=$(mktemp "$bin_dir/.$name.receipt.XXXXXX")
    printf '%s\n' "$hash" >"$tmp"
    mv -fT -- "$tmp" "$bin_dir/.$name.install-sha256"
    tmp=''
    printf 'Installed %s\n' "$bin_dir/$name"
done
printf 'Use absolute binary paths or add %s to PATH yourself. No server was started.\n' "$bin_dir"
