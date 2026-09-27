# Design verdict

This is a cross-judge review of two independently prepared sketches using the same model. It provides independent reasoning, not cross-model diversity. Scores evaluate written designs, not tested implementations.

| Criterion | Candidate A / 5 | Candidate B / 5 |
|---|---:|---:|
| Identity isolation | 5 | 4 |
| Plan revision and order correctness | 4 | 3 |
| Bounded hooks and recovery | 4 | 3 |
| Minimal single-package interfaces | 4 | 4 |
| Testable displayed snapshot integrity | 5 | 4 |
| Total | 22 | 18 |

## Base choice

Choose A's normalized relational facts with a short retained normalized event log. B independently reaches the same recommendation after considering retention. This convergence has a concrete reason. Deleting 30-day events must not erase a still-visible 90-day session's references, plan revisions, or execution binding. A preserves those facts directly. B would need durable checkpoints and checkpoint migrations, reintroducing much of A with an additional replay boundary.

A separates native session identity from pane bindings, repository from worktree, and plan revision from execution. It specifies byte-preserving paths, bounded Git reads, and export from the exact retained CapturedDiff. Those details make the design easier to test against the user's correctness requirements.

## Grafts

Take B's explicit EventBatch boundary for one native input that produces multiple observations. One hook parse can emit session context, repository candidates, references, and plan updates. Commit the batch atomically, retaining per-observation source identity and native call keys. A batch ID must not replace observation-level deduplication across hook and transcript sources.

Take B's pure reducer test style, but apply it to A's domain merge functions and transactional Store behavior. Do not add a second in-memory projection implementation. Test event permutations, duplicate imports, task merge/replace, and approval of an exact revision against literal expected views.

## Required corrections

1. A's PlanRevision contains no provider sequence or revision identity in its payload. SourcePosition supplies event order but is only comparable within a stream. Store approval evidence per revision and define promotion rules explicitly. An incomparable late approval may update history but must not silently replace a selected execution. Content hash identifies body bytes, not approval timing or execution identity.

2. Both sketches need compare-and-swap transcript cursor advancement. A explicitly carries cursor_before; B's optional cursor does not specify that check. Concurrent imports must reject stale starting cursors, commit observations and advancement together, and preserve an incomplete trailing record. Unknown schema is not the same as malformed known-schema input and must not be skipped as successfully reconciled history.

3. A's hook deadline is asserted without a concrete stdin strategy. Ordinary read_to_end can block before the byte bound matters. Test a producer that leaves stdin open and a locked DB. Avoid migrations or full spool scans on the hook hot path. A bounded spool cannot recover disk-full writes; loss reporting itself may fail and must remain best effort.

4. A's per-UI worker pool plus reconciliation leases is plausible but broader than the first vertical slice needs. Start with one bounded worker per UI and uniqueness-protected imports. Add coordination only when tests show duplicate work matters. Do not share a connection across worker I/O.

5. Both need a snapshot invariant test comparing displayed patch bytes to exported fenced bytes and persisted patch hash, including backtick fences, non-UTF-8 paths, truncation warnings, and refresh after export. Downstream review must open that stored snapshot without recapture.

## Rejected alternatives

Full-session JSON aggregates offer fewer tables but require growing read-modify-write transactions and hide relational identity constraints. Append-only replay is attractive for short histories, but retention requires checkpoints and complicates startup latency. Neither is the smallest complete solution for the requested durable state and recovery behavior. No daemon, provider trait hierarchy, generic repository layer, or projection cache is justified.

## Engine revision

The user subsequently selected the embedded Turso engine (formerly Limbo). The normalized model and transaction boundaries remain the same. See [the engine probe](turso-probe.md) and [storage boundaries](../storage.md). Automatic event/session retention and a combined subagent view are deferred; scoped identities remain separate and snapshots have explicit lifecycle commands.
