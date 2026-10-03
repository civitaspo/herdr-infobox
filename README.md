# herdr-infobox

[![CI](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml/badge.svg)](https://github.com/civitaspo/herdr-infobox/actions/workflows/pull_request.yml)

A Rust [Herdr](https://herdr.dev/) plugin that displays a coding-agent session's repositories, local Git changes, web references, and Plan documents in one terminal pane.

Info follows the identified agent session in the current Herdr tab. You can select and pin another session, including retained history. Multiple repositories and linked worktrees can belong to one session.

The current source implements the pane, CLI, local database, collectors, and diff exports. Provider integrations are experimental and depend on specific versions and collection routes. Automatic collection is incomplete. Manual registration remains available for every provider. See [compatibility evidence](docs/compatibility.md) before relying on a particular integration.

## Contents

- [Install and open Info](#install-and-open-info)
- [Try the CLI without provider hooks](#try-the-cli-without-provider-hooks)
- [Connect a coding agent](#connect-a-coding-agent)
- [Use the pane](#use-the-pane)
- [Understand the displayed information](#understand-the-displayed-information)
- [Inspect and export Git changes](#inspect-and-export-git-changes)
- [CLI reference](#cli-reference)
- [Storage and configuration](#storage-and-configuration)
- [Troubleshooting](#troubleshooting)
- [Develop and verify changes](#develop-and-verify-changes)
- [Further documentation](#further-documentation)

## Install and open Info

### Requirements

| Requirement | Purpose |
| --- | --- |
| macOS or Linux | Platforms declared in the plugin manifest. Linux runtime acceptance remains incomplete. |
| Herdr 0.9.1 or newer | Plugin panes, focus events, and native agent-session identity. |
| Git available on `PATH` | Repository discovery and local diffs. |
| [mise](https://mise.jdx.dev/) and the pinned tools | Build and development from source. `mise.toml` and `mise.lock` define the pinned toolchain. |
| Annotate Full 0.6.0 with plannotator-tui 0.9.4 | Optional copy-only annotation. Info requires the registered `doc` entrypoint and reviewer executable. |

The embedded database needs no Turso Cloud account, server, or credentials. GitHub links need no GitHub token or API request.

### Build and link a local checkout

```sh
git clone https://github.com/civitaspo/herdr-infobox.git
cd herdr-infobox
mise install --locked
mise run build
mkdir -p bin
cp target/debug/herdr-infobox bin/herdr-infobox
herdr plugin link "$PWD" --enabled
```

The manifest launches `./bin/herdr-infobox`. A Cargo build alone does not put the executable there. After rebuilding a linked checkout, copy the new executable into `bin` again.

For a release-mode local executable, run this command instead of the debug build and copy steps:

```sh
mise exec -- bash scripts/install-plugin.sh
```

The script builds for the host target and installs `bin/herdr-infobox`. It also serves as the manifest's build command. Herdr's GitHub installation route is `herdr plugin install civitaspo/herdr-infobox`; check `herdr plugin install --help` for your installed version's options. That route installs repository contents rather than establishing that a published binary release exists.

Open **Toggle Info** from Herdr's plugin actions. Focus events keep an enabled Info pane associated with its tab. Plugin installation and provider collector registration are separate steps. See [Connect a coding agent](#connect-a-coding-agent).

Standalone commands can use `./bin/herdr-infobox` or `./target/debug/herdr-infobox`. The examples below use `herdr-infobox` when the executable is on `PATH`.

### Session identity

A stored session has four identity fields, defined by `SessionKey` in [src/model.rs](src/model.rs):

- A host UUID persisted in the selected state directory.
- A provider, such as `claude` or `cursor`.
- The provider's native session ID.
- An agent scope, which defaults to `main`.

Herdr supplies pane metadata through its CLI API. Info accepts an `agent_session` with `kind = "id"` when its provider matches the pane's agent. Pane IDs and working directories do not establish permanent session identity. If identity is unavailable, use the session picker or register a native session explicitly.

CLI session selectors accept an unambiguous stored UUID prefix, native ID, or `provider:native-id`. Use the full UUID returned by `session add` when names overlap. Repeating `session add` for the same identity returns the existing session.

## Try the CLI without provider hooks

This example creates an isolated state directory and registers the current checkout manually. It does not run a coding agent or change provider settings.

Run it from the repository root after `mise run build`:

```sh
INFOBOX="$PWD/target/debug/herdr-infobox"
DEMO="$(mktemp -d)"

"$INFOBOX" --state-dir "$DEMO" session add --provider claude --native-id demo
"$INFOBOX" --state-dir "$DEMO" repo add --session claude:demo --path "$PWD"
"$INFOBOX" --state-dir "$DEMO" ref add --session claude:demo \
	--url https://example.com/docs --title 'Example docs'
printf '# Demo plan\n\nReview the current worktree.\n' > "$DEMO/plan.md"
"$INFOBOX" --state-dir "$DEMO" plan attach --session claude:demo --file "$DEMO/plan.md"
"$INFOBOX" --state-dir "$DEMO" ui --session claude:demo --once
```

The commands print a session UUID, a worktree ID, and a Plan revision hash. The final command prints a text view containing one repository, one manual reference, and one draft Plan. `ui --once` does not start the background reconciliation worker.

To open the interactive terminal view, run:

```sh
"$INFOBOX" --state-dir "$DEMO" ui --session claude:demo
```

Repeat `repo add` with another repository or linked-worktree path to associate it with the same session. Keep the same `--state-dir` for all commands that read that session.

## Connect a coding agent

### Collection routes and limits

| Provider | Information source | Extracted information | Current limits |
| --- | --- | --- | --- |
| Claude Code | Registered lifecycle and tool hooks | Native session, cwd, successful file-tool paths, WebFetch requests and explicit successful responses, structured WebSearch results, proposed and approved ExitPlanMode text, supported task records. | Fixture-tested against Agent SDK 0.3.283 contracts. Live CLI hook delivery remains unverified. TaskCreate and TaskUpdate partial updates are not interpreted. |
| Codex CLI | Registered hooks and an explicitly registered JSONL rollout | Native session and cwd, `update_plan` checklist, supported completed Plan and WebSearch records. | Hosted WebSearch bypasses hooks. Rollout format is unstable. Compressed or paginated storage and extension WebSearch are unsupported. |
| OpenCode V2 | Full session export through the CLI against an explicitly selected existing server | Native session and parent, current and historical directories, file-tool paths, successful or failed webfetch observations, completed Plan-agent prose. | CLI and server must each be 2.0.15 or 2.0.18. Titles, search results, checklists, Plan approval, and execution are unavailable. V1 is unsupported. |
| Cursor CLI | Hooks plus an explicitly registered project JSONL or saved print-mode `stream-json` file | Hook session and workspace information, transcript Web requests, proposed Plan text, task definitions, explicit successful fetches in saved streams. | Tested runtime is 2026.09.26-dd393fe. Durable transcripts omit tool results, titles, approval, and execution. ACP is a separate runtime. |
| Devin CLI | Conservative lifecycle decoder, including `DEVIN_PROJECT_DIR` | Stable native identity, project root, and capability diagnostics. | Automatic settings mutation and tool-specific Web, Plan, and checklist extraction are unavailable. |

These are implementation capabilities, not a claim that every installed provider emits every record. OpenCode 2.0.15 and Cursor have anonymized live captures. Other coverage includes source inspection and synthetic fixtures. Exact evidence and remaining runtime checks are recorded in [docs/compatibility.md](docs/compatibility.md).

### Register Claude, Codex, or Cursor hooks

Use the state directory used by the Herdr pane. Hooks must write to that same database.

First inspect the generated fragments:

```sh
herdr-infobox --state-dir /absolute/path/to/state adapters install claude --dry-run
herdr-infobox --state-dir /absolute/path/to/state adapters install codex --dry-run
herdr-infobox --state-dir /absolute/path/to/state adapters install cursor --dry-run
```

Without `--config`, installation writes a stable launcher and prints the fragment for you to merge. To merge supported JSON settings and record ownership, supply an explicit file:

```sh
herdr-infobox --state-dir /absolute/path/to/state adapters install claude \
	--config "$HOME/.claude/settings.json"
herdr-infobox --state-dir /absolute/path/to/state adapters install codex \
	--config "${CODEX_HOME:-$HOME/.codex}/hooks.json"
herdr-infobox --state-dir /absolute/path/to/state adapters install cursor \
	--config "$HOME/.cursor/hooks.json"
```

Choose the provider you use. Choose project-scoped settings by passing their path explicitly. Installation preserves unrelated JSON keys and hooks. JSONC and TOML require a manual merge. For Codex, do not register both inline hooks in `config.toml` and `hooks.json` in the same configuration layer. Complete the provider's hook trust review.

Each launcher captures the executable's absolute path and the canonical state path. Register the exact native session before collection, or open Info on a Herdr pane that exposes that identity:

```sh
herdr-infobox --state-dir /absolute/path/to/state session add \
	--provider claude --native-id EXACT_NATIVE_SESSION_ID
```

Hooks accept only enrolled sessions and require an initialized database. A matching cwd does not enroll a session. Collection errors do not block the coding agent. Run `doctor --json` and inspect the actual observations to distinguish registration from working delivery.

### Recover Codex rollout observations

For a supported, uncompressed Codex JSONL rollout, bind the exact file to an enrolled session:

```sh
herdr-infobox session add --provider codex --native-id EXACT_NATIVE_SESSION_ID
herdr-infobox reconcile --session codex:EXACT_NATIVE_SESSION_ID \
	--file /absolute/path/to/rollout.jsonl
```

The reader checks the initial `session_meta` ID against the selected session. It stores a byte cursor for the file and parser version. Complete records are imported transactionally. Partial final records wait for the next read. Each read batch and line is bounded to 8 MiB. Large supported files can need repeated reconciliation. Detectable truncation or file replacement restarts the reader without duplicating committed observations.

Registration survives restarts. The interactive UI refreshes registered sources, and `reconcile --session SESSION` imports changes recorded while Info was closed. Info does not search arbitrary session history to guess a matching file. See the [Codex compatibility notes](docs/compatibility.md#evidence-checked-on-2026-09-28) for unsupported records and rewrite limits.

### Connect OpenCode V2

Enroll the native session and connect to an already running, trusted server:

```sh
herdr-infobox session add --provider opencode --native-id ses_EXACT_NATIVE_ID
herdr-infobox opencode connect --session opencode:ses_EXACT_NATIVE_ID \
	--server http://127.0.0.1:4096
herdr-infobox reconcile --session opencode:ses_EXACT_NATIVE_ID
```

Use `--binary opencode2` or an absolute executable path when the command is not named `opencode`. Executable symlinks, including mise shims, retain their command name. Infobox invokes the CLI with an explicit `--server`; it does not start a server or coding session.

The transport checks the CLI version and `GET /api/info`, then reads `/api/experimental/session/{native_id}/export`. Each CLI call has a five-second deadline. Exports are limited to 16 MiB. The connection is saved only after successful validation and import. Both CLI and server versions must be in the supported set, 2.0.15 and 2.0.18.

Server URLs must use HTTP or HTTPS and contain no credentials, query string, or fragment. The OpenCode CLI handles `OPENCODE_PASSWORD`; infobox does not save the password. Reported repository paths must refer to this host.

When Info is closed, no OpenCode collector runs. Reopening Info or running `reconcile` imports the persisted projected history, including history before compaction. Repeated exports are deduplicated. Changed tool states can add new evidence. Unavailable servers and unsupported exports preserve cached observations.

### Register Cursor transcripts or saved streams

Hooks alone do not collect Cursor's built-in Web and Plan calls in the verified runtime. Register the project transcript for the exact native conversation:

```sh
herdr-infobox session add --provider cursor --native-id EXACT_CONVERSATION_ID
herdr-infobox reconcile --session cursor:EXACT_CONVERSATION_ID \
	--file /absolute/path/agent-transcripts/EXACT_CONVERSATION_ID/EXACT_CONVERSATION_ID.jsonl
```

For project transcripts, both the filename and its parent directory must match the native conversation ID. The transcript does not embed that identity. Do not choose a file by its modification time or workspace path alone.

If you already captured a print-mode `--output-format stream-json` file, register it with the same command. Each record must carry the selected native session ID. Infobox imports an existing capture; it does not start or wrap Cursor.

Cursor files use a bounded full read because the provider can rewrite them in place. The 16 MiB limit applies. Partial final records wait for completion. Repeated imports and rewrites do not duplicate observations. Registered paths refresh in the interactive UI and through `reconcile`.

Durable transcripts provide Web requests with relation `open_requested`, proposed Plan text, and task definitions. Saved streams can provide explicit successful fetches with relation `opened`. Neither route infers missing titles, approval, or execution. Info does not read Cursor's private database. See [the captured Cursor formats](fixtures/cursor/2026.09.26-dd393fe/README.md).

## Use the pane

Info has repository, reference, and Plan sections. Tasks and provider capability diagnostics appear separately. Selecting a session with `s` pins it. Press `p` to resume following the identified agent session in the current tab.

| Key | Action |
| --- | --- |
| `s` | Open the session picker. `Enter` selects and pins a session. |
| `Tab`, `Shift-Tab` | Switch sections. |
| `j`, `k`, arrow keys | Select an item or scroll detail. |
| `Enter` | Open a repository's changed-file list, a selected file's patch, reference provenance, or Plan detail. |
| `d` | Open the selected worktree's changed-file list. |
| `1`, `2`, `3` | Set the Git scope to unstaged, staged, or untracked. Refresh the view after changing scope. |
| `r` | Refresh the selected Git view. |
| `a` | Export the displayed patch and attempt to open copy-only annotation. |
| `p` | Pin or unpin the current session. |
| `/` | Filter the current section. `Enter` or `Esc` ends filter input. |
| `c` | Collapse or expand the current section. |
| `o` | Open a repository URL, reference URL, or available Plan source file. |
| `y` | Copy a worktree path, reference URL, or Plan body through OSC 52. |
| `Esc` | Return from detail or the changed-file list. |
| `q`, `Ctrl-C` | Close the terminal UI. |

The UI's background worker drains the spool, synchronizes connected OpenCode sessions and registered transcripts, discovers pending Git paths, and reads fresh Herdr pane identity. It sleeps two seconds between cycles. Slow subprocess calls extend that interval; two seconds is not a guaranteed refresh deadline. Git views run in separate jobs so pane input remains responsive.

## Understand the displayed information

### Repositories and worktrees

Repository associations come from manual `repo add`, identified Herdr pane directories, or supported provider directory and file-tool observations. Workspace membership and actual file use remain different relations. Git discovery resolves paths, including missing proposed files through their closest existing parent.

A repository is identified by its canonical Git common directory. Linked worktrees share that repository identity but retain separate worktree roots and Git directories. Independent clones stay separate even if their remote URLs match.

GitHub links come from local Git configuration. Remote selection checks `infobox.remote`, the branch upstream remote, `origin`, then a sole remaining remote. Ambiguous remotes have no link. A generated URL does not prove that a branch or commit has been pushed.

For an SSH alias or GitHub Enterprise host, configure an explicit host mapping in the repository:

```sh
git config infobox.remote work
git config --add infobox.github-host 'work-alias=github.example.com'
git config --add infobox.github-host 'github.example.com=github.example.com'
```

Each mapping has `remote-host=web-host` form. These commands change your Git configuration; infobox only reads it. Links remove credentials and one trailing `.git`. See [Git collection](docs/git.md).

### Web references and provenance

References come from supported tool inputs, explicit tool outcomes, structured search records, or `ref add`. Relation labels preserve the evidence, including `search_result`, `open_requested`, `opened`, `citation`, and `manual`.

A request does not prove a successful fetch. Failed fetches keep their reason. Missing titles display `Title unavailable`. Info does not fetch a URL to invent a title. Reference detail includes the collection source and relations. An empty reference list does not prove that the agent used no web tools.

### Plan documents, revisions, and execution

Plan text comes from supported provider Plan records or an explicit `plan attach`. Revisions retain their content and SHA-256 hash. Attaching an edited file under the same Plan key preserves another revision rather than overwriting the old body.

Document phases are `draft`, `proposed`, `approved`, and `superseded`. Execution states are `unknown`, `executing`, `completed`, and `cancelled`. Approval, agent switching, a successful turn, and task completion do not establish Plan execution or completion.

To record a manual execution selection, attach a file and use the returned hash:

```sh
PLAN_REVISION="$(herdr-infobox plan attach --session SESSION \
	--file /absolute/path/to/plan.md --name implementation)"
herdr-infobox plan select --session SESSION --revision "$PLAN_REVISION" --state executing
```

`plan select` accepts an unambiguous revision ID or hash prefix. Identical text under different Plan keys can make a hash selector ambiguous. Keep a stable `--name` when attaching revisions of the same Plan. The command changes the recorded execution selection. It does not approve a provider's Plan or dispatch work. Manual Plan files must be UTF-8 and at most 1 MiB.

Checklists are separate `Tasks` observations. Codex `update_plan` is a checklist, not Plan prose. Cursor transcript Plan bodies lack native document IDs; identical bodies coalesce and edited bodies remain separate observed documents without an inferred revision lineage.

## Inspect and export Git changes

Diffs describe the current local worktree, including edits made by other people or agents. Session association is not change attribution. Collection does not stage, check out, fetch, commit, or push.

| Scope | Comparison | Required arguments |
| --- | --- | --- |
| `unstaged` | Index to worktree. This is the default. | A session and a worktree if the session has more than one. |
| `staged` | HEAD to index, including an unborn HEAD. | A session and optional worktree selection. |
| `untracked` | One selected regular file or symlink without following the symlink. | `--path`. |
| `branch` | Merge-base of `--base` and HEAD to HEAD. | `--base REF`. No automatic fetch occurs. |

Use `ui --once` to see worktree ID prefixes. Prefer a sufficiently long prefix to identify the intended worktree. If a session has exactly one worktree, omit `--worktree`.

```sh
herdr-infobox diff --session SESSION --worktree WORKTREE_ID --scope unstaged
herdr-infobox diff --session SESSION --worktree WORKTREE_ID --scope staged --path src/main.rs
herdr-infobox diff --session SESSION --worktree WORKTREE_ID --scope untracked --path new-file.txt
herdr-infobox diff --session SESSION --worktree WORKTREE_ID --scope branch --base origin/main
herdr-infobox snapshot export --session SESSION --worktree WORKTREE_ID --scope unstaged
herdr-infobox snapshot list
```

The pane supports the first three scopes. Use the CLI for branch comparisons. Conflicts use a labeled combined diff. Git commands disable optional index locks, pagers, color, external diff drivers, text conversion, and fsmonitor. They have a five-second deadline and a 2 MiB output limit. Metadata changes trigger one retry. Continued changes report `Changed while reading`.

### Immutable snapshots and annotation

`snapshot export` prints the retained file's absolute path. The export records the exact patch, comparison commits, worktree, session, timestamp, and hash. Refreshing a diff does not overwrite a snapshot. Non-UTF-8 patches are retained as raw `.patch` files because the Markdown reviewer requires UTF-8.

Press `a` while displaying a patch to export it and open **Info review (copy only)** when the required Annotate installation is available. Info rechecks the target's native session before opening its own review pane. The review wrapper verifies the snapshot hash and removes delivery context before launching plannotator-tui.

Annotation delivery is copy only. Sending feedback to an agent is unavailable. Clipboard behavior depends on OSC 52 support in Herdr and the terminal. Live pane and clipboard integration still require manual verification. Export remains available without Annotate.

Snapshots are not automatically deleted. To remove one, supply its complete 64-character hash:

```sh
herdr-infobox snapshot purge --hash FULL_SNAPSHOT_SHA256
```

Purging a snapshot does not remove reviewer annotations or provider transcripts.

## CLI reference

`--state-dir PATH` is a global option. Commands without a session argument act on the selected state directory. Use `herdr-infobox COMMAND --help` for the installed binary's argument reference.

| Command | Purpose |
| --- | --- |
| `session add --provider PROVIDER --native-id ID [--scope SCOPE]` | Enroll a native session. Providers are `claude`, `codex`, `opencode`, `cursor`, and `devin`. |
| `session list` | List stored UUIDs, provider IDs, scopes, and ended markers. |
| `repo add --session SESSION --path PATH` | Associate a discovered Git worktree and print its ID. |
| `ref add --session SESSION --url URL [--title TITLE]` | Add a manual reference. |
| `plan attach --session SESSION --file FILE [--name NAME]` | Retain a draft revision and print its content hash. The canonical file path is the default Plan key. |
| `plan select --session SESSION --revision REVISION [--state STATE]` | Record an execution selection. The default state is `executing`. |
| `ui [--session SESSION] [--once]` | Open the terminal UI or print one text view. |
| `diff --session SESSION [--worktree ID] [--scope SCOPE] [--path PATH] [--base REF]` | Print a local patch. |
| `snapshot export --session SESSION [--worktree ID] [--scope SCOPE] [--path PATH] [--base REF]` | Retain a diff export. |
| `snapshot list` | List retained Markdown and patch paths. |
| `snapshot purge --hash HASH` | Delete an explicitly selected snapshot. |
| `adapters install PROVIDER [--config PATH] [--dry-run]` | Generate or merge supported provider hooks. |
| `adapters uninstall PROVIDER --config PATH [--dry-run]` | Remove owned hook entries from an explicit configuration. |
| `opencode connect --session SESSION --server URL [--binary PATH]` | Validate and save an OpenCode V2 connection. |
| `opencode disconnect --session SESSION` | Remove that connection while preserving cached history. |
| `reconcile [--session SESSION] [--file PATH]` | Synchronize connections and registered files, replay the spool, and discover pending paths. `--file` requires `--session`. |
| `doctor [--json]` | Report database health, adapter registration, Herdr inspection, annotation availability, connections, and collection loss counters. |
| `ingest --provider PROVIDER` | Read one bounded provider hook payload from stdin. Intended for generated launchers. |
| `toggle`, `ensure` | Herdr action and focus-event entrypoints. They require Herdr connection context. |

## Storage and configuration

State directory selection follows this precedence:

1. Explicit `--state-dir PATH`.
2. `HERDR_PLUGIN_STATE_DIR` supplied by Herdr.
3. `$XDG_STATE_HOME/herdr-infobox`.
4. `$HOME/.local/state/herdr-infobox` when `XDG_STATE_HOME` is unset.

Keep state outside the plugin checkout. The directory is canonicalized and set to mode `0700` on Unix. Separate state directories have separate host IDs, session records, and provider registrations.

| Location | Contents |
| --- | --- |
| `host-id` | Persistent host UUID used in session keys. |
| `infobox.sqlite3` and engine WAL files | Embedded Turso database, schema version 1, normalized events and projections, transcript cursors, and diagnostics. |
| `spool/` | Normalized event batches deferred because the database was busy. Malformed replay files receive an `.invalid` extension. |
| `snapshots/` | Immutable `.md` or `.patch` exports named by hash. |
| `adapters/` | Default launcher and ownership-receipt directory. `HERDR_PLUGIN_CONFIG_DIR` overrides this location. |
| `opencode-SESSION_UUID.json` | Saved server and executable configuration for an enrolled OpenCode session. |
| `panes/`, `ui-HASH.json` | Herdr pane coordination and saved selection or pin state. |
| `schema.lock`, `spool.lock`, `collection-loss-count` | Initialization and spool coordination, and a best-effort loss counter when present. |

`HERDR_BIN_PATH` overrides the Herdr executable. `HERDR_SOCKET_PATH` selects its connection context. Herdr tab and pane variables support pane ownership and following. Standalone `ui --session SESSION` needs no Herdr socket, but Herdr actions and annotation pane creation do.

Raw provider transcripts are not copied wholesale into the database. Normalized records can contain URLs, paths, task text, and Plan bodies. Snapshots contain patch contents. Treat the whole state directory as private project data.

### Persistence and recovery

The database uses embedded Turso with experimental multiprocess WAL, foreign keys, FULL synchronization, and a 25 ms busy timeout. A short-lived hook collector applies an enrolled session's normalized batch transactionally. On database contention, it writes to a bounded spool for later replay.

The spool has a 32 MiB byte limit, a 1,024-entry accounting limit, and a maximum 100 ms lock-reservation wait. Hook stdin is bounded to 8 MiB with a 150 ms read deadline. These bounds do not guarantee an end-to-end latency target or recovery after a full disk or forced termination. Failed hook commands are suppressed so they do not interrupt the agent.

The interactive UI and `reconcile` replay pending batches. Event deduplication prevents repeat replay from duplicating projections. Transcript application and cursor advancement share a transaction. Unknown schemas retain the last committed cursor and report a diagnostic.

### Backup, upgrade, and uninstall

Close all Info panes and collectors before copying the whole state directory. Do not copy only the main database file while WAL writers are active. Do not run `sqlite3`, a SQLite-based viewer, or a mixed SQLite writer against the active Turso database. An engine-aware `VACUUM INTO` backup is described in [local storage](docs/storage.md).

To upgrade, keep a consistent backup, replace the executable and plugin files, and run `doctor --json`. Rerun adapter installation with the same state directory, adapter directory, and explicit settings path to update owned launchers. Edited owned launchers or hook entries require manual resolution. A binary refuses a newer database schema.

To remove explicitly installed hooks, use the same configuration path:

```sh
herdr-infobox --state-dir /absolute/path/to/state adapters uninstall claude \
	--config "$HOME/.claude/settings.json"
herdr-infobox opencode disconnect --session SESSION
```

Uninstallation removes only entries matching the ownership receipt. Edited entries remain and are reported. Remove manually merged fragments yourself. Remove the plugin through Herdr. Keeping the state directory preserves history and snapshots. Provider transcripts and Annotate comments are not deleted. Session and normalized-event expiry are not automatic in the current implementation.

## Troubleshooting

Run diagnostics against the same state directory as the pane and launcher:

```sh
herdr-infobox --state-dir /absolute/path/to/state doctor --json
herdr-infobox --state-dir /absolute/path/to/state session list
herdr-infobox --state-dir /absolute/path/to/state reconcile --session SESSION
herdr-infobox --state-dir /absolute/path/to/state ui --session SESSION --once
```

| Symptom | Check or next step |
| --- | --- |
| `No sessions` | Register the exact native ID or open Info on an identified Herdr agent pane. Verify the state directory. |
| Hook installed but nothing collected | Verify enrollment, the launcher's absolute binary and state paths, provider trust, and the provider's supported payload format. Registration is not execution evidence. |
| Missing repositories | Add the path with `repo add`, then reconcile pending paths. Ensure the path is accessible on this host and belongs to a Git worktree. |
| `GitHub link unavailable` | Inspect remote selection. Add an explicit `infobox.remote` or host mapping for ambiguous or aliased hosts. |
| Empty references or `Plan unavailable` | Check the provider's collection route. Register a supported transcript or OpenCode connection, or use `ref add` and `plan attach`. |
| `Title unavailable` or unknown execution | The source supplied no verified title or execution evidence. Add an explicit title or manual execution selection when needed. |
| `Select --worktree from the session's repository list` | Read worktree IDs with `ui --once` and pass `--worktree`. |
| `Changed while reading` | Retry after concurrent worktree changes stop. The collector does not provide an atomic filesystem snapshot. |
| Transcript rejected or stale | Check native identity, file format, size, and Cursor filename binding. Run explicit `reconcile` for details. Register a relocated source again. |
| OpenCode cannot connect | Check CLI and server versions, executable path, authentication environment, explicit server URL, and export size. Cached data remains available. |
| Annotation unavailable | Check enabled Annotate Full 0.6.0, its `doc` entrypoint, plannotator-tui 0.9.4, and a current matching agent pane. Use `snapshot export` as the fallback. |
| Clipboard does not receive copied text | Check OSC 52 support and forwarding in the terminal and Herdr. Live clipboard behavior is not established by parser tests. |
| `HERDR_SOCKET_PATH` unavailable | Open Info through Herdr for actions, or use standalone `ui --session SESSION` for browsing. |
| Database schema newer than binary | Use the matching executable or a consistent older backup. Do not edit schema metadata to bypass the check. |

`doctor` inspects the installed Herdr version and schema. Inspection does not prove that live pane operations work. Its annotation result also keeps runtime verification separate from version checks.

## Develop and verify changes

Install the pinned tools and run the required checks before opening a pull request:

```sh
mise install --locked
mise run lint
mise run test
mise run build
```

See the [development guide](docs/development.md) for the data flow, source map, test coverage, benchmarks, and packaging. Follow the [contribution guidelines](CONTRIBUTING.md) and [repository rules](AGENTS.md).

## Further documentation

- [Installation and maintenance](docs/installation.md) covers provider setup, upgrade, and uninstall details.
- [Pane usage](docs/usage.md) covers interaction and display semantics.
- [Git collection](docs/git.md) covers repository identity, diff safeguards, and review exports.
- [Local storage](docs/storage.md) covers Turso, backups, spool limits, and retention.
- [Compatibility evidence](docs/compatibility.md) records provider versions, sources, captures, and remaining acceptance work.
- [Development guide](docs/development.md) covers architecture, source files, verification, and packaging.
- [Project scope](docs/project-scope.md) defines implementation and infrastructure boundaries.
- [Contributing](CONTRIBUTING.md) and [repository guidelines](AGENTS.md) define contribution policy.
- [Security reporting](SECURITY.md) describes private vulnerability reporting.
- [Release procedures](docs/releasing.md) and [Securefix configuration](docs/securefix.md) describe repository automation.

## License

[MIT](LICENSE).
