import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { performance } from "node:perf_hooks";
import { createInfobox } from "./index.ts";

const binary = resolve(process.argv[2] ?? "target/debug/herdr-infobox");
const state = await mkdtemp(join(tmpdir(), "infobox-bridge-integration-"));
function run(args) {
  const result = spawnSync(binary, ["--state-dir", state, ...args], { encoding: "utf8", timeout: 5000 });
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stderr);
  return result.stdout;
}
try {
  run(["session", "add", "--provider", "opencode", "--native-id", "bridge-integration"]);
  const plugin = await createInfobox(binary, state)({
    client: { session: { get: async () => ({ data: { id: "bridge-integration", directory: state, projectID: "fixture-project" } }) } },
    project: { id: "fixture-project" }, worktree: state,
  });
  const started = performance.now();
  await plugin["tool.execute.after"](
    { tool: "webfetch", sessionID: "bridge-integration", callID: "fetch-example", args: { url: "https://example.com/bridge-integration" } },
    { title: "This is a tool title", output: "Synthetic fixture body", metadata: {} },
  );
  const elapsed = performance.now() - started;
  const view = run(["ui", "--session", "bridge-integration", "--once"]);
  assert.match(view, /https:\/\/example.com\/bridge-integration/);
  assert.match(view, /Title unavailable/);
  assert.doesNotMatch(view, /This is a tool title/);
  console.log(`Bridge transport passed with mocked session API and real collector (${elapsed.toFixed(1)} ms, one observation).`);
} finally {
  await rm(state, { recursive: true, force: true });
}
