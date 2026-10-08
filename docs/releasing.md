# Releasing

Releases use the shared Client/Server Model in [securefix-server](https://github.com/civitaspo/securefix-server/blob/main/docs/client-releases.md). The client wrappers pin reusable workflows to an immutable commit.

## Flow

1. Squash-merge a reviewed PR to main using its Conventional Commit title.
2. **Release PR** runs git-cliff and asks Securefix to create or update signed commits on `release/next`, changing `.release-version` and `CHANGELOG.md`.
3. **Release PR Sync** updates an existing release PR's title and body.
4. A maintainer explicitly squash-merges `chore(release): vX.Y.Z`. Never enable auto-merge for release PRs.
5. **Release Tag** creates an annotated tag at the squash merge commit and requests server publication through a label.
6. The server validates the allowlist, workflow run, tag ancestry, and expected merge SHA, then creates and publishes a draft GitHub Release.

The server allowlist entry is `civitaspo/herdr-infobox` with `publish: github-release`. No GPG key, bot PAT, or server app key is stored here.

## Current publication scope

The server still publishes **source-only GitHub Releases**. Local binary packaging is available, but this change does not add asset upload, crates.io publication, release dispatch, or a new server strategy.

Before publishing binaries, review a server publication strategy that builds and verifies every intended target, uploads all assets to the draft, and publishes only after the asset set is complete. The current `publish: github-release` allowlist remains unchanged. Do not upload assets after publication when immutable releases are enabled.

## Version synchronization

`.release-version` is the version authority. Run `bash scripts/sync-version.sh` to update the root package in Cargo.toml and Cargo.lock and the top-level version in herdr-plugin.toml. The launcher compiles a standard-library-only Rust utility with rustc, so it can repair stale Cargo metadata without first asking Cargo to resolve that metadata. The script leaves dependency versions and the minimum Herdr version unchanged. It accepts stable versions and prereleases, rejects invalid numeric identifiers, and validates all three destinations before writing.

`bash scripts/sync-version.sh --check` reports mismatches without writing. All Rust lint, test, and build entrypoints run this check. The test entrypoint also exercises synchronization in temporary fixtures, including a prerelease, an invalid version, and an unrelated lockfile package.

The shared release preparer continues to update `.release-version` and CHANGELOG.md. The existing PR autofix job synchronizes the three package files and submits the resulting change through its existing Securefix action. It does not push directly or gain additional permissions. A release PR may initially fail the version check until that signed follow-up commit arrives. Review the synchronized versions and require green CI before explicitly merging the release PR. If Securefix cannot submit the change, perform the same synchronization on the release branch through the normal signed PR process. Never bypass the check.

## Preparing a local binary archive

Run `bash scripts/package.sh` on a native runner for the intended target. The optional first argument is a Rust target triple. Accepted archive layouts are `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, and `x86_64-unknown-linux-gnu`. A listed target is a packaging option, not evidence that its toolchain, binary, or Turso filesystem behavior has been tested.

The script checks version agreement, builds the locked release binary, and creates `dist/herdr-infobox-vVERSION-TARGET.tar.gz` plus a SHA-256 checksum file. The archive contains the Herdr manifest, executable, installation script, license, and documentation. It excludes agent settings, state databases, test fixtures, and dependency directories. The installation script recognizes the packaged executable and does not require a Rust build in the extracted archive.

For each target intended for publication, verify its checksum and inspect the archive contents before extracting it into a temporary directory. Run the extracted `bin/herdr-infobox --version` and compare it with `.release-version`. Run the extracted installation script and confirm the executable remains present. Run `doctor --json --state-dir TEMP_STATE` with a fresh temporary state directory and inspect the compatibility findings. These checks do not authorize changing live hooks or launching real coding agents. Cross-built artifacts require execution and filesystem tests on their actual target OS before being called verified.

Creating an archive does not publish a release. Keep publication behind the existing explicit release PR merge and server validation.

## Version policy

The initial `.release-version` is `0.0.0`. git-cliff uses stable `vX.Y.Z` tags:

- `fix:` and other releasable changes bump patch.
- `feat:` bumps minor.
- Breaking changes bump minor while the major version is zero, and major after 1.0.0.
- Release-preparation commits are excluded from the changelog.

With no stable tag, git-cliff starts automatic releases at `0.1.0`. To choose a prerelease instead, explicitly select its version. For example:

```bash
gh workflow run release-pr.yml -R civitaspo/herdr-infobox -f version=0.0.1-pre.1
```

This only requests a release PR. Review and explicitly merge that PR to publish. Do not run it solely to test setup without intending to create that release.

Pre-release tags do not become the base for automatic stable bumps. A later main push can rewrite an open explicit pre-release PR to a computed stable version; dispatch the explicit version again if needed. Tags are permanent once published.

The server handles the difference between the release branch head and the squash merge SHA; tag verification must use the latter. A label description longer than GitHub's limit omits the SHA and lets the server resolve it from the merged PR.

## Validation and recovery

`mise run test` checks the 0.x version bump policy in isolated temporary repositories. `mise exec -- git cliff --bumped-version` previews the current proposed version.

If Securefix pushes a prepare commit but reports that the release PR already exists, inspect `release/next` and **Release PR Sync** before retrying. Do not create another release branch.

For a failed tag/publish run, inspect both client and server workflow summaries. Reuse the existing immutable tag only when its commit matches the intended release; never force-update it. The reusable Release Tag workflow supports a manual `merge_sha` retry after checking the documented contract.

End-to-end tag and publication verification requires a separately approved release PR merge. The repository setup itself does not publish a release.

## Merge requests

A human requests the release merge by posting `/merge` on the pull request. Securefix performs the squash merge after the required checks and review pass. Only `civitaspo` may request it; release pull requests are not automerged. See the shared [merge policy](https://github.com/civitaspo/securefix-server/blob/main/docs/merging.md).
