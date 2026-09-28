#!/usr/bin/env bash
set -euo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
bash scripts/sync-version.sh --check
triple=${1:-$(rustc -vV | sed -n 's/^host: //p')}
case "$triple" in
  aarch64-apple-darwin|x86_64-apple-darwin|aarch64-unknown-linux-gnu|x86_64-unknown-linux-gnu) ;;
  *) echo "Unsupported release target: $triple" >&2; exit 2 ;;
esac
cargo build --release --locked --target "$triple" --target-dir "$root/target"
version=$(tr -d '[:space:]' < .release-version)
name="herdr-infobox-v${version}-${triple}"
staging=$(mktemp -d)
trap 'rm -rf -- "$staging"' EXIT
mkdir -p "$staging/$name/bin" "$staging/$name/adapters/opencode" "$staging/$name/scripts" dist
cp "target/$triple/release/herdr-infobox" "$staging/$name/bin/herdr-infobox"
cp scripts/install-plugin.sh "$staging/$name/scripts/"
cp herdr-plugin.toml LICENSE README.md .release-version "$staging/$name/"
cp -R docs "$staging/$name/docs"
cp adapters/opencode/index.ts adapters/opencode/package.json adapters/opencode/README.md "$staging/$name/adapters/opencode/"
tar -czf "dist/$name.tar.gz" -C "$staging" "$name"
(cd dist && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
printf 'Prepared %s\n' "dist/$name.tar.gz"
