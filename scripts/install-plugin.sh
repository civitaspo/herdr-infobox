#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
if [[ ! -f Cargo.toml ]]; then
  test -x bin/herdr-infobox
  exit
fi
target=$(rustc -vV | sed -n 's/^host: //p')
cargo build --release --locked --target "$target" --target-dir "$root/target"
mkdir -p bin
install -m 755 "target/$target/release/herdr-infobox" bin/herdr-infobox
