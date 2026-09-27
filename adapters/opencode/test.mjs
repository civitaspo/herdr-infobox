import assert from "node:assert/strict";
import childProcess from "node:child_process";
import { EventEmitter } from "node:events";
import { syncBuiltinESMExports } from "node:module";

const calls = [];
childProcess.spawn = (binary, args) => {
  const child = new EventEmitter();
  child.kill = () => {};
  child.stdin = new EventEmitter();
  child.stdin.end = (payload) => {
    calls.push({ binary, args, payload: JSON.parse(payload) });
    queueMicrotask(() => child.emit("close", 0));
  };
  return child;
};
syncBuiltinESMExports();
const { createInfobox } = await import("./index.ts");
const plugin = await createInfobox("/example/infobox", "/example/state")({
  client: { session: { get: async ({ path }) => ({ data: { id: path.id, directory: "/example/second", projectID: "project" } }) } },
  project: { id: "project" }, worktree: "/example/first",
});
const input = { tool: "webfetch", sessionID: "session-2", callID: "call-1", args: { url: "https://example.com" } };
const output = { title: "not a page title", output: "body", metadata: {} };
await plugin["tool.execute.after"](input, output);
assert.equal(calls.length, 1);
assert.equal(calls[0].payload.directory, "/example/second");
assert.equal(calls[0].payload.sessionID, "session-2");
assert.deepEqual(calls[0].args, ["--state-dir", "/example/state", "ingest", "--provider", "opencode"]);
assert.deepEqual(output, { title: "not a page title", output: "body", metadata: {} });
const unavailable = await createInfobox("/example/infobox", "/example/state")({
  client: { session: { get: async () => { throw new Error("offline"); } } }, project: {}, worktree: "/example",
});
await unavailable["tool.execute.after"](input, output);
assert.equal(calls.length, 1);
console.log("OpenCode bridge unit tests passed");
