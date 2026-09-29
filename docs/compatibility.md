# Compatibility evidence

Compatibility is specific to the provider version and collection route. Fixture
directories distinguish anonymized live captures from synthetic examples and
record their coverage in `compatibility.json`. Parser tests alone do not establish
installed CLI compatibility.

| Provider | Implemented extraction | Limits requiring manual registration or further verification |
| --- | --- | --- |
| Claude Code | Native session, cwd, successful file-tool paths, WebFetch requests and explicit successful HTTP responses, structured WebSearch URL/title groups, ExitPlanMode proposed and approved revisions, TodoWrite/TaskList replacement and TaskGet merge | Live hook delivery is unverified. TaskCreate/TaskUpdate partial updates are not interpreted. Approval does not establish execution. |
| OpenCode V2 2.0.15 and 2.0.18 | Native session and parent, current/historical directories and file-tool paths, successful/failed webfetch observations, completed Plan-agent prose as proposed documents | 2.0.15 was tested with real Plan and build turns, resume, and reconciliation. 2.0.18 is source- and fixture-tested. Page titles, search results, checklists, Plan approval and execution are unavailable. |
| Codex CLI | Native session/cwd, update_plan checklist, explicit source-derived completed Plan/WebSearch JSONL records | Hosted WebSearch bypasses hooks. Rollout is unstable. Compressed/paginated storage and extension WebSearch are unsupported. Checklist is not Plan prose. |
| Cursor CLI 2026.09.26-dd393fe | Hook identity/workspace roots; registered JSONL Web requests, proposed Plan text and task definitions; saved stream-json explicit successful WebFetch results | Real print and interactive Plan runs were tested. Durable transcripts omit tool results, titles, approval and execution. Saved streams require capture by the caller. ACP is a separate runtime. |
| Devin CLI | Stable native session, project root from DEVIN_PROJECT_DIR, conservative capability state | Tool argument schemas are unverified. No automatic Web, Plan, or checklist facts are inferred. |

A feature marked partial is not evidence that the provider emitted every event. Empty references must not be interpreted as proof that the agent made no web requests. Manual `repo add`, `ref add`, and `plan attach` remain available for every provider.

## Evidence checked on 2026-09-28

