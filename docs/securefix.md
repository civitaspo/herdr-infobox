# Securefix and repository automation

This client uses [civitaspo/securefix-server](https://github.com/civitaspo/securefix-server) for signed machine commits, independent approvals, and privileged release publication.

## Credentials and apps

Install the existing Securefix client/server GitHub Apps and Renovate on this repository. The server app needs the client repository permissions described in the server's [client release documentation](https://github.com/civitaspo/securefix-server/blob/main/docs/client-releases.md).

| Location | Configuration |
| --- | --- |
| Client repository secret | `SECUREFIX_CLIENT_PRIVATE_KEY` |
| Workflow constants | Client App ID `3872492`; server repository name `securefix-server` |
| securefix-server only | Server app key, GPG/signing keys, bot PAT, repository administration tokens |

No server credentials are copied to this repository. The client app requests work through server labels; the server validates the client workflow and request. No organization secret inheritance is assumed for this user-owned repository.

## CI and approval

`pull_request.yml` is the sole outer PR CI workflow, named **CI**. It calls reusable lint, Rust/release-policy, autofix, and PR-title checks. Its fail-only `status-check` fails when a dependency fails or is cancelled; otherwise it is skipped, which satisfies the required context.

Autofix requests a signed commit only when changes are required and the client key is available. Forks never receive the key. Contributors must fix failed checks themselves when autofix is unavailable.

**Approve Request** runs from trusted `pull_request_target` events or a maintainer's exact `/approve` comment. It never checks out PR code. The actor and committer lists include `civitaspo`, `cursoragent`, `renovate[bot]`, `dependabot[bot]`, and `civitaspo-securefix-server[bot]`. The approval is submitted by the separate `civitaspo-bot` account, which needs write access.

The initial foundation PR cannot run a `pull_request_target` workflow that is absent from its base branch. Approvals from this workflow become available after the foundation is merged.

## Repository policy

Shared settings live in securefix-server's `repo-settings/`: squash-only merges, branch deletion after merge, auto-merge availability, branch update suggestions, signed main commits, one required approval, and the single required context `status-check`. `all-tags` prevents deleting or force-updating tags.

The bootstrap phase applies signature, linear-history, deletion, and non-fast-forward protection first. After the foundation PR is merged, dispatch **Repo settings** for `herdr-infobox` to reconcile full protection; do not lower review or check requirements to force later merges.

Actions require SHA pins and explicitly allowed action owners, use a read-only default token, and require approval for first-time fork contributors. Renovate owns version updates. Dependabot is enabled only for security alerts/fixes, with no version-update configuration.

## Activation

1. Merge the reviewed server allowlist PR and client foundation PR.
2. Configure the client secret and confirm all three app installations.
3. Run `gh workflow run repo-settings.yml -R civitaspo/securefix-server -f repository=herdr-infobox -F create_if_missing=false`.
4. Confirm the bot collaborator and branch/tag rulesets, then verify approval on a subsequent signed PR.
5. Follow [releasing.md](releasing.md) for the first explicitly approved release.
