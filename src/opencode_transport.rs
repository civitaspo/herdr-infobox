use crate::{Result, git};
use std::{path::Path, process::Command, time::Duration};
use url::Url;

pub const VERIFIED_VERSION: &str = "2.0.18";

pub fn export(binary: &Path, server: &str, native_id: &str) -> Result<Vec<u8>> {
    let server_url = Url::parse(server).map_err(|_| "Invalid OpenCode server URL")?;
    if !matches!(server_url.scheme(), "http" | "https")
        || server_url.host_str().is_none()
        || !server_url.username().is_empty()
        || server_url.password().is_some()
        || server_url.query().is_some()
        || server_url.fragment().is_some()
    {
        return Err(
            "OpenCode server must be an HTTP(S) URL without credentials, query, or fragment".into(),
        );
    }
    if native_id.is_empty() {
        return Err("OpenCode session ID must not be empty".into());
    }
    let version = run(Command::new(binary).arg("--version"), 4096)?;
    if std::str::from_utf8(&version)
        .map(str::trim)
        .ok()
        .and_then(|version| version.strip_prefix("opencode v"))
        != Some(VERIFIED_VERSION)
    {
        return Err("OpenCode CLI version is unverified; this integration requires 2.0.18".into());
    }
    let info = run(
        Command::new(binary).args(["api", "--server", server_url.as_str(), "GET", "/api/info"]),
        64 * 1024,
    )?;
    let info: serde_json::Value =
        serde_json::from_slice(&info).map_err(|_| "Invalid OpenCode server info")?;
    if info.get("version").and_then(serde_json::Value::as_str) != Some(VERIFIED_VERSION) {
        return Err(
            "OpenCode server version is unverified; this integration requires 2.0.18".into(),
        );
    }
    let mut endpoint = server_url.clone();
    endpoint
        .path_segments_mut()
        .map_err(|_| "Invalid OpenCode server URL")?
        .clear()
        .extend(["api", "experimental", "session", native_id, "export"]);
    run(
        Command::new(binary).args([
            "api",
            "--server",
            server_url.as_str(),
            "GET",
            endpoint.path(),
        ]),
        16 * 1024 * 1024,
    )
}

fn run(command: &mut Command, limit: usize) -> Result<Vec<u8>> {
    let (success, stdout, _) = git::bounded(command, limit, Duration::from_secs(5))
        .map_err(|_| "OpenCode command could not complete within its time and output limits")?;
    if !success {
        return Err("OpenCode command failed".into());
    }
    Ok(stdout)
}
