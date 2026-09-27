# Provider contracts checked on 2026-09-28

Evidence is official documentation and public source only. No real agent session, credentials, live hooks, private transcript, or installed-provider runtime was exercised. All proposed fixtures are synthetic, documentation-derived fixtures, not captured compatibility evidence. Do not advertise a minimum runtime version from these findings.

## Early Codex and Cursor feasibility result

Normal CLI observation is possible only partially. Codex hosted WebSearch explicitly bypasses hooks. Cursor generic hooks document fields but do not establish the exact built-in Web/Plan payload on the normal interactive CLI. Keep manual references and plan attachment operational and show those limitations. Starting a new app-server or ACP process is not a verified observer for an existing CLI session.

## Codex

[Current hook docs](https://learn.chatgpt.com/docs/hooks) establish common `session_id`, nullable `transcript_path`, `cwd`, `hook_event_name`, `model`; turn hooks also carry `turn_id`. Subagent hooks use the parent session ID, so native subagent scope must remain distinct. PostToolUse adds `tool_name`, `tool_use_id`, `tool_input`, `tool_response`. Bash may exit nonzero and still produce PostToolUse. Bash/apply_patch inputs use `command`. Local `update_plan` is covered, hosted WebSearch is not. Transcript format is explicitly unstable.

Source HEAD resolved to `88235f881d4e222cf779df785e747d8c8b935768`, newer than the design's revision. [items.rs](https://github.com/openai/codex/blob/88235f881d4e222cf779df785e747d8c8b935768/codex-rs/protocol/src/items.rs) defines `PlanItem { id, text }` and `WebSearchItem { id, query, action, results: Option<Vec<JsonValue>> }`. [WebSearch test](https://github.com/openai/codex/blob/88235f881d4e222cf779df785e747d8c8b935768/codex-rs/app-server/tests/suite/v2/web_search.rs) uses `results` entries with `type: text_result`, `ref_id`, `url`, and `title`. This is source evidence, not proof every installed CLI writes this shape.

[plan_tool.rs](https://github.com/openai/codex/blob/88235f881d4e222cf779df785e747d8c8b935768/codex-rs/protocol/src/plan_tool.rs) defines `UpdatePlanArgs { explanation?: string, plan: [{step, status}] }`, statuses `pending`, `in_progress`, `completed`. Explicitly a checklist, not Plan mode prose. [rollout policy](https://github.com/openai/codex/blob/88235f881d4e222cf779df785e747d8c8b935768/codex-rs/rollout/src/policy.rs) stores completed Plan items in legacy and paginated modes; paginated stores completed TurnItems whereas legacy stores WebSearchEnd. PlanDelta and PlanUpdate are not durable event sources under this policy.

Minimal parser should support hook identity/cwd/checklist, known versioned completed transcript items only, and distinguish source envelope format. Do not interpret any nested URL/string as a result, any `update_plan` as plan approval, any absent `results` as no references. Unknown rollout format gets a diagnostic, not an empty successful reconciliation.

## Cursor

[Hooks](https://cursor.com/docs/hooks) define `conversation_id` as stable across turns; `generation_id` changes with messages. Common fields include `workspace_roots`, `cursor_version`, nullable `transcript_path`. `preToolUse`/`postToolUse` examples provide `tool_name`, `tool_input`, `tool_use_id`, `cwd`. PostToolUse `tool_output` is a JSON-stringified result, requiring a second JSON parse. `afterMCPExecution.result_json` is also JSON text. Generic successful and failed hooks are distinct. Workspace roots are workspace membership, not observed edits.

Docs discuss Agent/Cmd+K hooks; exact normal CLI built-in Web and Plan fields were not verified. Do not implement a guessed `WebSearch.results` or `CreatePlan.plan` normal-CLI schema. Use explicit native conversation identity, observed paths from documented file hook contracts, workspace relations, and manual fallback. Reject malformed inner JSON as unknown output. ACP `cursor/create_plan` and `cursor/update_todos` belong to the ACP runtime and cannot establish normal CLI behavior. Cursor binary version remains unverified.

## Claude Code

[Hook reference](https://code.claude.com/docs/en/hooks) documents `session_id`, `cwd`, `transcript_path`, `hook_event_name`, tool `tool_use_id`, `tool_input` and `tool_response`. ExitPlanMode PreToolUse receives injected `tool_input.plan` and `planFilePath`; PostToolUse receives approved `tool_response.plan` and `filePath`. Preserve response text rather than rereading a mutable plan file. Pre is proposed and post is approved, neither proves execution. Docs mention a v2.1.205 change to deprecated `allowedPrompts`, not a minimum version for all these contracts.

WebFetch input has `url` and `prompt`. WebSearch input has `query` but the full structured search response schema is not fixed in these hook docs. A parser claiming automatic Claude WebSearch titles from invented response fixture fields would exceed evidence. A WebFetch request can still provide the reference URL with title unavailable, allowing all four information panels to work from one provider without unsupported title claims. Failed fetch remains a request or failure, not opened. Task/Todo updates remain separate from plan prose.

## OpenCode

HEAD still equals design revision `b471c2b4495747353af768fbf2e0790c9d820ce2`. [V1 type definition](https://github.com/anomalyco/opencode/blob/b471c2b4495747353af768fbf2e0790c9d820ce2/packages/plugin/src/index.ts) defines `tool.execute.after(input: {tool,sessionID,callID,args}, output: {title,output,metadata})`. [Plugin docs](https://opencode.ai/docs/plugins/) describe plugin events. Do not use plugin startup directory for every session, query the corresponding session context.

[webfetch.ts](https://github.com/anomalyco/opencode/blob/b471c2b4495747353af768fbf2e0790c9d820ce2/packages/opencode/src/tool/webfetch.ts) validates `args.url`; output `title` is URL plus content type, not page title. Thus reference title stays unavailable. [session.ts](https://github.com/anomalyco/opencode/blob/b471c2b4495747353af768fbf2e0790c9d820ce2/packages/opencode/src/session/session.ts) function `plan` builds `<worktree>/.opencode/plans/<time.created>-<slug>.md` for Git projects and data/plans otherwise. Read only the exact session-associated path. [plan.ts](https://github.com/anomalyco/opencode/blob/b471c2b4495747353af768fbf2e0790c9d820ce2/packages/opencode/src/tool/plan.ts) `plan_exit` asks before switching to build agent; rejection fails. A before event does not prove approval. Successful after output can establish transition, but exact plan revision must be captured from that session's path with change detection.

V2 is a separate interface. This research did not verify its executable SDK types; mark V2 unsupported rather than claiming V1 hooks cover it. A small V1 bridge should send known args/output, native session/call IDs, and queried session cwd as an infobox-defined envelope. Catch collector failures and never mutate tool objects.

## Devin

[Lifecycle docs](https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks) establish stable `session_id`, turn `prompt_id`, tool `tool_name`, `tool_input`, and PostToolUse `tool_response {success:boolean, output:string, error:string|null}`. Documented tool names include `webfetch`, `todo_write`, `exit_plan_mode`, `read`, `write`, and `edit`, but their input schemas are not specified. Do not assume `path`, `url`, or `plan` keys solely from their names.

[Overview](https://docs.devin.ai/cli/extensibility/hooks/overview) documents `DEVIN_PROJECT_DIR` and loading Claude hooks by default via `read_config_from.claude`. Therefore a command registered as Claude must detect Devin environment before assigning provider identity. Session metadata and project directory can be supported conservatively. Automatic Web/Plan remains unavailable until verified payloads arrive. Do not deduplicate on prompt ID plus tool name because a turn may invoke a tool twice.

## Fixture and adapter recommendation

Use provider + fixture schema revision + evidence URL + checked date metadata. Label fixtures synthetic docs-derived and runtime_tested false. Suggested tests include Claude Pre/Post ExitPlanMode changed text; Claude WebFetch requested versus failed; Codex update_plan separate from plan document; Codex nullable transcript and hosted web limitation; Cursor malformed JSON-stringified tool_output; Devin unsuccessful PostToolUse and repeated same-tool calls in one prompt; OpenCode two session directories, successful webfetch without page title, and rejection of unknown envelope version. Persist ingress UUID once for events without native IDs; retain tool ID/phase for hook-transcript reconciliation.

Unknown tool payload should retain a bounded diagnostic and session/cwd observation but produce no guessed reference, plan, approval, or completion. Provider capability text should distinguish fixture-tested parsing from runtime verification. Manual registration supplies a useful fallback while respecting this boundary.

## Subsequent implementation evidence

The initial Claude schema gaps above were narrowed by inspecting the official npm package `@anthropic-ai/claude-agent-sdk` version `0.3.283`, file `package/sdk-tools.d.ts`, from https://registry.npmjs.org/@anthropic-ai/claude-agent-sdk/-/claude-agent-sdk-0.3.283.tgz. Package integrity is `sha512-KB+mqU5JLbH2sztlSQeCOu71bK6padYAha3uacBzxFSOVfuRTywYzvsC9P+qV6gXmPXcu98FaPqQv6vBF9j8hA==`. It defines WebSearchOutput.results as an array of string commentary or objects containing tool_use_id and content[{title,url}]. WebFetchOutput carries code and url, so successful fetch requires explicit 2xx status. TodoWriteOutput.newTodos gives the replaced checklist. TaskListOutput.tasks and TaskGetOutput.task provide complete task values suitable for replace/merge. Those exact schemas are implemented with synthetic fixtures; live CLI compatibility remains unverified. TaskCreate/TaskUpdate partial output and provider runtime version claims remain deferred.
