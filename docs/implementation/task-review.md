# Shell and mise task review

Historical validation record. OpenCode V1 and its Node bridge were subsequently removed; see [current compatibility](../compatibility.md).

The review found three gaps and one redundant check path:

- The Rust dispatcher silently succeeded outside the checkout because it treated a missing Cargo.toml as the old infrastructure-only phase. Its callers now use native mise tasks, and the dispatcher is deleted. Task descriptions no longer imply Rust is optional.
- Release-policy fixtures resolved cliff.toml from the caller's directory and inherited Git repository/configuration state. The script now resolves its own root, clears Git's repository-local environment variables, and disables system/global configuration and templates in disposable repositories.
- The existing PTY UI and OpenCode bridge tests were outside the default test/CI path. PTY checks now run as Rust integration tests; Node is scoped to the OpenCode test task. The test task explicitly selects the native host target and output directory, then passes that binary to the probes. This avoids accidentally testing a stale executable when CARGO_BUILD_TARGET or build.target selects a cross-compilation target.
- Lint used a workflow mutator followed by git diff, which also rejected unrelated unstaged workflow edits. Ghalint already rejects missing checkout credential protection. Local and CI lint now use that read-only policy check; mutation remains in the existing Securefix autofix job. Required status checks, policy enforcement, approvals, and action pins remain intact.

The package and install scripts retain their root anchoring, native target/output-directory selection, bounded list of archive targets, private temporary staging cleanup, and archive-install executable check. No generic shell helper is needed. The benchmark remains opt-in because timing is host-dependent; deterministic assertions run in the test task.

Verification included a release-policy run from outside the checkout with a disposable foreign GIT_DIR, GIT_WORK_TREE, GIT_INDEX_FILE, and global hook configuration. The foreign repository remained unchanged and its hook did not execute. A negative workflow fixture without persist-credentials failed ghalint's checkout_persist_credentials_should_be_false policy. Mise task validation and execution from the tests subdirectory passed. Required lint/test/build checks include the 52 Rust tests, PTY checks, and both bridge checks.

The follow-up ponytail-review found no unnecessary complexity. A correctness follow-up identified target-path ambiguity; the native host selection above resolves it without another shell dispatcher.

The native test task also passed with CARGO_BUILD_TARGET=wasm32-unknown-unknown and a foreign CARGO_TARGET_DIR. A failing RUSTC_WRAPPER caused the task to stop before any runtime probe, so stale executables cannot mask a Cargo failure.

The expanded CI run exposed a separate collector defect: a 10 ms spool-accounting stopwatch rejected a valid event under Linux runner load. [PR #7](https://github.com/civitaspo/herdr-infobox/pull/7) removed that scheduling-dependent rejection, retained byte/entry/lock limits, and passed Linux/macOS CI before merge. The tooling branch includes that fix through its main base.
