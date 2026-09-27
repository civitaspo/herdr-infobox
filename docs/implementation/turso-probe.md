# Turso engine prototype decision

Go for the requested local Turso engine migration, with `turso = { version = "=0.7.2", default-features = false }` and explicit `.experimental_multiprocess_wal(true)`. This is a tested local prototype decision, not a general production-reliability claim. Linux and crash/power-loss testing remain outstanding.

## Exact source

The published crate is the Rust rewrite, formerly Limbo, rather than the libSQL SQLite fork or Turso cloud service. The official [rename announcement](https://turso.tech/blog/upcoming-changes-to-the-turso-platform-and-roadmap) establishes the naming. [Rust API](https://docs.rs/turso/0.7.2/turso/) exposes local Builder, Connection and Transaction. Crate `.cargo_vcs_info.json` identifies source commit `046e9cbf67d22491e8ecc941ec2891b02a9f3cad`, path `bindings/rust`.

[Builder source](https://github.com/tursodatabase/turso/blob/046e9cbf67d22491e8ecc941ec2891b02a9f3cad/bindings/rust/src/lib.rs) defaults multiprocess WAL to false. `.experimental_multiprocess_wal(true)` forwards the exact `multiprocess_wal` engine feature. It remains named experimental. The core supports shared WAL on 64-bit Unix and Windows, rejects unsuitable IO backends and network filesystems, and does not support this mode for `:memory:`. Target macOS/Linux local files are the intended host shape.

## Observed probe results

Throwaway project `/tmp/infobox-turso-probe`. Published dependencies are cached in `/tmp/infobox-turso-cargo` because the default user Cargo cache was not writable in the sandbox. No implementation repository files changed.

The executable uses `futures::executor::block_on`, no Tokio runtime. Parent holds its file database connection open while two independent executable child processes each insert 100 rows with UPSERT. Both exited 0. A third child holds `BEGIN IMMEDIATE` for contention measurement.

```text
foreign_key_rejected=true
rollback_count=Integer(0)
worker_a=ExitStatus(0) worker_b=ExitStatus(0)
SELECT count(*) FROM events => Integer(200)
PRAGMA integrity_check => Text("ok")
PRAGMA synchronous => Integer(2)
PRAGMA foreign_keys => Integer(1)
PRAGMA wal_checkpoint(TRUNCATE) => Integer(0)
busy_result=Err(Busy("database is locked")) elapsed_ms=25
vacuum_backup=Ok(0)
backup_count=Integer(200)
reopened_count=Integer(200)
```

Foreign-key enforcement rejected an orphan. A dropped Rust Transaction rolled back its insertion before the next query. Explicit FULL synchronous read back as 2. UPSERT, file reopen, concurrent processes, integrity check, 25ms busy timeout, and VACUUM INTO consistent backup all passed on this host. `VACUUM INTO` worked without `.experimental_vacuum(true)`; that flag is for in-place VACUUM.

Re-run with a fresh filename, since the probe deliberately does not overwrite an existing backup:

```sh
PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME=/tmp/infobox-turso-cargo cargo run --manifest-path /tmp/infobox-turso-probe/Cargo.toml -- /tmp/infobox-turso-probe/fresh.db
```

## Store integration facts

Use `Connection::execute_batch`, async `execute`/`query`, `Rows::next`, and typed `Row::get<T>`. Existing synchronous Store methods can `block_on` internal operations without moving an async runtime into UI/hooks. `busy_timeout(Duration)` is a synchronous method returning Result. Spool on `turso::Error::Busy` and `BusySnapshot`, not string matching. An explicit transaction boundary still owns one event batch and transcript cursor.

[Compatibility matrix](https://github.com/tursodatabase/turso/blob/046e9cbf67d22491e8ecc941ec2891b02a9f3cad/COMPAT.md) says FULL and OFF synchronous modes are supported; foreign_keys is supported; foreign_key_check and defer_foreign_keys are not. The SQLite backup API is not implemented. Use `VACUUM INTO` before migrations instead of rusqlite backup APIs. `VACUUM INTO` may be bound through SQL; the probe safely quoted its generated local filename.

The same matrix explicitly excludes mixed SQLite/Turso multiprocess access. Do not leave rusqlite test/doctor connections opening the live file while Turso processes run. SQLite file-format compatibility does not authorize a mixed-engine concurrency test. Change every in-process database path to Turso, including test fixture writers, and state this operational boundary in docs.

Default crate features enable FTS and mimalloc, both unnecessary here. Disable them. The crate still has a substantial transitive dependency graph; this is the cost of the user-selected engine, not justification for adding more abstraction. Published code uses MIT licensing.
