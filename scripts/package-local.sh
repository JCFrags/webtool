#!/usr/bin/env bash
# Native Linux candidates only. No installation, server startup or publication.
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
exec python3 "$root/scripts/package.py" "$@"
