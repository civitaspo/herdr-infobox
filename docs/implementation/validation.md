# Validation and remaining acceptance

Historical validation record. OpenCode V1 and its Node bridge were subsequently removed; see [current compatibility](../compatibility.md).

Validation uses temporary state, temporary Git repositories, synthetic versioned provider fixtures, and fake Herdr/reviewer processes. It does not establish live provider compatibility. No credential-consuming coding-agent session, live hook edit, or release publication was performed.

## Executed on macOS

`mise run lint`, `mise run test`, and `mise run build` passed after migration to Turso 0.7.2. The Rust suites contain 52 tests. Lint includes rustfmt, Clippy with warnings denied, ShellCheck, and existing workflow policy checks.

| Design area | Evidence |
| --- | --- |
| Session and persistence | Native identity/scope isolation, same-session resume, atomic batch rollback, cursor compare-and-swap, concurrent first-run host identity, ordered revision selection in tests/core.rs |
| End-to-end | Actual collector CLI, two repositories, duplicate Claude fixtures, restart, references, Plan revisions, retained diff in tests/end_to_end.rs |
| Multiprocess storage | Sixteen actual hook subprocesses while another connection remains open; replay and reopen in tests/multiprocess.rs |
| Git | Nested repositories, absorbed submodules, moved linked-worktree rediscovery, staged/unstaged/untracked, unborn HEAD, rename, binary, symlink, conflict, executable mode, branch commit IDs, remote mapping, hostile inherited environment, output/timeout bounds, disabled external drivers |
| Snapshot/reviewer | Exact retained patch and metadata, safe Markdown fences, raw byte fallback, integrity checks, copy-only launch, reused session rejection, snapshot lifecycle |
| Herdr | Fake API tests for pane reuse, plugin focus, toggle/ensure, lost creation response and UI registration, installation discovery |
| Provider parsers | Explicit success/approval evidence, missing status, unknown shapes, task merge/replace, incomplete JSONL and truncation, session guards, Devin imported-hook classification |
| Configuration | Foreign/edited hook preservation, JSONC refusal, parsed Codex inline-hook conflict detection |
| Diagnostics | Held-open stdin returns silently, doctor avoids private content, unsupported capabilities remain explicit |

The independent correctness passes found and corrected Git environment contamination, untracked executable mode, Codex inline-table detection, orphan pane creation, explicit session fallback, cross-instance pin state, Plan open/copy routing, raw snapshot lifecycle, title precedence, phase regression, and source-ordered execution selection. The Turso pass separately verified rollback and enrolled-session-only busy spooling. Concurrent spool pressure exposed a lock held across fsync; reservation now releases the lock before the write.

`node --experimental-strip-types adapters/opencode/integration.mjs target/debug/herdr-infobox` passed against Turso with a mocked session lookup and the actual collector. This proves local bridge transport, not OpenCode runtime delivery.

The original Python PTY probe (now replaced by `tests/ui_runtime.rs`) passed: a 30-column PTY copies selected Plan text, opens its canonical source through a fake opener, and keeps pins separate across fake Herdr instances. The [collector benchmark](collector-performance.md) measured 100 sequential debug processes at p95 17.838 ms; this does not establish live-provider or sustained-load latency.

## Unverified and deliberately unavailable

The installed local Herdr is 0.8.2, below the 0.9.1 manifest minimum. Live plugin installation, actual Herdr restart, clipboard programs, herdr-annotate UI, and real provider hook delivery remain manual checks. The fake API verifies contracts but cannot substitute for those checks. All provider fixture metadata keeps `runtime_tested: false`.

Linux execution, power-loss/crash durability, filesystem disk-full recovery, automatic persisted-identity reconciliation after worktree relocation, edits during capture, and sustained p95/observation-to-display measurements need further acceptance runs. Non-UTF-8 path bytes are fixture-tested; this host's filesystem refused creating an invalid-UTF-8 filename, so the real-file case remains for Linux.

Codex compressed/paginated rollouts and extension search, Cursor built-in Web/Plan output, OpenCode V2, and Devin tool payloads have no verified decoder. Send is unavailable: review always uses copy-only mode. Automatic event/session retention and a combined subagent view are deferred. The source version is 0.0.0; binary publication requires a separately reviewed server strategy, and GitHub App installation coverage has not been independently established.

Before changing a runtime claim, obtain approval for credential-consuming agent sessions and any live hook edits, record exact binary versions, capture anonymized payloads, then run the design's resume/UI-closed/multi-session cases. Keep parser coverage and runtime evidence separate.
