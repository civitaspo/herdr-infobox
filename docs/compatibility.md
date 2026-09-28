# Compatibility evidence

No coding-agent session was started to validate this implementation. Fixtures are anonymized, synthetic examples derived from official documentation and source. They test parsers, not installed CLI compatibility. Each fixture directory records origin, parser revision, source revision where available, and `runtime_tested: false` in `compatibility.json`.

| Provider | Implemented extraction | Limits requiring manual registration or further verification |
| --- | --- | --- |
| Claude Code | Native session, cwd, successful file-tool paths, WebFetch requests and explicit successful HTTP responses, structured WebSearch URL/title groups, ExitPlanMode proposed and approved revisions, TodoWrite/TaskList replacement and TaskGet merge | Live hook delivery is unverified. TaskCreate/TaskUpdate partial updates are not interpreted. Approval does not establish execution. |
| OpenCode V2 2.0.18 | Native session and parent, current/historical directories and file-tool paths, successful/failed webfetch observations, completed Plan-agent prose as proposed documents | Rust reads the full projected export from an explicit existing server. V1 is unsupported. Page titles, search results, checklists, Plan approval and execution are unavailable. Live coding-agent sessions remain unverified. |
| Codex CLI | Native session/cwd, update_plan checklist, explicit source-derived completed Plan/WebSearch JSONL records | Hosted WebSearch bypasses hooks. Rollout is unstable. Compressed/paginated storage and extension WebSearch are unsupported. Checklist is not Plan prose. |
| Cursor CLI | Conversation identity, cwd, workspace roots, validation of JSON-stringified generic tool/MCP output | Built-in Web and Plan payloads and normal CLI hook delivery are unverified. No guessed URL or Plan extraction. ACP is a separate runtime. |
| Devin CLI | Stable native session, project root from DEVIN_PROJECT_DIR, conservative capability state | Tool argument schemas are unverified. No automatic Web, Plan, or checklist facts are inferred. |

A feature marked partial is not evidence that the provider emitted every event. Empty references must not be interpreted as proof that the agent made no web requests. Manual `repo add`, `ref add`, and `plan attach` remain available for every provider.

## Evidence checked on 2026-09-28

[Claude hooks](https://code.claude.com/docs/en/hooks) specify injected ExitPlanMode input `plan`/`planFilePath` and approved response `plan`/`filePath`. The decoder uses response text rather than rereading an approved file. The official [Claude Agent SDK 0.3.283 package](https://registry.npmjs.org/@anthropic-ai/claude-agent-sdk/-/claude-agent-sdk-0.3.283.tgz), `package/sdk-tools.d.ts`, specifies structured WebSearch groups, WebFetch HTTP status, and checklist result types. The package integrity is recorded in the fixture metadata. SDK source is not a guarantee that every Claude CLI release emits the same payload.

[Codex hooks](https://learn.chatgpt.com/docs/hooks) explicitly exclude hosted WebSearch and warn that transcript format is unstable. Source commit [`88235f881d4e222cf779df785e747d8c8b935768`](https://github.com/openai/codex/tree/88235f881d4e222cf779df785e747d8c8b935768) provides the completed item and checklist types. The reader accepts newline-complete JSONL records only, verifies the initial session metadata, keeps a byte cursor per session/file/parser, and restarts after detectable truncation or inode replacement. Unknown records report their byte offset. Concurrent rewrite to identical inode and equal or greater size is not completely detectable. Raw transcripts are not copied to the database.

OpenCode is pinned to the released [v2.0.18 source](https://github.com/anomalyco/opencode/tree/cd9a14a6b688d4021bee381dfd39d2cef9c0f862), rather than the V1 default branch.
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

## Remaining manual verification

With explicit approval, capture anonymized payloads from each target CLI version, verify hooks while the UI is closed, resume the same session, run a second session in the same repository, and confirm multi-directory OpenCode behavior. Verify Codex hosted and standalone search separately. Check Cursor normal interactive CLI independently from ACP. Record binary versions and replace runtime-unverified status only after those checks pass. Do not start credential-consuming agent sessions or edit live hooks merely to satisfy a fixture test.
