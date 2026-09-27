use crate::{
    Result,
    config::Paths,
    git::{self, CapturedDiff, Scope},
    model::*,
    store::Store,
};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, ClearType},
};
use std::{
    io::{self, Write},
    time::Duration,
};

struct Terminal;
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show);
    }
}
struct Displayed {
    session: SessionSummary,
    worktree: Worktree,
    diff: CapturedDiff,
}
enum JobResult {
    Status((String, u64), Box<(Worktree, git::Status)>),
    Diff((String, u64), Box<Displayed>),
    Error((String, u64), String),
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Selection {
    pinned: bool,
    session: Option<String>,
}
pub fn run(paths: &Paths, store: &mut Store, selector: Option<&str>, once: bool) -> Result<()> {
    crate::herdr::register_ui(paths)?;
    let herdr = crate::herdr::Herdr::from_env().ok();
    if let Some(herdr) = &herdr
        && let Ok(snapshot) = herdr.snapshot().map(tab_snapshot)
        && let Some((pane, key)) = crate::herdr::follow(
            &snapshot,
            std::env::var("INFOBOX_TARGET_PANE").ok().as_deref(),
            &paths.host_id,
        )
    {
        bind_pane(store, &pane, &key)?;
    }
    if let Some(selector) = selector {
        store.resolve(selector)?;
    }
    let sessions = store.sessions()?;
    if sessions.is_empty() {
        println!("No sessions. Register one with session add --provider PROVIDER --native-id ID.");
        return Ok(());
    }
    let instance = herdr
        .as_ref()
        .and_then(|h| h.instance().ok())
        .unwrap_or_else(|| "manual".into());
    let selection_path = paths.state.join(format!(
        "ui-{}.json",
        content_hash(
            format!(
                "{instance}:{}",
                std::env::var("HERDR_TAB_ID").unwrap_or_else(|_| "manual".into())
            )
            .as_bytes()
        )
    ));
    let mut selection: Selection = std::fs::read(&selection_path)
        .ok()
        .and_then(|s| serde_json::from_slice(&s).ok())
        .unwrap_or_default();
    let mut session = if let Some(selector) = selector {
        store.resolve(selector)?
    } else {
        selection
            .session
            .as_deref()
            .and_then(|s| store.resolve(s).ok())
            .unwrap_or_else(|| sessions[0].clone())
    };
    if once {
        print!("{}", render(&store.view(&session)?));
        return Ok(());
    }
    terminal::enable_raw_mode()?;
    let _terminal = Terminal;
    execute!(io::stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;
    let (tx, rx) = std::sync::mpsc::channel();
    let (follow_tx, follow_rx) = std::sync::mpsc::sync_channel(1);
    let worker_paths = paths.clone();
    std::thread::spawn(move || {
        let mut pane_id = None;
        loop {
            if let Ok(mut db) = Store::open(&worker_paths) {
                let _ = crate::ingest::drain(&worker_paths, &mut db);
                if let Ok(pending) = db.pending_paths() {
                    for p in pending {
                        let result = git::discover(&p.path, &p.cwd).map_err(|e| e.to_string());
                        let _ = db.finish_path(&p, &result);
                    }
                }
                if let Ok(sources) = db.transcript_sources() {
                    for (session, cursor) in sources {
                        if session.key.provider == Provider::Codex {
                            match crate::providers::read_codex_transcript(
                                &cursor.source_path,
                                &session,
                                Some(cursor.clone()),
                            ) {
                                Ok(chunk) => {
                                    let _ = db.commit_import(&chunk);
                                }
                                Err(_) => {
                                    let _=db.diagnostic("transcript","Transcript unavailable or unsupported; run reconcile explicitly for details");
                                }
                            }
                        }
                    }
                }
                if let Ok(herdr) = crate::herdr::Herdr::from_env()
                    && let Ok(snapshot) = herdr.snapshot().map(tab_snapshot)
                    && let Some((pane, key)) =
                        crate::herdr::follow(&snapshot, pane_id.as_deref(), &worker_paths.host_id)
                {
                    pane_id = Some(pane.pane_id.clone());
                    if let Ok(id) = bind_pane(&mut db, &pane, &key) {
                        let _ = follow_tx.try_send(id);
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    });
    let mut generation = 0u64;
    let mut selected = 0usize;
    let mut section = 0usize;
    let mut detail: Option<String> = None;
    let mut displayed: Option<Displayed> = None;
    let mut busy = false;
    let mut message = String::new();
    let mut scope = Scope::Unstaged;
    let mut offset = 0usize;
    let mut picker = false;
    let mut changes: Option<(Worktree, git::Status)> = None;
    let mut filter = String::new();
    let mut filtering = false;
    let mut collapsed = [false; 3];
    let mut status_refresh = std::time::Instant::now();
    loop {
        while let Ok(id) = follow_rx.try_recv() {
            if !selection.pinned && session.id != id {
                session = store.resolve(&id)?;
                generation += 1;
                detail = None;
                changes = None;
                displayed = None;
                selected = 0;
                offset = 0;
            }
        }
        if let Ok(result) = rx.try_recv() {
            busy = false;
            match result {
                JobResult::Diff(id, captured) if id == (session.id.clone(), generation) => {
                    detail = Some(format!(
                        "{:?}\n{}\n{}",
                        captured.diff.scope,
                        safe_text(&String::from_utf8_lossy(&captured.diff.patch)),
                        captured.diff.warnings.join("\n")
                    ));
                    displayed = Some(*captured);
                    offset = 0;
                }
                JobResult::Status(id, data) if id == (session.id.clone(), generation) => {
                    let first = changes.is_none();
                    changes = Some(*data);
                    if first {
                        selected = 0;
                    }
                }
                JobResult::Error(id, error) if id == (session.id.clone(), generation) => {
                    message = error
                }
                _ => {}
            }
        }
        if !busy && detail.is_none() && status_refresh.elapsed() >= Duration::from_secs(2) {
            if let Some((worktree, _)) = &changes {
                let worktree = worktree.clone();
                let id = (session.id.clone(), generation);
                let tx = tx.clone();
                busy = true;
                std::thread::spawn(move || {
                    let result = git::status(&worktree.root)
                        .map(|status| JobResult::Status(id.clone(), Box::new((worktree, status))))
                        .unwrap_or_else(|e| JobResult::Error(id, e.to_string()));
                    let _ = tx.send(result);
                });
            }
            status_refresh = std::time::Instant::now();
        }
        let mut view = store.view(&session)?;
        if !filter.is_empty() {
            let query = filter.to_lowercase();
            match section {
                0 => view
                    .worktrees
                    .retain(|w| git::display_path(&w.root).to_lowercase().contains(&query)),
                1 => view.references.retain(|r| {
                    format!("{} {}", r.url, r.title.as_deref().unwrap_or(""))
                        .to_lowercase()
                        .contains(&query)
                }),
                _ => view.plans.retain(|p| {
                    format!("{} {}", p.plan_key, p.markdown)
                        .to_lowercase()
                        .contains(&query)
                }),
            }
        }

        let (width, height) = terminal::size()?;
        let text = if picker {
            format!(
                "Select a session with j/k and Enter\n{}",
                store
                    .sessions()?
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!(
                        "{} {} / {} [{}]",
                        if i == selected { ">" } else { " " },
                        s.key.provider.name(),
                        s.key.native_session_id,
                        &s.id[..8]
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else if let Some(text) = &detail {
            text.clone()
        } else if let Some((worktree, status)) = &changes {
            format!(
                "Changes {} / {:?}\n1 unstaged  2 staged  3 untracked\n{}",
                git::display_path(&worktree.root),
                scope,
                status
                    .changes
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!(
                        "{} {}{} {} {}",
                        if i == selected { ">" } else { " " },
                        c.index,
                        c.worktree,
                        c.kind,
                        git::display_path(&c.path)
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        } else {
            let mut v = view.clone();
            if collapsed[0] {
                v.worktrees.clear();
            }
            if collapsed[1] {
                v.references.clear();
            }
            if collapsed[2] {
                v.plans.clear();
            }
            let mut rendered = render_selected(&v, section, selected);
            if collapsed[0] {
                rendered = rendered.replace(
                    "Repositories 0",
                    &format!("Repositories {} [collapsed]", view.worktrees.len()),
                );
            }
            if collapsed[1] {
                rendered = rendered
                    .replace(
                        "References 0",
                        &format!("References {} [collapsed]", view.references.len()),
                    )
                    .replace("  No collected references. Manual fallback: ref add.\n", "");
            }
            if collapsed[2] {
                rendered = rendered
                    .replace(
                        "Plan revisions 0",
                        &format!("Plan revisions {} [collapsed]", view.plans.len()),
                    )
                    .replace("  Plan unavailable. Manual fallback: plan attach.\n", "");
            }
            rendered
        };
        let mut out = io::stdout();
        execute!(out, cursor::MoveTo(0, 0), terminal::Clear(ClearType::All))?;
        for line in text
            .lines()
            .skip(offset)
            .take(height.saturating_sub(2) as usize)
        {
            let clipped = clip(line, width.saturating_sub(1) as usize);
            write!(out, "{}\r\n", safe_text(&clipped))?;
        }
        execute!(out, cursor::MoveTo(0, height.saturating_sub(2)))?;
        write!(
            out,
            "{}\r\n",
            safe_text(&format!(
                "{} {} {}",
                if selection.pinned { "Pinned" } else { "Follow" },
                if filtering {
                    format!("Filter /{filter}")
                } else {
                    String::new()
                },
                message
            ))
            .chars()
            .take(width as usize)
            .collect::<String>()
        )?;
        write!(out,"{}",if busy{"Reading Git. UI remains available."}else{"q quit | s sessions | Tab section | j/k select | d changes | a annotate | p pin | / filter | Esc back"}.chars().take(width as usize).collect::<String>())?;
        out.flush()?;
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if filtering {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => filtering = false,
                KeyCode::Backspace => {
                    filter.pop();
                }
                KeyCode::Char(c) => filter.push(c),
                _ => {}
            }
            continue;
        }
        if key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            break;
        }
        if matches!(
            key.code,
            KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('s')
        ) {
            generation += 1;
        }
        match key.code {
            KeyCode::Esc => {
                if detail.is_some() {
                    detail = None;
                    displayed = None;
                } else {
                    changes = None;
                    picker = false;
                }
                offset = 0;
                selected = 0;
                filter.clear();
            }
            KeyCode::Char('/') => {
                filtering = true;
                filter.clear();
            }
            KeyCode::Char('c') => {
                collapsed[section] = !collapsed[section];
            }
            KeyCode::Char('p') => {
                selection.pinned = !selection.pinned;
                selection.session = Some(session.id.clone());
                std::fs::write(&selection_path, serde_json::to_vec(&selection)?)?;
            }
            KeyCode::Char('s') => {
                picker = true;
                detail = None;
                changes = None;
                displayed = None;
                selected = 0;
                offset = 0;
            }
            KeyCode::Tab => {
                section = (section + 1) % 3;
                selected = 0;
                detail = None;
                changes = None;
                displayed = None;
                offset = 0;
            }
            KeyCode::BackTab => {
                section = (section + 2) % 3;
                selected = 0;
                detail = None;
                changes = None;
                displayed = None;
                offset = 0;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if detail.is_some() {
                    offset = offset.saturating_add(1);
                } else {
                    selected = selected.saturating_add(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if detail.is_some() {
                    offset = offset.saturating_sub(1);
                } else {
                    selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Enter if picker => {
                if let Some(s) = store.sessions()?.get(selected) {
                    session = s.clone();
                    generation += 1;
                    selection.session = Some(s.id.clone());
                    selection.pinned = true;
                    std::fs::write(&selection_path, serde_json::to_vec(&selection)?)?;
                    picker = false;
                    selected = 0;
                }
            }
            KeyCode::Enter if changes.is_some() && !busy => {
                let (worktree, status) = changes.as_ref().unwrap();
                if let Some(change) = status.changes.get(selected) {
                    let worktree = worktree.clone();
                    let session = session.clone();
                    let path = change.path.clone();
                    let id = (session.id.clone(), generation);
                    let tx = tx.clone();
                    busy = true;
                    std::thread::spawn(move || {
                        let result = git::capture(&worktree.root, scope, Some(&path), None)
                            .map(|diff| {
                                JobResult::Diff(
                                    (session.id.clone(), generation),
                                    Box::new(Displayed {
                                        session,
                                        worktree,
                                        diff,
                                    }),
                                )
                            })
                            .unwrap_or_else(|e| JobResult::Error(id, e.to_string()));
                        let _ = tx.send(result);
                    });
                }
            }
            KeyCode::Enter if section == 1 => {
                if let Some(r) = view.references.get(selected) {
                    detail = Some(format!(
                        "{}\n{}\nRelations: {}\nProvenance: {}\n{}",
                        r.title.as_deref().unwrap_or("Title unavailable"),
                        r.url,
                        r.relations.join(", "),
                        r.sources.join(", "),
                        r.failure.as_deref().unwrap_or("")
                    ));
                    offset = 0;
                }
            }
            KeyCode::Enter if section == 2 => {
                if let Some(p) = view.plans.get(selected) {
                    detail = Some(format!(
                        "Plan {} / {:?}\nExecution {:?} {}\nSource {}\n\n{}",
                        p.plan_key,
                        p.phase,
                        p.execution,
                        if p.selected {
                            "Selected revision"
                        } else {
                            "Historical or draft"
                        },
                        p.source_path
                            .as_ref()
                            .map(|p| git::display_path(p))
                            .unwrap_or_else(|| "Provider item".into()),
                        p.markdown
                    ));
                    offset = 0;
                }
            }
            KeyCode::Char('o') if section == 2 => {
                if let Some(path) = view
                    .plans
                    .get(selected)
                    .and_then(|p| p.source_path.as_ref())
                {
                    match std::fs::canonicalize(path).and_then(|path| {
                        #[cfg(target_os = "macos")]
                        let program = "open";
                        #[cfg(not(target_os = "macos"))]
                        let program = "xdg-open";
                        std::process::Command::new(program)
                            .arg(path)
                            .spawn()
                            .map(|_| ())
                    }) {
                        Ok(()) => {}
                        Err(e) => message = e.to_string(),
                    }
                }
            }
            KeyCode::Char('y') if section == 2 => {
                if let Some(plan) = view.plans.get(selected) {
                    copy(&plan.markdown)?;
                    message = "Copied plan using OSC 52".into();
                }
            }
            KeyCode::Char('o') if section < 2 => {
                let url = if section == 0 {
                    view.worktrees
                        .get(selected)
                        .and_then(|w| w.github_url.as_deref())
                } else {
                    view.references.get(selected).map(|r| r.url.as_str())
                };
                if let Some(url) = url
                    && let Err(e) = open_url(url)
                {
                    message = e.to_string();
                }
            }
            KeyCode::Char('y') if section < 2 => {
                let value = if section == 0 {
                    view.worktrees
                        .get(selected)
                        .map(|w| git::display_path(&w.root))
                } else {
                    view.references.get(selected).map(|r| r.url.clone())
                };
                if let Some(value) = value {
                    copy(&value)?;
                    message = "Copied using OSC 52".into();
                }
            }
            KeyCode::Char('1') => scope = Scope::Unstaged,
            KeyCode::Char('2') => scope = Scope::Staged,
            KeyCode::Char('3') => scope = Scope::Untracked,
            KeyCode::Char('d') | KeyCode::Char('r') | KeyCode::Enter if section == 0 && !busy => {
                if let Some(worktree) = changes
                    .as_ref()
                    .map(|(w, _)| w)
                    .or_else(|| view.worktrees.get(selected))
                {
                    let worktree = worktree.clone();
                    let id = (session.id.clone(), generation);
                    let tx = tx.clone();
                    busy = true;
                    message = "Current worktree changes, including edits by other actors".into();
                    std::thread::spawn(move || {
                        let result = git::status(&worktree.root)
                            .map(|status| {
                                JobResult::Status(id.clone(), Box::new((worktree, status)))
                            })
                            .unwrap_or_else(|e| JobResult::Error(id, e.to_string()));
                        let _ = tx.send(result);
                    });
                }
            }
            KeyCode::Char('a') => {
                if let Some(displayed) = &displayed {
                    match crate::annotate::export(
                        paths,
                        &displayed.session,
                        &displayed.worktree,
                        &displayed.diff,
                    ) {
                        Ok(snapshot) => {
                            message = format!("Exported {}", snapshot.path.display());
                            if let Some(herdr) = &herdr {
                                let opened = herdr
                                    .snapshot()
                                    .and_then(|s| {
                                        s.panes
                                            .into_iter()
                                            .find(|p| {
                                                p.session_key(&paths.host_id).as_ref()
                                                    == Some(&displayed.session.key)
                                            })
                                            .ok_or_else(|| {
                                                "No current pane matches snapshot session".into()
                                            })
                                    })
                                    .and_then(|pane| {
                                        crate::annotate::open_copy_review(herdr, &pane, &snapshot)
                                    });
                                if let Err(e) = opened {
                                    message = format!("{message}. {e}");
                                }
                            }
                        }
                        Err(e) => message = e.to_string(),
                    }
                }
            }
            _ => {}
        }
        if !picker && detail.is_none() {
            let len = if let Some((_, status)) = &changes {
                status.changes.len()
            } else {
                match section {
                    0 => view.worktrees.len(),
                    1 => view.references.len(),
                    _ => view.plans.len(),
                }
            };
            selected = selected.min(len.saturating_sub(1));
        }
    }
    Ok(())
}
pub fn render(view: &SessionView) -> String {
    render_selected(view, usize::MAX, usize::MAX)
}
fn render_selected(view: &SessionView, section: usize, selected: usize) -> String {
    let mut text = format!(
        "Info  {} / {}  {}\n\nRepositories {}\n",
        view.session.key.provider.name(),
        safe_text(&view.session.key.native_session_id),
        if view.session.ended {
            "Ended"
        } else {
            "Registered"
        },
        view.worktrees.len()
    );
    for (i, w) in view.worktrees.iter().enumerate() {
        text.push_str(&format!(
            "{} {}  {} [{}]\n  {}\n",
            mark(section, selected, 0, i),
            git::display_path(&w.root),
            w.branch.as_deref().unwrap_or("Detached/unborn"),
            &w.id[..8],
            w.github_url.as_deref().unwrap_or("GitHub link unavailable")
        ));
    }
    text.push_str(&format!("\nReferences {}\n", view.references.len()));
    for (i, r) in view.references.iter().enumerate() {
        text.push_str(&format!(
            "{} {} [{}]\n  {}\n",
            mark(section, selected, 1, i),
            r.title.as_deref().unwrap_or("Title unavailable"),
            r.relations.join(", "),
            r.url
        ));
        if let Some(f) = &r.failure {
            text.push_str(&format!("  Fetch failed: {f}\n"));
        }
    }
    if view.references.is_empty() {
        text.push_str("  No collected references. Manual fallback: ref add.\n");
    }
    text.push_str(&format!("\nPlan revisions {}\n", view.plans.len()));
    for (i, p) in view.plans.iter().enumerate() {
        text.push_str(&format!(
            "{} {} {:?} / execution {:?} [{}] {}\n",
            mark(section, selected, 2, i),
            p.plan_key,
            p.phase,
            p.execution.unwrap_or(ExecutionState::Unknown),
            &p.hash[..8],
            if p.selected { "Selected" } else { "" }
        ));
    }
    if view.plans.is_empty() {
        text.push_str("  Plan unavailable. Manual fallback: plan attach.\n");
    }
    text.push_str(&format!(
        "\nTasks {} (separate from plan completion)\n",
        view.tasks.len()
    ));
    for task in &view.tasks {
        text.push_str(&format!("  [{}] {}\n", task.status, task.text));
    }
    for cap in &view.capabilities {
        text.push_str(&format!(
            "\n{}: {}. {}\n",
            cap.feature, cap.state, cap.reason
        ));
    }
    safe_text(&text)
}
fn mark(section: usize, selected: usize, target: usize, index: usize) -> &'static str {
    if section == target && selected == index {
        ">"
    } else {
        " "
    }
}
pub fn open_url(url: &str) -> Result<()> {
    let url = crate::store::normalize_url(url)?;
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(target_os = "macos"))]
    let program = "xdg-open";
    std::process::Command::new(program).arg(url).spawn()?;
    Ok(())
}
pub fn copy(value: &str) -> Result<()> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();
    for chunk in value.as_bytes().chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | (chunk.get(2).copied().unwrap_or(0) as u32);
        for shift in [18, 12, 6, 0] {
            encoded.push(alphabet[((n >> shift) & 63) as usize] as char);
        }
        if chunk.len() < 3 {
            encoded.pop();
            encoded.push('=');
        }
        if chunk.len() < 2 {
            let last = encoded.len() - 2;
            encoded.replace_range(last..last + 1, "=");
        }
    }
    print!("\x1b]52;c;{encoded}\x07");
    io::stdout().flush()?;
    Ok(())
}

fn clip(text: &str, width: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut used = 0;
    safe_text(text)
        .replace('\t', "    ")
        .chars()
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used <= width
        })
        .collect()
}
fn tab_snapshot(mut snapshot: crate::herdr::Snapshot) -> crate::herdr::Snapshot {
    if let Ok(tab) = std::env::var("HERDR_TAB_ID") {
        snapshot.panes.retain(|p| p.tab_id == tab);
    }
    snapshot
}
fn bind_pane(store: &mut Store, pane: &crate::herdr::Pane, key: &SessionKey) -> Result<String> {
    let id = store.register(key)?;
    if let Some(cwd) = pane.foreground_cwd.as_ref().or(pane.cwd.as_ref()) {
        let batch = EventBatch {
            schema_version: 1,
            session: key.clone(),
            source: "herdr".into(),
            source_event_id: content_hash(
                format!(
                    "{}:{}:{}",
                    pane.pane_id,
                    pane.terminal_id,
                    git::display_path(cwd)
                )
                .as_bytes(),
            ),
            native_call_key: None,
            source_order: None,
            events: vec![Observation::Path {
                path: cwd.clone(),
                cwd: cwd.clone(),
                relation: "cwd".into(),
            }],
        };
        store.commit_batch(&batch)?;
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_by_terminal_cells() {
        assert_eq!(clip("日本語abc", 5), "日本");
        assert_eq!(clip("ab\tcd", 5), "ab   ");
    }
}
