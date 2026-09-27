# Local storage

Infobox uses the Rust Turso engine, pinned to 0.7.2 with default features disabled. This is the embedded engine formerly named Limbo. It does not require Turso Cloud, a database server, an account, or credentials.

Each collector process and UI worker opens the same local database with `experimental_multiprocess_wal(true)`. The option remains experimental in this version. The store enables foreign keys, FULL synchronization, and a 25 ms busy timeout. A busy enrolled-session collector writes its normalized event to the bounded private spool. Replay deduplicates events transactionally.

The synchronous Store API drives native Turso futures internally. UI rendering does not need an async runtime. Event application and transcript cursor advancement share a transaction. Failed event batches roll back. Session identity, repository identity, worktree identity, Plan revisions, checklist entries, and selected execution revisions remain separate records.

The [prototype evidence](implementation/turso-probe.md) records independent-process writes, rollback, lock contention, integrity, file reopen, and backup checks on macOS. Those checks do not establish crash or power-loss durability on every supported filesystem. Keep the database on a local filesystem. Linux runtime verification remains a separate acceptance item.

## Backup and recovery boundaries

Turso's SQLite file-format compatibility does not permit mixed SQLite and Turso processes to access the live database concurrently. Do not point sqlite3, rusqlite, or another SQLite-based viewer at the active file. The Turso 0.7.2 compatibility matrix does not implement SQLite's backup API.

Close all collectors and UI panes before taking a filesystem backup of the whole state directory. An engine-aware backup can use Turso's `VACUUM INTO` with a new destination. The prototype verified reopening that backup. Do not copy only the main database file while a WAL writer is active.

The initial application schema is version 1. There is no released earlier infobox schema to migrate. A binary rejects newer schema versions. Future migrations must create and verify a consistent backup before changing the schema.

The spool has a 32 MiB byte limit and a 1,024-entry accounting limit. Reserving space waits at most 100 ms for its filesystem lock; writing and syncing happen after releasing that lock. The collector records best-effort loss counts when capacity, lock wait, or accounting limits are exhausted. These individual bounds do not establish a 100 ms end-to-end p95. It cannot guarantee recovery from a full disk or forced process termination. Unknown transcript schemas report a diagnostic and retain the last committed cursor.

Normalized event and session expiry are not automatic in this release. This avoids deleting provenance before retention behavior has its own acceptance tests. Snapshot files are immutable and never automatically collected. Use `snapshot list` and explicit `snapshot purge --hash HASH` for snapshots you no longer need. Purging a snapshot does not delete annotate comments or provider transcripts.
