use crate::{
    Result,
    config::Paths,
    model::{Provider, SessionKey, content_hash},
};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{fs, io::Write, path::PathBuf, process::Command, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AgentSession {
    pub source: String,
    pub agent: String,
    pub kind: String,
    pub value: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Pane {
    pub pane_id: String,
    pub terminal_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    #[serde(default)]
    pub focused: bool,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub foreground_cwd: Option<PathBuf>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub agent_session: Option<AgentSession>,
}
impl Pane {
    pub fn session_key(&self, host_id: &str) -> Option<SessionKey> {
        let session = self.agent_session.as_ref()?;
        if session.kind != "id" || session.value.trim().is_empty() {
            return None;
        }
        let provider = match session.agent.as_str() {
            "claude" => Provider::Claude,
            "codex" => Provider::Codex,
            "opencode" => Provider::OpenCode,
            "cursor" => Provider::Cursor,
            "devin" => Provider::Devin,
            _ => return None,
        };
        if self.agent.as_deref() != Some(session.agent.as_str()) {
            return None;
        }
        Some(SessionKey {
            host_id: host_id.into(),
            provider,
            native_session_id: session.value.clone(),
            agent_scope: "main".into(),
        })
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct Snapshot {
    pub panes: Vec<Pane>,
    #[serde(default)]
    pub focused_pane_id: Option<String>,
    #[serde(default)]
    pub focused_tab_id: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Herdr {
    pub binary: PathBuf,
    pub socket: PathBuf,
}
impl Herdr {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            binary: std::env::var_os("HERDR_BIN_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| "herdr".into()),
            socket: std::env::var_os("HERDR_SOCKET_PATH")
                .map(PathBuf::from)
                .ok_or("HERDR_SOCKET_PATH is unavailable. Open Info from Herdr.")?,
        })
    }
    pub fn request(&self, args: &[&str]) -> Result<serde_json::Value> {
        let mut cmd = Command::new(&self.binary);
        cmd.args(args).env("HERDR_SOCKET_PATH", &self.socket);
        let (ok, out, err) =
            crate::git::bounded(&mut cmd, 2 * 1024 * 1024, Duration::from_secs(5))?;
        if !ok {
            return Err(format!(
                "Herdr command failed: {}",
                String::from_utf8_lossy(&err).trim()
            )
            .into());
        }
        let value: serde_json::Value = serde_json::from_slice(&out)?;
        if let Some(error) = value.get("error") {
            return Err(format!("Herdr API error: {error}").into());
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "Herdr response lacks result".into())
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let result = self.request(&["api", "snapshot"])?;
        Ok(serde_json::from_value(
            result
                .get("snapshot")
                .cloned()
                .ok_or("Herdr response lacks snapshot")?,
        )?)
    }
    pub fn pane(&self, id: &str) -> Result<Pane> {
        let result = self.request(&["pane", "get", id])?;
        Ok(serde_json::from_value(
            result
                .get("pane")
                .cloned()
                .ok_or("Herdr response lacks pane")?,
        )?)
    }
    pub fn instance(&self) -> Result<String> {
        let meta = fs::metadata(&self.socket)?;
        Ok(format!(
            "{}:{}:{}:{}:{}",
            self.socket.display(),
            meta.dev(),
            meta.ino(),
            meta.ctime(),
            meta.ctime_nsec()
        ))
    }
    pub fn open(
        &self,
        entrypoint: &str,
        target: &Pane,
        env: &[(&str, &str)],
        focus: bool,
    ) -> Result<Pane> {
        let mut args = vec![
            "plugin",
            "pane",
            "open",
            "--plugin",
            "herdr-infobox",
            "--entrypoint",
            entrypoint,
            "--placement",
            "split",
            "--target-pane",
            &target.pane_id,
            "--direction",
            "right",
            if focus { "--focus" } else { "--no-focus" },
        ];
        let values: Vec<_> = env.iter().map(|(k, v)| format!("{k}={v}")).collect();
        for value in &values {
            args.extend(["--env", value]);
        }
        let result = self.request(&args)?;
        Ok(serde_json::from_value(
            result
                .pointer("/plugin_pane/pane")
                .cloned()
                .ok_or("Herdr open response lacks pane")?,
        )?)
    }
}
#[derive(Debug, Default, Deserialize, Serialize)]
struct TabState {
    enabled: bool,
    pane: Option<String>,
    terminal: Option<String>,
    target: Option<String>,
    reserved: bool,
    #[serde(default)]
    creation_token: Option<String>,
}
fn save(path: &std::path::Path, state: &TabState) -> Result<()> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    f.write_all(&serde_json::to_vec(state)?)?;
    f.sync_all()?;
    fs::rename(temporary, path)?;
    Ok(())
}
pub fn ensure(paths: &Paths, herdr: &Herdr, toggle: bool) -> Result<Option<String>> {
    let snapshot = herdr.snapshot()?;
    let tab = snapshot
        .focused_tab_id
        .as_deref()
        .or_else(|| {
            snapshot
                .panes
                .iter()
                .find(|p| Some(&p.pane_id) == snapshot.focused_pane_id.as_ref())
                .map(|p| p.tab_id.as_str())
        })
        .ok_or("no focused Herdr tab")?;
    let key = content_hash(format!("{}:{tab}", herdr.instance()?).as_bytes());
    let dir = paths.state.join("panes");
    fs::create_dir_all(&dir)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(format!("{key}.lock")))?;
    if lock.try_lock().is_err() {
        return Ok(None);
    }
    let snapshot = herdr.snapshot()?;
    if snapshot.focused_tab_id.as_deref() != Some(tab) {
        return Ok(None);
    }
    let file = dir.join(format!("{key}.json"));
    let mut state: TabState = match fs::read(&file) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => TabState::default(),
        Err(e) => return Err(e.into()),
    };
    let existing = state.pane.as_ref().and_then(|id| {
        snapshot.panes.iter().find(|p| {
            &p.pane_id == id && Some(&p.terminal_id) == state.terminal.as_ref() && p.tab_id == tab
        })
    });
    if state.reserved && existing.is_none() {
        return Err("Info pane creation is still unresolved. Wait for its UI to register; refusing a duplicate pane.".into());
    }
    if toggle {
        if let Some(pane) = existing {
            herdr.request(&["plugin", "pane", "close", &pane.pane_id])?;
            state.enabled = false;
            state.pane = None;
            state.terminal = None;
            save(&file, &state)?;
            return Ok(None);
        }
        state.enabled = true;
        state.reserved = false;
        state.pane = None;
        state.terminal = None;
    } else {
        if !state.enabled {
            return Ok(None);
        }
        if let Some(pane) = existing {
            return Ok(Some(pane.pane_id.clone()));
        }
        if state.pane.is_some() || state.reserved {
            state.enabled = false;
            save(&file, &state)?;
            return Ok(None);
        }
    }
    let target = snapshot
        .panes
        .iter()
        .find(|p| {
            p.tab_id == tab
                && Some(&p.pane_id) == snapshot.focused_pane_id.as_ref()
                && p.agent.is_some()
        })
        .or_else(|| {
            snapshot.panes.iter().find(|p| {
                p.tab_id == tab && Some(&p.pane_id) == state.target.as_ref() && p.agent.is_some()
            })
        })
        .ok_or("focus an agent pane before opening Info")?;
    state.target = Some(target.pane_id.clone());
    state.reserved = true;
    let token = uuid::Uuid::new_v4().to_string();
    state.creation_token = Some(token.clone());
    save(&file, &state)?;
    let pane = herdr.open(
        "info",
        target,
        &[
            ("INFOBOX_CREATION_TOKEN", &token),
            ("INFOBOX_TARGET_PANE", &target.pane_id),
        ],
        false,
    )?;
    state.pane = Some(pane.pane_id.clone());
    state.terminal = Some(pane.terminal_id);
    state.reserved = false;
    save(&file, &state)?;
    Ok(Some(pane.pane_id))
}
pub fn follow(
    snapshot: &Snapshot,
    current: Option<&str>,
    host: &str,
) -> Option<(Pane, SessionKey)> {
    let pane = snapshot
        .panes
        .iter()
        .find(|p| {
            Some(&p.pane_id) == snapshot.focused_pane_id.as_ref() && p.session_key(host).is_some()
        })
        .or_else(|| {
            snapshot
                .panes
                .iter()
                .find(|p| Some(p.pane_id.as_str()) == current && p.session_key(host).is_some())
        })?;
    Some((pane.clone(), pane.session_key(host)?))
}

pub fn register_ui(paths: &Paths) -> Result<()> {
    let Ok(token) = std::env::var("INFOBOX_CREATION_TOKEN") else {
        return Ok(());
    };
    let pane = std::env::var("HERDR_PANE_ID").map_err(|_| "Info pane identity is unavailable")?;
    register_pane(paths, &Herdr::from_env()?, &token, &pane)
}
pub fn register_pane(paths: &Paths, herdr: &Herdr, token: &str, pane_id: &str) -> Result<()> {
    let pane = herdr.pane(pane_id)?;
    let key = content_hash(format!("{}:{}", herdr.instance()?, pane.tab_id).as_bytes());
    let dir = paths.state.join("panes");
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(format!("{key}.lock")))?;
    lock.lock()?;
    let file = dir.join(format!("{key}.json"));
    let mut state: TabState = serde_json::from_slice(&fs::read(&file)?)?;
    if state.creation_token.as_deref() != Some(token) || !state.enabled {
        return Err("Info pane reservation no longer matches".into());
    }
    state.pane = Some(pane.pane_id);
    state.terminal = Some(pane.terminal_id);
    state.reserved = false;
    save(&file, &state)
}
