# Releasing

This repository uses [csm-ci-actions](https://github.com/civitaspo/csm-ci-actions/blob/main/docs/release-pr.md) hosted by [civitaspo/csm-ci-server](https://github.com/civitaspo/csm-ci-server).

## Flow

1. Squash-merge a reviewed change to `main` through `/merge`.
2. **csm-ci-release-pr** runs git-cliff and asks the server to create or update signed commits on `release/next`. It updates `CHANGELOG.md`, `.release-version`, and the root package versions in `Cargo.toml` and `Cargo.lock`.
3. The existing PR autofix job runs the version synchronizer and submits the corresponding `herdr-plugin.toml` version update through `actions/client/securefix`. The first check run may fail until that signed follow-up commit arrives; require passing full CI on the final head.
4. Review the generated `release-pr: chore(release): vX.Y.Z` PR. The owner comments `/approve` for the current head, waits for required checks and policy, then comments `/merge`. Release PRs are never automerged.
5. **csm-ci-release** requests publication. The server verifies the merged release PR, creates an annotated tag at its squash merge, and publishes an immutable GitHub Release with `SHA256SUMS` and provenance attestation.

The server configuration uses `release.build_type: none`. Releases remain source-only; there are no published binary assets. Local packaging below is separate from publication. A future binary strategy must be reviewed before adding any target or uploaded asset.

The preparation workflow skips release commits to avoid duplicate PRs before the tag exists. Workflow dispatch accepts an optional explicit semantic version for a real release. The publish caller accepts only a merged, same-repository `release/next` PR targeting the default branch.

## Version synchronization

`.release-version` is the version authority. `bash scripts/sync-version.sh` synchronizes the root package in `Cargo.toml` and `Cargo.lock` and the top-level version in `herdr-plugin.toml`. It leaves dependencies and the minimum Herdr version unchanged and validates every destination before writing. `--check` reports mismatches without writing. The existing Rust lint, test, and build entrypoints require agreement.

## Preparing a local binary archive

Run `bash scripts/package.sh` on a native runner for the intended target. The optional first argument is a Rust target triple. Accepted archive layouts are `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, and `x86_64-unknown-linux-gnu`. A listed target is a packaging option, not evidence that its toolchain, binary, or Turso filesystem behavior has been tested.

The script checks version agreement, builds the locked release binary, and creates `dist/herdr-infobox-vVERSION-TARGET.tar.gz` plus a SHA-256 checksum file. The archive contains the Herdr manifest, executable, installation script, license, and documentation. It excludes agent settings, state databases, test fixtures, and dependency directories. The installation script recognizes the packaged executable and does not require a Rust build in the extracted archive.

For each target intended for publication, verify its checksum and inspect the archive contents before extracting it into a temporary directory. Run the extracted `bin/herdr-infobox --version` and compare it with `.release-version`. Run the extracted installation script and confirm the executable remains present. Run `doctor --json --state-dir TEMP_STATE` with a fresh temporary state directory and inspect the compatibility findings. These checks do not authorize changing live hooks or launching real coding agents. Cross-built artifacts require execution and filesystem tests on their actual target OS before being called verified.

Creating an archive does not publish a release. Keep publication behind the existing explicit release PR merge and server validation.

## Version policy

`cliff.toml` controls inferred versions. While major is zero, breaking changes and `feat:` commits bump the minor version; other releasable changes bump the patch version. Release preparation commits do not produce another release.

```bash
mise exec -- git cliff --bumped-version
gh workflow run csm-ci-release-pr.yml -R civitaspo/herdr-infobox -f version=X.Y.Z
```

Dispatch requests a real release PR. Review the version and explicitly request its merge. Published tags and releases are immutable.

## Credentials and recovery

Only the Client App key lives here; see [securefix.md](securefix.md). Strong credentials stay in the server's `csm-ci` environment. The managed `release-tags` ruleset restricts `v*` tag creation to the Server App. General tag integrity protection remains active.

If version synchronization is pending, inspect the autofix job and its server request before retrying. Do not bypass failing checks. For publication failures, inspect the client caller and server run. Reuse an existing tag only when it points to the intended squash merge; never force-update a published tag.
