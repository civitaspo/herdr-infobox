#!/usr/bin/env bash
set -euo pipefail
config="$(pwd)/cliff.toml"
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT
check_bump() {
  local name=$1 subject=$2 expected=$3 actual
  mkdir "$fixture/$name"
  (
    cd "$fixture/$name"
    git init --quiet --initial-branch=main
    git config user.name "Release Policy Test"
    git config user.email "release-policy@example.invalid"
    git config commit.gpgsign false
    git config tag.gpgsign false
    git commit --quiet --allow-empty -m "chore: initial fixture"
    git tag v0.2.0
    git commit --quiet --allow-empty -m "$subject"
    actual=$(git cliff --config "$config" --bumped-version)
    if [[ "${actual#v}" != "$expected" ]]; then
      echo "$name: expected $expected, got $actual" >&2
      exit 1
    fi
  )
}
check_bump fix "fix: handle missing titles" 0.2.1
check_bump feature "feat: add plan view" 0.3.0
check_bump breaking "feat!: change session identity" 0.3.0
echo "Release policy verified: fix -> patch, feature -> minor, 0.x breaking -> minor."
