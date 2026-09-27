# herdr-infobox

[![CI](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml/badge.svg)](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml)

A Herdr information pane for coding-agent repositories, diffs, web references, and plans.

## Status

This repository currently contains the project foundation and release infrastructure. The Rust plugin is not implemented yet, and there is no installable binary or Herdr plugin manifest.

The planned pane will show:

- GitHub links for the repositories and worktrees observed in an agent session.
- Local worktree diffs, with review comments opened in herdr-annotate.
- URLs and titles supplied by web-search tools.
- Plan documents and task progress, with provider-specific collection status.

The initial providers are Codex, Claude Code, OpenCode, and Cursor CLI. Devin CLI is an additional target. Missing provider metadata will be shown explicitly rather than inferred.

## Development

Install the pinned tools and run the checks:

```bash
mise install --locked
mise run lint
mise run test
mise run build
```

Before a root `Cargo.toml` exists, Rust checks report that the implementation is absent. Release-policy tests still run. Once the Rust package is added, the same commands run formatting, Clippy, tests, and a locked build.

See [CONTRIBUTING.md](CONTRIBUTING.md), [the project scope](docs/project-scope.md), [Securefix](docs/securefix.md), and [releasing](docs/releasing.md). All repository communication is in English.

## License

[MIT](LICENSE).
