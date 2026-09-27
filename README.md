# herdr-infobox

[![CI](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml/badge.svg)](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml)

A Rust information pane for coding-agent sessions in Herdr.

Info keeps repository and worktree links, read-only local diffs, web references with provenance, and Plan revisions together. Data persists in a local embedded Turso database while the pane is closed. Session identity is independent of pane identity. Plan documents, task checklists, approval, and execution are separate records.

Diff review exports the displayed patch to an immutable Markdown snapshot and opens the reviewer supplied by herdr-annotate in copy-only mode. It never stages files, changes the Git index, fetches remotes, approves a Plan, or sends comments to an uncertain agent session.

## Try it

```sh
mise install --locked
mise run build
target/debug/herdr-infobox session add --provider claude --native-id example
target/debug/herdr-infobox repo add --session example --path /path/to/repository
target/debug/herdr-infobox ref add --session example --url https://example.com/docs
target/debug/herdr-infobox plan attach --session example --file /path/to/plan.md
target/debug/herdr-infobox ui --session example
```

Use `--state-dir /absolute/path` for isolated tests. Add more repositories with `repo add`. Use `ui --once` to inspect a session without an interactive terminal. Use `doctor --json` to distinguish registration, observed collection, and unavailable integrations.

See [installation and upgrades](docs/installation.md), [pane controls](docs/usage.md), [Git behavior](docs/git.md), and [local storage](docs/storage.md).

## Provider coverage

Claude Code has a fixture-tested path through repository discovery, diffs, references, Plan documents, and checklists. OpenCode V1 has a small TypeScript bridge. Codex has hook collection and an explicit versioned JSONL reader. Cursor and Devin expose conservative session metadata and manual fallbacks where tool schemas are unverified.

All provider fixtures are synthetic examples derived from official documentation or source. Real coding-agent sessions and live hook delivery have not been validated. The pane shows partial or unavailable capabilities with reasons. Missing metadata never proves successful fetching, plan approval, or completion. See [compatibility evidence](docs/compatibility.md).

The target platforms are macOS and Linux, with Herdr 0.9.1 or newer. Live Herdr and clipboard smoke tests remain manual acceptance items. Turso 0.7.2 multiprocess WAL is explicitly experimental. No daemon, cloud database, web dashboard, or replacement agent runtime is installed.

## Development

```sh
mise run lint
mise run test
mise run build
```

Tests include versioned parser fixtures, temporary Git repositories, fake Herdr/annotate executables, and actual collector CLI processes with temporary state. The default test task also runs the PTY UI and OpenCode bridge checks using the pinned Python and Node runtimes. See [the bridge README](adapters/opencode/README.md).

Mise owns the lint/test/build commands directly; CI uses the same tasks. `lint:rust` is the Rust-only check used by the platform matrix. Lint does not rewrite workflows: ghalint enforces checkout credential policy, while the Securefix autofix job owns repairs. Runtime tests build the native host target explicitly and pass its exact executable path to the probes, even when Cargo has a cross-compilation target configured. The build task respects normal Cargo configuration. Use `mise -C /path/to/herdr-infobox run test` from outside the checkout.

The existing Securefix release architecture remains in place. No release is published by the test suite. Read [releasing](docs/releasing.md) before preparing a release. See [the implementation workflow](docs/implementation/plan.md) for decisions and acceptance evidence.

## License

[MIT](LICENSE).
