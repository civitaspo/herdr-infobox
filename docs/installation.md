# Install and maintain infobox

Build the local executable with the pinned toolchain.

```sh
mise install --locked
mise run build
mkdir -p bin
cp target/debug/herdr-infobox bin/herdr-infobox
```

Register this directory using your Herdr version's plugin installation command. Check `herdr plugin install --help` before registering it. The manifest requires Herdr 0.9.1. The implementation uses split panes. Source inspection does not replace a live installation test. See [compatibility](compatibility.md).

Keep user data outside the plugin checkout. Herdr supplies `HERDR_PLUGIN_STATE_DIR`. Outside Herdr, pass `--state-dir /absolute/path` or use the default `$XDG_STATE_HOME/herdr-infobox`, falling back to `$HOME/.local/state/herdr-infobox`.

## Try manual registration

Use an isolated state directory for a smoke test.

```sh
herdr-infobox --state-dir /tmp/infobox-demo session add --provider claude --native-id demo
herdr-infobox --state-dir /tmp/infobox-demo repo add --session demo --path /path/to/repository
herdr-infobox --state-dir /tmp/infobox-demo ref add --session demo --url https://example.com/docs --title 'Example docs'
herdr-infobox --state-dir /tmp/infobox-demo plan attach --session demo --file /path/to/plan.md
herdr-infobox --state-dir /tmp/infobox-demo ui --session demo
```

Repeat `repo add` to associate more repositories or linked worktrees. Session selectors accept an unambiguous UUID prefix, native ID, or `provider:native-id`. Use the returned session UUID when names overlap.

Plan attachment records a draft. `plan select --session SESSION --revision HASH --state executing` records your display selection. It does not approve the provider's plan or dispatch work. Attach an edited file again to retain another revision.

## Register provider collectors

Plugin registration does not install provider hooks. Review the generated configuration before changing your settings.

```sh
herdr-infobox adapters install claude --dry-run
herdr-infobox adapters install codex --dry-run
herdr-infobox adapters install cursor --dry-run
```

Without `--config`, installation generates a stable collector launcher and prints a fragment to merge yourself. With `--config PATH`, it merges supported JSON hook entries and records ownership. It keeps unrelated keys and hooks. It does not rewrite JSONC or TOML. For these formats, merge the generated fragment into the existing configuration.

Typical explicit targets are `$HOME/.claude/settings.json`, `$CODEX_HOME/hooks.json`, and `$HOME/.cursor/hooks.json`. If `CODEX_HOME` is unset, use `$HOME/.codex/hooks.json`. Choose project scope explicitly by passing its path. Do not register both inline Codex hooks and hooks.json in one configuration layer. Complete the provider's own hook trust review.

A collector stores only enrolled sessions. Register the exact native session with `session add`, or let the current Herdr pane establish that identity. A matching working directory alone does not enroll a session. The launcher records an absolute binary path and state path. It does not depend on the provider's working directory or Herdr plugin environment. A Devin process importing Claude hooks is classified through the documented Devin environment rather than silently recorded as Claude.

For OpenCode V1, set `HERDR_PLUGIN_ROOT` to the installed infobox directory and run `adapters install opencode`. Merge the printed file URL into your existing `plugin` array. The generated TypeScript loader records the absolute binary and state paths. The bridge resolves each tool's native session before collecting it. OpenCode V2 remains unsupported pending a verified adapter.

Devin automatic settings mutation is unavailable because individual tool contracts and imported-hook interactions need runtime verification. Its conservative parser can consume documented lifecycle fixtures. Do not label that parser coverage as a verified installed integration.

## Upgrade

Back up the state directory while all collectors and panes are closed, or use the Turso backup procedure in [local storage](storage.md). Do not use a SQLite process against an active Turso database, and do not copy an active WAL database file alone.

Replace the executable and plugin files. Run `doctor --json`. Run adapter installation again with the same state, config directory, and explicit settings path to update owned launchers. If you edited an owned launcher or hook, the upgrade refuses to overwrite that edit. Resolve it manually before retrying.

A binary refuses a newer database schema. Keep the previous executable and a consistent backup until the upgrade is verified. No release or live hook update is performed by building this repository.

## Uninstall

For settings installed with an explicit path, run the corresponding command.

```sh
herdr-infobox adapters uninstall claude --config "$HOME/.claude/settings.json"
```

Only entries matching the ownership receipt are removed. Edited entries stay in place and are reported. Other provider settings remain intact. Remove manually merged fragments yourself. For OpenCode, remove the generated loader's exact file URL from the plugin array, then run `adapters uninstall opencode`.

Remove the plugin through Herdr. Keep the state directory if you want session history or snapshots. Removing infobox does not remove provider transcripts or annotate comments. Snapshots are never automatically deleted.

## Diagnose collection

Run `doctor --json` from the same state directory used by the launcher. Registration and observed hook execution are different checks. A missing title remains `Title unavailable`. Unknown Web or Plan formats expose a reason and manual registration remains available.

Do not change provider sandbox permissions automatically to permit collection. A nonwritable state path is a configuration failure. Collector failures must not block the coding agent.

Real provider sessions may consume credentials or credits. Run them only after explicit approval. Fixture tests and fake Herdr integration tests do not establish real-session compatibility.