[Claude hooks](https://code.claude.com/docs/en/hooks) specify injected ExitPlanMode input `plan`/`planFilePath` and approved response `plan`/`filePath`. The decoder uses response text rather than rereading an approved file. The official [Claude Agent SDK 0.3.283 package](https://registry.npmjs.org/@anthropic-ai/claude-agent-sdk/-/claude-agent-sdk-0.3.283.tgz), `package/sdk-tools.d.ts`, specifies structured WebSearch groups, WebFetch HTTP status, and checklist result types. The package integrity is recorded in the fixture metadata. SDK source is not a guarantee that every Claude CLI release emits the same payload.

[Codex hooks](https://learn.chatgpt.com/docs/hooks) explicitly exclude hosted WebSearch and warn that transcript format is unstable. Source commit [`88235f881d4e222cf779df785e747d8c8b935768`](https://github.com/openai/codex/tree/88235f881d4e222cf779df785e747d8c8b935768) provides the completed item and checklist types. The reader accepts newline-complete JSONL records only, verifies the initial session metadata, keeps a byte cursor per session/file/parser, and restarts after detectable truncation or inode replacement. Unknown records report their byte offset. Concurrent rewrite to identical inode and equal or greater size is not completely detectable. Raw transcripts are not copied to the database.

OpenCode accepts the verified contracts in [v2.0.15](https://github.com/anomalyco/opencode/tree/6f3639d82ed0760091792189b78f8eeb44f699b1) and [v2.0.18](https://github.com/anomalyco/opencode/tree/cd9a14a6b688d4021bee381dfd39d2cef9c0f862).
The [session export API](https://github.com/anomalyco/opencode/blob/cd9a14a6b688d4021bee381dfd39d2cef9c0f862/packages/protocol/src/groups/session.ts) returns the full projected transcript.
The [CLI connection implementation](https://github.com/anomalyco/opencode/blob/cd9a14a6b688d4021bee381dfd39d2cef9c0f862/packages/cli/src/services/server-connection.ts) uses an existing server when `--server` is explicit. Infobox always supplies it and never uses standalone mode.
The [message schema](https://github.com/anomalyco/opencode/blob/cd9a14a6b688d4021bee381dfd39d2cef9c0f862/packages/schema/src/session-message.ts) distinguishes running, completed, and failed tools.
The [Plan plugin](https://github.com/anomalyco/opencode/blob/cd9a14a6b688d4021bee381dfd39d2cef9c0f862/packages/core/src/plugin/plan.ts) primarily discusses plans in conversation. V1 filename and plan_exit assumptions have been removed.

[Cursor hooks](https://cursor.com/docs/hooks) define stable `conversation_id`, turn-specific `generation_id`, nullable transcript path, and JSON-stringified results. Workspace roots record workspace membership only. [Devin lifecycle hooks](https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks) define `tool_response.success`; [Devin hook configuration](https://docs.devin.ai/cli/extensibility/hooks/overview) also loads Claude hooks by default. A Claude-configured collector with `DEVIN_PROJECT_DIR` is classified as Devin rather than storing events under the wrong provider.

The released macOS arm64 OpenCode 2.0.18 executable was also tested against an
ephemeral local HTTP fixture server. This verified the actual version output,
explicit-server health checks, and raw export response end to end. The archive
SHA-1 matched npm metadata (`9f0c66abd02c4acffb0d998deb24db420adc0b04`).
Child HOME/XDG directories were isolated. No coding-agent session, model request,
or provider configuration change was involved.

## OpenCode V2 collection


See [connection instructions](installation.md#connect-opencode-v2). Rust validates the CLI/server version and export before persistence. No provider configuration is changed. Full export reconciliation was chosen over a TypeScript hook bridge so recovery also covers periods when the pane is closed. Limits are explicit; no missing metadata is converted into successful fetches, approval, or execution.

## Runtime evidence on 2026-09-29

OpenCode 2.0.15 ran against an isolated local server. A Plan-agent turn read a
file, fetched a public page, and proposed a Plan. A resumed build-agent turn
edited the file while infobox was closed. Reconciliation displayed the GitHub
link, exact unstaged diff, opened URL, and proposed Plan with unknown execution.
Repeated collection preserved counts and Git index/worktree hashes. The actual
absolute mise shim also connected. The captured export includes terminal `idle`
records, which do not imply Plan completion. See
[the anonymized fixture](../fixtures/opencode/v2.0.15/README.md).

Cursor print-mode runs exercised Read, Write, WebFetch, CreatePlan, and resume.
Hooks delivered workspace roots and file tools but omitted the built-in Web and
Plan calls. The registered project JSONL recovered their inputs while the UI was
closed. A later interactive Plan run produced another proposed document without
approval. Saved print streams supplied explicit fetch success. Both collection
routes share content-addressed Plan documents and keep task definitions separate.
See [the captured formats](../fixtures/cursor/2026.09.26-dd393fe/README.md).

Cursor transcripts contain no native Plan document or tool-call IDs. Identical
Plan bodies coalesce; edited bodies remain separate observed documents without
an inferred revision lineage. Missing task statuses stay unknown and do not
overwrite known statuses. No exact cross-source Web call count is inferred.
The reader uses complete records and a bounded full read because Cursor can
rewrite a transcript in place. Unknown records identify their line, and cached
observations survive unavailable files. Cursor's private database is not read.

In this runtime, `/tmp` and its canonical `/private/tmp` spelling produced
different Cursor project transcript directories. A relocated source needs an
explicit registration for the new path. Infobox does not scan other sessions
to guess a replacement. Neither provider's live settings were modified.

## Remaining manual verification

Live multi-repository and linked-worktree association, concurrent sessions,
Herdr pane controls, and annotate interaction remain to be checked. Cursor
interactive Web collection was not separately exercised; its tested interactive
operation was Plan creation. Verify Codex hosted and standalone search and the
remaining providers independently. Record exact versions and collection routes.
Credential-consuming sessions and live hook edits require explicit authorization.
