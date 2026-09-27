use crate::{Result, config::Paths, model::*, store::Store};
use std::{
    fs,
    io::{Read, Write},
    time::Duration,
};
const MAX_INPUT: u64 = 8 * 1024 * 1024;
const MAX_SPOOL: u64 = 32 * 1024 * 1024;
pub fn read_stdin() -> Result<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = (|| {
            let mut input = std::io::stdin();
            input.by_ref().take(MAX_INPUT + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_INPUT {
                std::io::copy(&mut input, &mut std::io::sink())?;
                return Err(std::io::Error::other("Input truncated"));
            }
            Ok(bytes)
        })();
        let _ = tx.send(result);
    });
    Ok(rx
        .recv_timeout(Duration::from_millis(150))
        .map_err(|_| "Hook stdin deadline exceeded")??)
}
pub fn persist_or_spool(paths: &Paths, batch: &EventBatch) -> Result<()> {
    let mut store = Store::open_existing(paths)?;
    if store.id_for(&batch.session)?.is_none() {
        return Err("Session is not registered".into());
    }
    match store.commit_batch(batch) {
        Ok(_) => Ok(()),
        Err(error) => {
            let busy = error.downcast_ref::<turso::Error>().is_some_and(|e| {
                matches!(e, turso::Error::Busy(_) | turso::Error::BusySnapshot(_))
            });
            if !busy {
                return Err(error);
            }
            spool(paths, batch)
        }
    }
}
fn spool(paths: &Paths, batch: &EventBatch) -> Result<()> {
    let bytes = serde_json::to_vec(batch)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(paths.state.join("spool.lock"))?;
    let deadline = std::time::Instant::now() + Duration::from_millis(100);
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(_) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1))
            }
            Err(_) => {
                record_loss(paths);
                return Err("Spool busy".into());
            }
        }
    }
    let mut used = 0;
    let mut count = 0;
    for entry in fs::read_dir(&paths.spool)? {
        count += 1;
        if count >= 1024 {
            record_loss(paths);
            return Err("Spool accounting limit exceeded".into());
        }
        match entry?.metadata() {
            Ok(metadata) => used += metadata.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    if used + bytes.len() as u64 > MAX_SPOOL {
        record_loss(paths);
        return Err("Spool capacity exceeded".into());
    }
    let id = uuid::Uuid::new_v4();
    let temp = paths.spool.join(format!(".{id}.tmp"));
    let dest = paths.spool.join(format!("{id}.json"));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    file.set_len(bytes.len() as u64)?;
    drop(lock);
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(temp, dest)?;
    Ok(())
}
pub fn drain(paths: &Paths, store: &mut Store) -> Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(&paths.spool)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        let batch: EventBatch = match serde_json::from_slice(&bytes) {
            Ok(batch) => batch,
            Err(_) => {
                let _ = store.diagnostic(
                    "spool_format",
                    "A normalized spool record is malformed; inspect quarantined spool files",
                );
                let _ = fs::rename(&path, path.with_extension("invalid"));
                continue;
            }
        };
        if batch.schema_version != 1 {
            let _ = store.diagnostic(
                "spool_schema",
                "A normalized spool record uses an unsupported schema",
            );
            continue;
        }
        store.commit_batch(&batch)?;
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        count += 1;
    }
    Ok(count)
}
pub fn manual(session: SessionKey, events: Vec<Observation>) -> EventBatch {
    EventBatch {
        schema_version: 1,
        session,
        source: "manual".into(),
        source_event_id: uuid::Uuid::new_v4().to_string(),
        native_call_key: None,
        source_order: None,
        events,
    }
}

fn record_loss(paths: &Paths) {
    let path = paths.state.join("collection-loss-count");
    let count = fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let _ = fs::write(path, count.saturating_add(1).to_string());
}
