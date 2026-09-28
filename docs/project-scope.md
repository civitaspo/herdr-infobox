# Project scope

The implementation target is a Rust terminal pane inside Herdr, with provider adapters for Codex, Claude Code, OpenCode, and Cursor CLI, followed by Devin CLI where supported.

## Responsibilities

- Associate a provider's native session with its current Herdr pane without using pane IDs as permanent session IDs.
- Retain multiple repository and worktree associations per session.
- Read GitHub remote links and local Git diffs without staging, committing, or fetching automatically.
- Open immutable Markdown diff snapshots in the reviewer supplied by herdr-annotate.
- Preserve web URLs, titles, and their provenance, distinguishing search results from successful fetches.
- Store plan documents separately from task checklists and preserve the revision selected for execution.
- Use a local embedded Turso store and short-lived collection hooks. OpenCode V2 uses bounded Rust reconciliation against an explicitly selected existing server; V1 is not supported.

Provider capabilities differ. In particular, Codex hosted web search is not a normal local tool hook, and Cursor's interactive CLI and ACP are different collection paths. Support must be established with versioned fixtures before claiming complete collection.

## Implementation boundaries

The Rust package and Herdr manifest implement the local pane and collectors. Provider compatibility remains evidence-specific. Synthetic parser fixtures do not establish live CLI hook behavior. See [compatibility](compatibility.md) and [storage](storage.md).

The existing Securefix client registration, required `status-check`, signing policy, and shared release workflows remain authoritative. Binary packaging is separate from privileged publication. No first release, live hook changes, or credential-consuming agent session is run as a setup test.
