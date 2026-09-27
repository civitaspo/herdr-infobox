# Herdr and annotate contracts verified 2026-09-28

## Provenance and limits

Read the complete design document. Independently cloned official current sources without changing the implementation repository or live Herdr state.

- Herdr master `fff6c820aa45f4eabb9b2e0456326dc74cca5a25` in `/tmp/infobox-herdr-source`.
- herdr-annotate `1bc258353f0a7af0781493e1c1ffca09b71666bc`, manifest version 0.6.0 in `/tmp/infobox-annotate-source`.
- plannotator-tui `59fa0af536260b54b4a6341ab306c9e6c93d9a55`, version 0.9.4 in `/tmp/infobox-plannotator-source`.
- Located binary `/Users/takahiro.nakayama/.local/share/mise/installs/herdr/0.8.2/herdr`. Its `--version` reports 0.8.2 and `api schema` reports protocol 20. Bundled JSON is `/tmp/infobox-herdr-schema.json`. This is older than design minimum 0.9.1. It may not be the currently running server binary. No runtime pane creation, hook changes or real agent sessions performed.

Official source links use these exact commits. Current Herdr differs from design revision, whereas annotate and plannotator match design revisions.

## Herdr boundary

Use `HERDR_BIN_PATH` when supplied and preserve `HERDR_SOCKET_PATH`. Invoke argv directly.

Read-only commands are `herdr pane list [--workspace ID]`, `herdr pane get PANE`, `herdr agent get PANE`, `herdr plugin list --plugin annotate --json`, and `herdr api schema --json`. Standard API responses use an envelope with `result`; `pane get` yields `/result/pane`, `agent get` yields `/result/agent`.

Info launch argv is `herdr plugin pane open --plugin herdr-infobox --entrypoint info --placement split --target-pane PANE --direction right --no-focus` with optional `--workspace ID`, `--cwd PATH`, repeated `--env KEY=VALUE`. Installed 0.8.2 help confirms those flags. Current source additionally supports popup width/height, but 0.8.2 help has no popup.

`src/api/schema/panes.rs:441` defines pane fields `pane_id`, `terminal_id`, `workspace_id`, `tab_id`, `focused`, optional `cwd`, optional `foreground_cwd`, optional `agent`, optional `agent_session`, and revision. `src/api/schema/agents.rs:232` defines `agent_session` as `{source: string, agent: string, kind: "id"|"path", value: string}`. Do not deserialize it as a raw string. Only kind `id` directly gives a native session ID. Unknown kinds and unavailable sessions must remain unbound. If path kinds are added later, require a known transcript parser to obtain native identity.

`src/agent_resume.rs` shows official current Claude/Codex/Cursor/Devin reports use IDs; path is accepted for pi/omp. Preserve provider from session.agent rather than display labels. `PluginInvocationContext` in `src/api/schema/plugins.rs:364` provides focused pane ID, cwd, agent and tab/workspace but does NOT provide agent_session. Requery pane to resolve identity. Environment context is a snapshot at invocation and cannot prove current session later.

`src/api/schema/events.rs` confirms event names `pane.focused`, `tab.focused`, `workspace.focused`. An event notification should trigger a current pane snapshot, not become long-lived session identity. A fresh process/socket generation must invalidate old binding evidence.

Installed plugin record (`src/api/schema/plugins.rs:37`) contains `plugin_id`, `version`, `min_herdr_version`, `manifest_path`, `plugin_root`, `enabled`, and `panes`. Resolve annotate binary from actual registered plugin_root + `bin/plannotator-tui.exe`; this filename is intentional on Unix too. Validate entrypoint `doc` exists and binary is executable. Lite installations may lack it.

## Manifest

Current docs at `docs/next/website/src/content/docs/plugins.mdx` and schema confirm required id/name/version/min_herdr_version; commands are argv arrays, not shell strings. The design's manifest shape is valid current-source syntax:

```toml
id = "herdr-infobox"
name = "Info"
version = "0.1.0"
min_herdr_version = "0.9.1"
platforms = ["macos", "linux"]
[[panes]]
id = "info"
title = "Info"
placement = "split"
command = ["./bin/herdr-infobox", "ui"]
[[actions]]
id = "toggle"
title = "Toggle Info"
command = ["./bin/herdr-infobox", "toggle"]
[[events]]
on = "pane.focused"
command = ["./bin/herdr-infobox", "ensure"]
```

Add tab/workspace focus separately if implemented. Current docs say Herdr injects HERDR_ENV, BIN_PATH, SOCKET_PATH, PLUGIN_ID/ROOT/CONFIG_DIR/STATE_DIR/CONTEXT_JSON, and available WORKSPACE_ID/TAB_ID/PANE_ID. Events additionally get PLUGIN_EVENT and PLUGIN_EVENT_JSON. Data must be outside checkout. Avoid trusting PANE_ID as target when called from own UI.

## Copy-only annotation

`crates/plannotator-tui/src/cli.rs:56` selects Clipboard when interactive unless BOTH HERDR_ENV=1 and a delivery_target exists. `herdr/context.rs` derives delivery_target first from PLANNOTATOR_TUI_DELIVER_TO, else focused pane/agent in HERDR_PLUGIN_CONTEXT_JSON. `delivery.rs:121` calls `herdr agent prompt PANE FEEDBACK` without native session comparison. It stores pane and agent only. Therefore send cannot meet infobox session-reuse correctness requirement.

Recommended infobox-owned pane entrypoint invokes `herdr-infobox annotate-view SNAPSHOT` or receives snapshot path via a narrow dedicated environment variable. The wrapper executes registered `bin/plannotator-tui.exe` with exactly one absolute snapshot path argv. Immediately before exec remove HERDR_ENV, HERDR_PLUGIN_CONTEXT_JSON and all PLANNOTATOR_TUI_* environment variables. Also remove infobox-specific HERDR_PLUGIN_* context for cleanliness. Removing HERDR_ENV alone guarantees Clipboard for this pinned version; clearing delivery/context variables makes the intent robust. Keep terminal I/O inherited for interactive mode. Do NOT call `herdr open` for default copy-only mode because it repopulates delivery context while opening doc.

A secondary send-capable launch is source-confirmed as `plannotator-tui.exe herdr open SNAPSHOT --placement split --deliver-to PANE`, with child HERDR_PLUGIN_ID=annotate; it opens `annotate.doc`, sets PLANNOTATOR_TUI_FILE and delivery variables. Do not expose this as session-safe. There is no source-confirmed send-time session check.

Clipboard uses OSC 52 (`delivery.rs:64`), and its comment specifies Herdr forwarding from 0.9.0. No terminal clipboard roundtrip or UI runtime smoke performed. Copy selection cannot be called runtime supported from source inspection alone.

## Minimal recommendation

Use one Herdr module with typed minimal pane/session deserialization and a bounded argv runner; no socket client is necessary for initial list/get and pane creation. Persist session keys independently. Serialize toggle/ensure by per-instance+tab OS lock, record enabled state and launch reservation before creation, reconcile current pane listing before reopening, and ignore focus on plugin panes. Keep explicit manual picker as fallback. Ship copy-only annotate wrapper and label runtime tests pending.

Unknowns remaining are actual installed/running 0.9.1 availability, manifest install compatibility of annotate's popup panes on older binaries, live focus and duplicate ensure behavior, OSC52 forwarding under user's terminal, and instance restart identity. Source compatibility is not runtime compatibility.
