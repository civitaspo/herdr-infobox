# Repository guidelines

## Scope

herdr-infobox is a planned Rust Herdr plugin that displays session repositories, worktree diffs, web references, and plans. Keep infrastructure changes separate from plugin implementation.

## Contributions

- Use English for code, documentation, comments, commits, issues, and pull requests.
- Use Conventional Commit PR titles: feat, fix, docs, refactor, test, ci, build, chore, perf, or revert.
- Sign commits. Never push directly to main; use pull requests and squash merges.
- Merge only pull requests explicitly authorized by the maintainer. Never automerge release PRs.
- Never remove required checks, signature requirements, or reviews to force a merge.
- Run `mise run lint`, `mise run test`, and `mise run build` before opening a PR.
- Preserve the MIT notices of imported code.

## Automation

- Pin external actions and reusable workflows to full commit SHAs.
- Set `persist-credentials: false` on checkout steps.
- Use minimal workflow permissions and explicit secrets, never `secrets: inherit`.
- Keep strong credentials on civitaspo/securefix-server. This client may hold only its Securefix client app private key.
- Keep the required CI context named `status-check`.
- Do not edit CHANGELOG.md outside release/next; git-cliff owns release notes.
- See docs/securefix.md and docs/releasing.md before changing automation.
