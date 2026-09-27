import { spawn } from "node:child_process";
import { open } from "node:fs/promises";
import { constants } from "node:fs";
import { isAbsolute, join, relative, sep } from "node:path";
import type { Plugin } from "@opencode-ai/plugin";

const limit = 1024 * 1024;

export function createInfobox(binary: string, state: string): Plugin {
  return async ({ client, project, worktree }) => ({
  "tool.execute.after": async (input, output) => {
    if (!isAbsolute(binary) || !isAbsolute(state)) return;
    try {
      const response = await client.session.get({
        path: { id: input.sessionID },
        signal: AbortSignal.timeout(75),
      });
      const session = response.data;
      if (!session || session.id !== input.sessionID || !session.directory) return;
      let plan: { path: string; markdown: string } | undefined;
      const slug = "slug" in session && typeof session.slug === "string" ? session.slug : undefined;
      const within = relative(worktree, session.directory);
      try {
      if (input.tool === "plan_exit" && session.projectID === project.id &&
          !isAbsolute(within) && within !== ".." && !within.startsWith(`..${sep}`) &&
          slug && /^[a-zA-Z0-9_-]+$/.test(slug) && Number.isSafeInteger(session.time.created)) {
        const path = join(worktree, ".opencode", "plans", `${session.time.created}-${slug}.md`);
        const file = await open(path, constants.O_RDONLY | constants.O_NONBLOCK | constants.O_NOFOLLOW);
        try {
          const before = await file.stat();
          if (before.isFile() && before.size <= limit) {
            const bytes = Buffer.alloc(before.size);
            const { bytesRead } = await file.read(bytes, 0, bytes.length, 0);
            const after = await file.stat();
            if (bytesRead === before.size && before.size === after.size && before.mtimeMs === after.mtimeMs) {
              plan = { path, markdown: bytes.toString("utf8") };
            }
          }
        } finally {
          await file.close();
        }
      }
      } catch {
      }
      const payload = JSON.stringify({
        infobox_schema: 1,
        event: "tool.execute.after",
        ...input,
        directory: session.directory,
        worktree,
        output,
        plan,
      });
      if (Buffer.byteLength(payload) > limit) return;
      await new Promise<void>((resolve) => {
        const child = spawn(binary, ["--state-dir", state, "ingest", "--provider", "opencode"], {
          stdio: ["pipe", "ignore", "ignore"],
        });
        const timer = setTimeout(() => { child.kill(); resolve(); }, 75);
        child.on("error", () => { clearTimeout(timer); resolve(); });
        child.on("close", () => { clearTimeout(timer); resolve(); });
        child.stdin.on("error", () => {});
        child.stdin.end(payload);
      });
    } catch {}
  },
});

}

export const Infobox = createInfobox(process.env.HERDR_INFOBOX_BIN ?? "", process.env.HERDR_INFOBOX_STATE_DIR ?? "");
