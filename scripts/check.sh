#!/usr/bin/env sh
# Compile and test the actual Rust application. No success is reported without Cargo.
set -eu
cd "$(dirname "$0")/.."
if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' 'Cargo is required. Rust compilation and tests have not run.' >&2
    exit 127
fi
if [ ! -f Cargo.lock ]; then
    cargo generate-lockfile
    printf '%s\n' 'Generated Cargo.lock. Review and commit it before distributing binaries.' >&2
fi
cargo fmt --all
cargo test --locked --workspace
cargo check --locked -p webtool-server --features documents
cargo check --locked -p webtool-server --features crw-browser
