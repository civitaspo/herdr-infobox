# herdr-infobox

[![CI](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml/badge.svg)](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml)

A Herdr plugin that brings a coding-agent session's repositories, changes, references, and plans into one pane.

- Open GitHub links for the session's repositories and worktrees.
- Browse local Git diffs and add comments with herdr-annotate.
- Review web reference URLs, titles, and where they came from.
- Read plan revisions and their execution state.

Switch between sessions or pin one while you work elsewhere. Session history is stored locally.

## Get started

herdr-infobox runs on macOS and Linux with Herdr 0.9.1 or newer. Diff annotation also requires herdr-annotate.

To build from source, install [mise](https://mise.jdx.dev/) and run:

```sh
git clone https://github.com/civitaspo/herdr-infobox.git
cd herdr-infobox
mise install --locked
mise run build
```

Follow the [installation guide](docs/installation.md) to register the plugin with Herdr and configure collection for your coding agent. Then open the pane with Herdr's **Toggle Info** action.

Provider integrations are experimental. Check the [compatibility guide](docs/compatibility.md) for collection capabilities and limitations across Claude Code, Codex, OpenCode, Cursor CLI, and Devin CLI. You can also register repositories, references, and plans manually.

## Use the pane

Press `s` to choose a session, `Tab` to switch sections, and `Enter` to open an item. Press `d` to browse a worktree's changes and `a` to annotate the displayed diff.

See [pane controls](docs/usage.md) for the full key bindings, [Git behavior](docs/git.md) for diff scopes and snapshots, and [local storage](docs/storage.md) for data locations and backups.

## Development

```sh
mise run lint
mise run test
mise run build
```

Read [the contribution guidelines](AGENTS.md) before submitting a change and [the release guide](docs/releasing.md) when preparing a release.

## License

[MIT](LICENSE).
