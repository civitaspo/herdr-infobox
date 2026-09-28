#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
build=$(mktemp -d)
trap 'rm -rf -- "$build"' EXIT
rustc --edition=2024 -D warnings "$root/scripts/sync-version.rs" -o "$build/sync-version"
"$build/sync-version" --root "$root" "$@"
