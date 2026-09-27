# Project scope

The implementation target is a Rust terminal pane inside Herdr, with provider adapters for Codex, Claude Code, OpenCode, and Cursor CLI, followed by Devin CLI where supported.

## Planned responsibilities

- Associate a provider's native session with its current Herdr pane without using pane IDs as permanent session IDs.
- Retain multiple repository and worktree associations per session.
- Read GitHub remote links and local Git diffs without staging, committing, or fetching automatically.
- Open immutable Markdown diff snapshots in the reviewer supplied by herdr-annotate.
- Preserve web URLs, titles, and their provenance, distinguishing search results from successful fetches.
- Store plan documents separately from task checklists and preserve the revision selected for execution.
- Use a local SQLite store and short-lived collection hooks. OpenCode needs a small native TypeScript bridge.

Provider capabilities differ. In particular, Codex hosted web search is not a normal local tool hook, and Cursor's interactive CLI and ACP are different collection paths. Support must be established with versioned fixtures before claiming complete collection.

## Not part of the foundation

This setup does not implement the plugin, install agent hooks, create a fake executable, or publish a first release. Binary packaging and a Herdr plugin manifest will be added with the implementation. The current release flow publishes source-only GitHub Releases through Securefix.
