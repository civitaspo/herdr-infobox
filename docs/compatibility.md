# Compatibility evidence

No coding-agent session was started to validate this implementation. Fixtures are anonymized, synthetic examples derived from official documentation and source. They test parsers, not installed CLI compatibility. Each fixture directory records origin, parser revision, source revision where available, and `runtime_tested: false` in `compatibility.json`.

| Provider | Implemented extraction | Limits requiring manual registration or further verification |
| --- | --- | --- |
| Claude Code | Native session, cwd, successful file-tool paths, WebFetch requests and explicit successful HTTP responses, structured WebSearch URL/title groups, ExitPlanMode proposed and approved revisions, TodoWrite/TaskList replacement and TaskGet merge | Live hook delivery is unverified. TaskCreate/TaskUpdate partial updates are not interpreted. Approval does not establish execution. |
| OpenCode V1 | Native session/call ID, queried session directory, successful webfetch URL, exact source-derived Git-project Plan path | Bridge SDK is pinned to 1.18.32. Live delivery and V2 are unverified. Search-engine outputs and non-Git Plan paths are unavailable. Tool titles are not page titles. |
| Codex CLI | Native session/cwd, update_plan checklist, explicit source-derived completed Plan/WebSearch JSONL records | Hosted WebSearch bypasses hooks. Rollout is unstable. Compressed/paginated storage and extension WebSearch are unsupported. Checklist is not Plan prose. |
| Cursor CLI | Conversation identity, cwd, workspace roots, validation of JSON-stringified generic tool/MCP output | Built-in Web and Plan payloads and normal CLI hook delivery are unverified. No guessed URL or Plan extraction. ACP is a separate runtime. |
| Devin CLI | Stable native session, project root from DEVIN_PROJECT_DIR, conservative capability state | Tool argument schemas are unverified. No automatic Web, Plan, or checklist facts are inferred. |

A feature marked partial is not evidence that the provider emitted every event. Empty references must not be interpreted as proof that the agent made no web requests. Manual `repo add`, `ref add`, and `plan attach` remain available for every provider.

## Evidence checked on 2026-09-28

[Claude hooks](https://code.claude.com/docs/en/hooks) specify injected ExitPlanMode input `plan`/`planFilePath` and approved response `plan`/`filePath`. The decoder uses response text rather than rereading an approved file. The official [Claude Agent SDK 0.3.283 package](https://registry.npmjs.org/@anthropic-ai/claude-agent-sdk/-/claude-agent-sdk-0.3.283.tgz), `package/sdk-tools.d.ts`, specifies structured WebSearch groups, WebFetch HTTP status, and checklist result types. The package integrity is recorded in the fixture metadata. SDK source is not a guarantee that every Claude CLI release emits the same payload.

[Codex hooks](https://learn.chatgpt.com/docs/hooks) explicitly exclude hosted WebSearch and warn that transcript format is unstable. Source commit [`88235f881d4e222cf779df785e747d8c8b935768`](https://github.com/openai/codex/tree/88235f881d4e222cf779df785e747d8c8b935768) provides the completed item and checklist types. The reader accepts newline-complete JSONL records only, verifies the initial session metadata, keeps a byte cursor per session/file/parser, and restarts after detectable truncation or inode replacement. Unknown records report their byte offset. Concurrent rewrite to identical inode and equal or greater size is not completely detectable. Raw transcripts are not copied to the database.

[OpenCode plugin types](https://github.com/anomalyco/opencode/blob/b471c2b4495747353af768fbf2e0790c9d820ce2/packages/plugin/src/index.ts), [session Plan path](https://github.com/anomalyco/opencode/blob/b471c2b4495747353af768fbf2e0790c9d820ce2/packages/opencode/src/session/session.ts), and [plan_exit](https://github.com/anomalyco/opencode/blob/b471c2b4495747353af768fbf2e0790c9d820ce2/packages/opencode/src/tool/plan.ts) establish the V1 bridge contract. The bridge queries session directory rather than assigning startup directory to every session. It reads a Plan only when session project and worktree match, the runtime supplies a valid slug and creation time, and the file is a bounded regular file. Missing Plan evidence does not stop other panels.

[Cursor hooks](https://cursor.com/docs/hooks) define stable `conversation_id`, turn-specific `generation_id`, nullable transcript path, and JSON-stringified results. Workspace roots record workspace membership only. [Devin lifecycle hooks](https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks) define `tool_response.success`; [Devin hook configuration](https://docs.devin.ai/cli/extensibility/hooks/overview) also loads Claude hooks by default. A Claude-configured collector with `DEVIN_PROJECT_DIR` is classified as Devin rather than storing events under the wrong provider.

## OpenCode bridge setup

The plugin exports `createInfobox(binary, state)`. Both arguments must be absolute paths. A generated local wrapper can export `createInfobox("/absolute/herdr-infobox", "/absolute/state")`. The optional `Infobox` export instead reads `HERDR_INFOBOX_BIN` and `HERDR_INFOBOX_STATE_DIR`. A missing or relative path disables collection safely. The bridge uses a 75 ms session lookup deadline and a 75 ms child-process deadline. These are bounds, not measured p95 results. Existing OpenCode configuration must be preserved when adding the wrapper's file URL.

## Remaining manual verification

With explicit approval, capture anonymized payloads from each target CLI version, verify hooks while the UI is closed, resume the same session, run a second session in the same repository, and confirm multi-directory OpenCode behavior. Verify Codex hosted and standalone search separately. Check Cursor normal interactive CLI independently from ACP. Record binary versions and replace runtime-unverified status only after those checks pass. Do not start credential-consuming agent sessions or edit live hooks merely to satisfy a fixture test.
