use crate::{Result, ingest, model::*, store::Store};
use std::path::Path;

pub(crate) fn open(path: &Path) -> Result<std::fs::File> {
    #[cfg(unix)]
    let file = {
        use rustix::fs::{Mode, OFlags};
        std::fs::File::from(rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?)
    };
    #[cfg(not(unix))]
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err("Transcript must be a regular file".into());
    }
    Ok(file)
}

fn read(
    path: &Path,
    session: &SessionSummary,
    previous: Option<SourceCursor>,
) -> Result<ImportChunk> {
    match session.key.provider {
        Provider::Codex => crate::providers::read_codex_transcript(path, session, previous),
        Provider::Cursor => crate::cursor::read(path, session, previous),
        _ => Err("Transcript import supports Codex and Cursor sessions".into()),
    }
}

pub fn import(store: &mut Store, session: &SessionSummary, path: &Path) -> Result<()> {
    let path = std::fs::canonicalize(path)?;
    let previous = store
        .transcript_sources()?
        .into_iter()
        .find(|(s, c)| s.id == session.id && c.source_path == path)
        .map(|(_, c)| c);
    let chunk = read(&path, session, previous)?;
    store.commit_import(&chunk)
}

pub fn reconcile(store: &mut Store, selected: Option<&SessionSummary>) -> Result<()> {
    let sources = store.transcript_sources()?;
    let mut first_error = None;
    for session in store.sessions()? {
        if selected.is_some_and(|s| s.id != session.id) {
            continue;
        }
        let mut found = false;
        let mut failed = false;
        for (owner, cursor) in &sources {
            if owner.id != session.id {
                continue;
            }
            found = true;
            let result = read(&cursor.source_path, owner, Some(cursor.clone()))
                .and_then(|chunk| store.commit_import(&chunk));
            if let Err(error) = result {
                failed = true;
                first_error.get_or_insert(error);
            }
        }
        if found {
            let (state, reason) = if failed {
                (
                    "unavailable",
                    "A registered transcript is unavailable or unsupported. Cached observations are retained; run reconcile for details.",
                )
            } else {
                ("available", "Registered transcript sources reconciled.")
            };
            if !store
                .view(&session)?
                .capabilities
                .iter()
                .any(|c| c.feature == "transcript_sync" && c.state == state && c.reason == reason)
            {
                store.commit_batch(&ingest::manual(
                    session.key.clone(),
                    vec![Observation::Capability {
                        feature: "transcript_sync".into(),
                        state: state.into(),
                        reason: reason.into(),
                    }],
                ))?;
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}
