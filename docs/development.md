# Development guide

This guide describes the collection architecture, source files, verification, and packaging for herdr-infobox contributors. For installation and everyday use, see the [README](../README.md).

## Data flow and source map

The core data shape is `EventBatch`, which combines a `SessionKey`, source identity, ordering metadata, and typed `Observation` records. Observations include paths, references, fetch failures, Plan documents, execution evidence, tasks, and capability states.

The collection flow is:

```text
Provider hook stdin / registered transcript / OpenCode session export
    -> provider-specific validation and decoding
    -> EventBatch and typed Observation records
    -> transactional Store application, or a busy-hook spool
    -> stored session projections and pending path discovery
    -> SessionView rendered by the CLI or terminal UI

Herdr native session + pane directories / manual commands
    -> explicit enrollment and session associations

Selected worktree -> bounded local Git capture -> patch view
    -> immutable snapshot -> optional copy-only reviewer pane
```

| File | Responsibility |
| --- | --- |
| [src/main.rs](../src/main.rs) | Clap commands, manual registration, reconciliation orchestration, and hook error suppression. |
| [src/model.rs](../src/model.rs) | Session identity, normalized observations, Plan and task types, source cursors, and view types. |
| [src/config.rs](../src/config.rs) | State path precedence, private directories, and persistent host identity. |
| [src/providers.rs](../src/providers.rs) | Claude, Codex, Cursor, and Devin hook decoding, plus Codex rollout parsing. |
| [src/cursor.rs](../src/cursor.rs) | Cursor project transcript and saved stream parsing. |
| [src/opencode.rs](../src/opencode.rs) | OpenCode V2 export validation and normalization. |
| [src/opencode_transport.rs](../src/opencode_transport.rs) | Bounded CLI calls, explicit server selection, version checks, and export retrieval. |
| [src/opencode_sync.rs](../src/opencode_sync.rs) | Connection persistence and full-export reconciliation. |
| [src/transcripts.rs](../src/transcripts.rs) | Explicit source registration and reconciliation for Codex and Cursor. |
| [src/adapters.rs](../src/adapters.rs) | Hook fragments, absolute launchers, ownership receipts, and supported settings merges. |
| [src/ingest.rs](../src/ingest.rs) | Bounded stdin, enrolled-session persistence, busy spool, replay, and loss accounting. |
| [src/store.rs](../src/store.rs) | Turso schema, atomic projection updates, deduplication, source cursors, and session views. |
| [src/herdr.rs](../src/herdr.rs) | Bounded Herdr CLI calls, native identity checks, and tab-local pane coordination. |
| [src/git.rs](../src/git.rs) | Repository discovery, remote links, status, diff scopes, and subprocess limits. |
| [src/ui.rs](../src/ui.rs) | Terminal rendering, selection, background collection, Git jobs, and pane following. |
| [src/annotate.rs](../src/annotate.rs) | Snapshot export, hash verification, reviewer validation, and copy-only launch. |
| [src/doctor.rs](../src/doctor.rs) | Database, collection, Herdr, and annotation diagnostics. |

Collectors do not run Git discovery in the hook path. They record pending paths; the UI worker or `reconcile` resolves them later. Rendering consumes stored projections rather than rereading all raw provider history on every input event.

When extending a provider, preserve native identity, source provenance, explicit success evidence, and independent Plan/task state. Unsupported formats must report a capability or parsing failure. Missing evidence must not become a successful fetch, approved Plan, or completed execution.

## Run checks

```sh
mise install --locked
mise run lint
mise run test
mise run build
```

`lint` checks version consistency, Rust formatting, Clippy, shell scripts, workflow security, and pinned actions. `test` checks release policy and Rust unit and integration behavior. It uses an explicit host target. `build` checks versions and builds the executable.

The tests cover transaction rollback, replay, native identity, repository and worktree discovery, exact read-only diffs, transcript recovery, provider parsing, fake Herdr coordination, and PTY terminal behavior. Fixtures under [fixtures/](../fixtures/) record their source and coverage in `compatibility.json`. Synthetic payloads and fake integrations do not establish live provider or Herdr compatibility. The released-OpenCode HTTP fixture check is ignored unless a verified binary is explicitly supplied.

Run the synthetic collector benchmark with:

```sh
mise run benchmark
```

See [collector performance evidence](implementation/collector-performance.md) for its workload and interpretation. It is not a production latency guarantee.

Live verification still includes Herdr pane controls, OSC 52, annotation interaction, multi-repository and linked-worktree association, concurrent live sessions, Linux runtime behavior, and remaining provider routes. Record exact provider versions and routes for any new evidence. Credential-consuming sessions and changes to live hooks require explicit authorization.

## Package a binary

```sh
mise exec -- bash scripts/package.sh
```

The script builds a release executable for the host and writes an archive and SHA-256 file under `dist/`. An explicit target argument supports `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, or `x86_64-unknown-linux-gnu`. Install the target and any required cross-compilation tools before using a non-host target.

Archives include the manifest, binary, installation script, README, license, version marker, and docs. Packaging does not publish a release. The source version is currently `0.0.0`; release automation owns version and changelog changes.

## Contribution and automation rules

Use English for code, documentation, comments, issues, commits, and pull requests. Sign commits, use Conventional Commit PR titles, and submit changes through pull requests. Run all three required checks before opening a PR. Maintainer authorization is required for merges, including release PRs.

Keep infrastructure changes separate from plugin implementation. Preserve the required `status-check` context, signature requirements, reviews, pinned action SHAs, and minimal workflow permissions. Read [Securefix](securefix.md) and [releasing](releasing.md) before changing automation. Do not edit `CHANGELOG.md` outside `release/next`; git-cliff owns release notes.

