# Implementation workflow

## Definition of done

A single Rust package runs an Info terminal pane. A fixture-driven session with two repositories can switch GitHub links and immutable diff details, persist references and plan revisions across restart, and open the displayed snapshot in a copy-only reviewer. Each provider reports its evidence and unsupported capabilities. Required repository checks pass and reviewable PRs preserve the existing Securefix workflows.

Real provider sessions, live hook installation, and release publication require separate authorization. Synthetic fixtures establish parser behavior, not runtime compatibility.

## Checklist

- [x] Read the Principles section of the poteto-mode skill.
- [x] Phase A: Frame.
- [x] Phase B: Design the workflow.
- [x] Phase C: Run the loop.
- [x] Phase D: Keep the audit trail.
- [x] Phase E: Verify and hand back.

## Verification units

1. Research provider and Herdr contracts. Compare two independent data ownership sketches.
2. Add typed identities, Turso transactions, manual registration, and a session picker. Verify restart and isolation.
3. Add bounded Git discovery, status, immutable diff snapshots, and copy-only annotate. Verify temporary repositories and hostile paths.
4. Connect Claude records end to end. Add OpenCode, Codex, Cursor, and verified Devin fields with versioned fixtures and explicit limitations.
5. Add spool replay, transcript cursors, safe configuration fragments, doctor, and installation documentation.
6. Prepare version synchronization and local packaging separately from plugin implementation. Preserve shared release publication.
7. Run behavioral integration checks and the required mise checks. Perform independent correctness and ponytail reviews. Open signed PRs without merging.

## Throughput checkpoint

- Blocking first steps. Read the full design and repository policy, verify external contracts, and settle shared types before implementation.
- Independent workstreams. Provider research and Herdr research have separate output files. Implementation ownership follows module boundaries after shared types compile.
- Shared mutable state. One owner changes Cargo metadata and the database schema. Workers do not modify another owner's files.
- Smallest safe decomposition. Persistence precedes Git and UI integration. Provider breadth follows one complete Claude path.

## Baseline

The working tree was clean at 492d992. `mise run lint`, `mise run test`, and `mise run build` passed before implementation. Rust checks reported that Cargo.toml was absent. No live provider settings or coding-agent sessions were used.

## Delivery evidence

See [implementation validation](validation.md), [ponytail review](ponytail-review.md), [review resolutions](review-resolution.md), and [packaging validation](packaging-validation.md). GitHub CLI was used because the Origin CLI was unavailable. The implementation and packaging have separate signed branches; neither PR is authorized for merge.
