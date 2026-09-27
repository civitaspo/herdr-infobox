use crate::{
    Result,
    config::Paths,
    git::{CapturedDiff, display_path},
    herdr::{Herdr, Pane},
    model::{SessionSummary, Worktree, content_hash},
};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::PathBuf, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub hash: String,
    pub path: PathBuf,
    pub patch_hash: String,
    pub content_hash: String,
}
pub fn export(
    paths: &Paths,
    session: &SessionSummary,
    worktree: &Worktree,
    diff: &CapturedDiff,
) -> Result<Snapshot> {
    if diff.root != worktree.root {
        return Err("displayed diff belongs to another worktree".into());
    }
    let patch_hash = content_hash(&diff.patch);
    let metadata = serde_json::to_vec(&(
        session.key.clone(),
        display_path(&worktree.root),
        diff.path.as_ref().map(|p| display_path(p)),
        diff.scope,
        diff.merge_base.clone(),
        diff.head.clone(),
        diff.base.clone(),
        diff.captured_at,
        patch_hash.clone(),
        diff.warnings.clone(),
    ))?;
    let hash = content_hash(&metadata);
    let mut longest = 0;
    let mut run = 0;
    for byte in &diff.patch {
        if *byte == b'`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat((longest + 1).max(3));
    let mut markdown=format!("# Local diff snapshot\n\nSnapshot: {hash}\nPatch SHA-256: {patch_hash}\nSession: {} / {} / {}\nWorktree: {}\nRepository: {}\nScope: {:?}\nHEAD: {}\nBase: {}\nMerge base: {}\nCaptured at Unix time: {}\n\n",session.key.provider.name(),escape(&session.key.native_session_id),escape(&session.key.agent_scope),escape(&display_path(&worktree.root)),escape(worktree.github_url.as_deref().unwrap_or("Remote unavailable")),diff.scope,diff.head.as_deref().unwrap_or("Unborn HEAD"),diff.base.as_deref().unwrap_or("Not applicable"),diff.merge_base.as_deref().unwrap_or("Not applicable"),diff.captured_at).into_bytes();
    for warning in &diff.warnings {
        markdown.extend(format!("Warning: {}\n", escape(warning)).as_bytes());
    }
    markdown.extend(
        format!(
            "\n## {}\n\n{fence}diff\n",
            diff.path
                .as_ref()
                .map_or_else(|| "Selected changes".into(), |p| escape(&display_path(p)))
        )
        .as_bytes(),
    );
    markdown.extend(&diff.patch);
    if !diff.patch.ends_with(b"\n") {
        markdown.push(b'\n');
    }
    markdown.extend(format!("{fence}\n").as_bytes());
    if markdown.len() > 10 * 1024 * 1024 {
        return Err("snapshot exceeds 10 MiB limit".into());
    }
    if std::str::from_utf8(&markdown).is_err() {
        let raw = export_raw(paths, diff)?;
        return Err(format!(
            "patch is not UTF-8. Exact raw patch exported to {}. Annotate requires UTF-8 Markdown",
            raw.display()
        )
        .into());
    }
    let path = paths.snapshots.join(format!("{hash}.md"));
    write_immutable(&path, &markdown)?;
    Ok(Snapshot {
        hash,
        path,
        patch_hash,
        content_hash: content_hash(&markdown),
    })
}
fn escape(text: &str) -> String {
    crate::model::safe_text(text)
        .replace(['\n', '\r'], " ")
        .replace('`', "\\`")
}
pub fn installation(herdr: &Herdr) -> Result<PathBuf> {
    let result = herdr.request(&["plugin", "list", "--plugin", "annotate", "--json"])?;
    let plugins = result
        .get("plugins")
        .and_then(serde_json::Value::as_array)
        .ok_or("Herdr plugin list lacks plugins")?;
    let plugin = plugins
        .iter()
        .find(|p| p.get("plugin_id").and_then(serde_json::Value::as_str) == Some("annotate"))
        .ok_or("Annotate Full is not installed. Markdown export remains available.")?;
    if plugin.get("enabled").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err("Annotate is disabled".into());
    }
    if !plugin
        .get("panes")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|panes| {
            panes
                .iter()
                .any(|p| p.get("id").and_then(serde_json::Value::as_str) == Some("doc"))
        })
    {
        return Err("Annotate Full doc entrypoint is unavailable".into());
    }
    if plugin.get("version").and_then(serde_json::Value::as_str) != Some("0.6.0") {
        return Err(
            "Annotate version is unverified. Expected 0.6.0 with plannotator-tui 0.9.4".into(),
        );
    }
    let binary = PathBuf::from(
        plugin
            .get("plugin_root")
            .and_then(serde_json::Value::as_str)
            .ok_or("Annotate root unavailable")?,
    )
    .join("bin/plannotator-tui.exe");
    if !binary.is_file() {
        return Err("Annotate Full binary is missing".into());
    }
    let (ok, out, _) = crate::git::bounded(
        Command::new(&binary).arg("--version"),
        4096,
        std::time::Duration::from_secs(5),
    )?;
    if !ok || String::from_utf8_lossy(&out).trim() != "plannotator-tui 0.9.4" {
        return Err("plannotator-tui version is unverified. Expected 0.9.4".into());
    }
    Ok(binary)
}
pub fn open_copy_review(herdr: &Herdr, target: &Pane, snapshot: &Snapshot) -> Result<()> {
    if content_hash(&fs::read(&snapshot.path)?) != snapshot.content_hash {
        return Err("snapshot was modified after export".into());
    }
    let binary = installation(herdr)?;
    let current = herdr.pane(&target.pane_id)?;
    if current.terminal_id != target.terminal_id || current.agent_session != target.agent_session {
        return Err("target pane was reused; select a current pane".into());
    }
    let file = snapshot.path.to_str().ok_or("snapshot path is not UTF-8")?;
    let binary = binary
        .to_str()
        .ok_or("annotate executable path is not UTF-8")?;
    herdr.open(
        "review",
        &current,
        &[
            ("INFOBOX_SNAPSHOT", file),
            ("INFOBOX_REVIEWER", binary),
            ("INFOBOX_SNAPSHOT_HASH", &snapshot.content_hash),
        ],
        true,
    )?;
    Ok(())
}
pub fn copy_command(binary: &std::path::Path, snapshot: &std::path::Path) -> Command {
    let mut command = Command::new(binary);
    command.arg(snapshot);
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name == "HERDR_ENV"
            || name.starts_with("HERDR_PLUGIN_")
            || name.starts_with("PLANNOTATOR_TUI_")
        {
            command.env_remove(key);
        }
    }
    command
        .env_remove("HERDR_ENV")
        .env_remove("HERDR_PLUGIN_CONTEXT_JSON")
        .env_remove("PLANNOTATOR_TUI_DELIVER_TO")
        .env_remove("PLANNOTATOR_TUI_DELIVER_AGENT");
    command
}
pub fn view_from_env() -> Result<()> {
    let binary = std::env::var_os("INFOBOX_REVIEWER").ok_or("reviewer path missing")?;
    let snapshot = std::env::var_os("INFOBOX_SNAPSHOT").ok_or("snapshot path missing")?;
    let expected =
        std::env::var("INFOBOX_SNAPSHOT_HASH").map_err(|_| "snapshot integrity hash missing")?;
    if content_hash(&fs::read(&snapshot)?) != expected {
        return Err("snapshot integrity mismatch".into());
    }
    let status = copy_command(
        std::path::Path::new(&binary),
        std::path::Path::new(&snapshot),
    )
    .status()?;
    if !status.success() {
        return Err("annotate reviewer failed".into());
    }
    Ok(())
}

pub fn export_raw(paths: &Paths, diff: &CapturedDiff) -> Result<PathBuf> {
    let path = paths
        .snapshots
        .join(format!("{}.patch", content_hash(&diff.patch)));
    write_immutable(&path, &diff.patch)?;
    Ok(path)
}
fn write_immutable(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_readonly(true);
        file.set_permissions(permissions)?;
        match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if fs::read(path)? != bytes {
                    Err("snapshot integrity mismatch".into())
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(e.into()),
        }
    })();
    let _ = fs::remove_file(temporary);
    result
}
