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

The foundation supports **source-only GitHub Releases**. There is no Rust executable yet, so no binaries are built, no crates.io package is published, and no placeholder executable is shipped.

When the implementation is added, wire binary builds into a reviewed server publication strategy that uploads all assets before publishing the draft. Do not add post-publication asset uploads if immutable releases are enabled. The shared release preparer updates `.release-version`; it currently does not update Cargo.toml, Cargo.lock, or herdr-plugin.toml. Choose and test Rust/manifest version synchronization before the first binary release.

## Version policy

`.release-version` starts at `0.0.0`. git-cliff uses stable `vX.Y.Z` tags:

- `fix:` and other releasable changes bump patch.
- `feat:` bumps minor.
- Breaking changes bump minor while the major version is zero, and major after 1.0.0.
- Release-preparation commits are excluded from the changelog.

Before the first stable tag, explicitly select the first version. For example:

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
