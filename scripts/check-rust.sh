#!/usr/bin/env bash
set -euo pipefail
case "${1:-}" in
  lint|test|build) ;;
  *) echo "Usage: $0 lint|test|build" >&2; exit 2 ;;
esac
if [[ ! -f Cargo.toml ]]; then
  echo "Infrastructure-only repository: no Cargo.toml exists yet; Rust ${1} is not applicable."
  exit 0
fi
python3 scripts/sync-version.py --check
case "$1" in
  lint)
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    ;;
  test)
    python3 scripts/test-version-sync.py
    cargo test --workspace --all-features --locked
    ;;
  build) cargo build --workspace --locked ;;
esac
