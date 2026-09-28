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

## Connect OpenCode V2

OpenCode V1 is not supported. Use the verified CLI and server version 2.0.18.
No OpenCode plugin, JavaScript bridge, or provider hook configuration is needed.

Enroll the native session, then select the already running server explicitly:

```sh
herdr-infobox session add --provider opencode --native-id ses_EXACT_NATIVE_ID
herdr-infobox opencode connect --session ses_EXACT_NATIVE_ID --server http://127.0.0.1:4096
herdr-infobox reconcile --session ses_EXACT_NATIVE_ID
```

Use `--binary /absolute/path/to/opencode` if the V2 executable is not on PATH.
The connection is saved only after a successful import. The CLI and server version
are both checked. URLs containing credentials, query strings, or fragments are rejected.
The OpenCode CLI handles `OPENCODE_PASSWORD`; infobox does not save passwords.
Only connect trusted servers whose reported repository paths refer to this host.

The UI worker refreshes connected sessions between its two-second cycles. Each
CLI call has a five-second deadline and the export is capped at 16 MiB.
A slow server can delay the next refresh, but not interactive pane input.
When the UI is closed no collector runs; the next reconciliation reads the full
persisted projected history, including messages before compaction. Repeated
exports are deduplicated, while changed tool states are imported again.
An unavailable server, unknown version/schema, or oversized export reports an error
and retains previous observations. Run `reconcile` for details and `doctor` for diagnostics.

Completed Plan-agent text becomes a proposed document. Switching agents or a
successful session outcome does not imply Plan approval or execution.
Use `plan attach` for explicit files and `plan select` for manual execution selection.

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

Only entries matching the ownership receipt are removed. Edited entries stay in place and are reported. Other provider settings remain intact. Remove manually merged fragments yourself. For OpenCode V2, run `herdr-infobox opencode disconnect --session SESSION`. This removes only the infobox connection, preserving cached history and provider configuration. If upgrading from the old V1 bridge, remove only its exact loader URL from your OpenCode `plugin` array and then remove the owned `opencode.ts` loader; V1 collection is no longer supported.

Remove the plugin through Herdr. Keep the state directory if you want session history or snapshots. Removing infobox does not remove provider transcripts or annotate comments. Snapshots are never automatically deleted.

## Diagnose collection

Run `doctor --json` from the same state directory used by the launcher. Registration and observed hook execution are different checks. A missing title remains `Title unavailable`. Unknown Web or Plan formats expose a reason and manual registration remains available.

Do not change provider sandbox permissions automatically to permit collection. A nonwritable state path is a configuration failure. Collector failures must not block the coding agent.

Real provider sessions may consume credentials or credits. Run them only after explicit approval. Fixture tests and fake Herdr integration tests do not establish real-session compatibility.
