use crate::{Result, config::Paths, model::*, store::Store};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Serialize, Deserialize)]
struct Source {
    session: SessionKey,
    server: String,
    binary: PathBuf,
}

fn path(paths: &Paths, session: &SessionSummary) -> PathBuf {
    paths.state.join(format!("opencode-{}.json", session.id))
}

fn collect(source: &Source, store: &mut Store) -> Result<usize> {
    let bytes = crate::opencode_transport::export(
        &source.binary,
        &source.server,
        &source.session.native_session_id,
    )?;
    let batches = crate::opencode::decode_export(&bytes, &source.session)?;
    let mut count = 0;
    for batch in batches {
        store.commit_batch(&batch)?;
        count += 1;
    }
    for pending in store.pending_paths()? {
        let result = crate::git::discover(&pending.path, &pending.cwd).map_err(|e| e.to_string());
        store.finish_path(&pending, &result)?;
    }
    Ok(count)
}

fn status(store: &mut Store, session: &SessionSummary, state: &str, reason: &str) -> Result<()> {
    if !store
        .view(session)?
        .capabilities
        .iter()
        .any(|c| c.feature == "opencode_sync" && c.state == state && c.reason == reason)
    {
        store.commit_batch(&crate::ingest::manual(
            session.key.clone(),
            vec![Observation::Capability {
                feature: "opencode_sync".into(),
                state: state.into(),
                reason: reason.into(),
            }],
        ))?;
    }
    Ok(())
}

pub fn configured(paths: &Paths, session: &SessionSummary) -> bool {
    path(paths, session).is_file()
}

pub fn connect(
    paths: &Paths,
    store: &mut Store,
    session: &SessionSummary,
    server: String,
    binary: PathBuf,
) -> Result<usize> {
    if session.key.provider != Provider::OpenCode {
        return Err("OpenCode connection requires an OpenCode session".into());
    }
    let binary = if binary.components().count() > 1 || binary.is_absolute() {
        std::path::absolute(binary)?
    } else {
        binary
    };
    let source = Source {
        session: session.key.clone(),
        server,
        binary,
    };
    let count = collect(&source, store)?;
    let temporary = paths
        .state
        .join(format!(".opencode-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        fs::write(&temporary, serde_json::to_vec_pretty(&source)?)?;
        fs::rename(&temporary, path(paths, session))?;
        Ok(())
    })();
    if temporary.exists() {
        let _ = fs::remove_file(temporary);
    }
    result?;
    status(
        store,
        session,
        "available",
        "Last OpenCode V2 export synchronized.",
    )?;
    Ok(count)
}

pub fn disconnect(paths: &Paths, session: &SessionSummary) -> Result<()> {
    match fs::remove_file(path(paths, session)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn reconcile(paths: &Paths, store: &mut Store, session: &SessionSummary) -> Result<usize> {
    let source_path = path(paths, session);
    if !source_path.exists() {
        return Ok(0);
    }
    let source: Source = serde_json::from_slice(&fs::read(source_path)?)?;
    if source.session != session.key || session.key.provider != Provider::OpenCode {
        return Err("OpenCode connection belongs to another session".into());
    }
    match collect(&source, store) {
        Ok(count) => {
            status(
                store,
                session,
                "available",
                "Last OpenCode V2 export synchronized.",
            )?;
            Ok(count)
        }
        Err(error) => {
            status(
                store,
                session,
                "unavailable",
                "OpenCode V2 sync unavailable; cached observations remain. Run reconcile for details.",
            )?;
            Err(error)
        }
    }
}
