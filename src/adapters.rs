use crate::{Result, config::Paths, model::Provider};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
struct Receipt {
    target: PathBuf,
    entries: Vec<(String, Value)>,
}

fn directory(paths: &Paths) -> PathBuf {
    std::env::var_os("HERDR_PLUGIN_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.state.join("adapters"))
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let parent = path.parent().ok_or("Path has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".infobox-{}", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        let mut file = options.open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if temp.exists() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn entries(provider: Provider, launcher: &Path) -> Result<Vec<(String, Value)>> {
    let command = quote(launcher.to_str().ok_or("Launcher path must be UTF-8")?);
    let events: &[&str] = match provider {
        Provider::Claude => &["SessionStart", "PreToolUse", "PostToolUse", "PostToolUseFailure", "SubagentStart", "SubagentStop", "SessionEnd"],
        Provider::Codex => &["SessionStart", "PreToolUse", "PostToolUse", "SubagentStart", "SubagentStop", "SessionEnd"],
        Provider::Cursor => &["sessionStart", "preToolUse", "postToolUse", "postToolUseFailure", "sessionEnd"],
        _ => return Err("Native adapter registration requires the documented provider-specific installation steps".into()),
    };
    Ok(events
        .iter()
        .map(|event| {
            let entry = if provider == Provider::Cursor {
                json!({"command": command, "timeout": 1})
            } else {
                json!({"hooks": [{"type":"command", "command":command, "timeout":1}]})
            };
            ((*event).to_string(), entry)
        })
        .collect())
}

pub fn configure(
    paths: &Paths,
    provider: Provider,
    target: Option<&Path>,
    dry_run: bool,
    uninstall: bool,
) -> Result<String> {
    if provider == Provider::OpenCode {
        return opencode(paths, dry_run, uninstall);
    }
    if provider == Provider::Devin {
        return Ok(format!(
            "{} automatic settings mutation is unavailable. Follow docs/installation.md. Existing configuration is unchanged.",
            provider.name()
        ));
    }
    let dir = directory(paths);
    let launcher = dir.join(format!("{}-ingest", provider.name()));
    let desired = entries(provider, &launcher)?;
    let mut fragment = json!({"hooks": {}});
    if provider == Provider::Cursor {
        fragment["version"] = json!(1);
    }
    for (event, entry) in &desired {
        fragment["hooks"][event] = json!([entry]);
    }
    let fragment_text = serde_json::to_string_pretty(&fragment)?;
    if dry_run && target.is_none() {
        return Ok(fragment_text);
    }
    if !uninstall && !dry_run {
        fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        let binary = fs::canonicalize(std::env::current_exe()?)?;
        let script = format!(
            "#!/bin/sh\n{} --state-dir {} ingest --provider {} >/dev/null || :\n",
            quote(binary.to_str().ok_or("Binary path must be UTF-8")?),
            quote(paths.state.to_str().ok_or("State path must be UTF-8")?),
            provider.name()
        );
        let old = dir.join(format!("{}-launcher-owned", provider.name()));
        if launcher.exists() && fs::read(&launcher)? != fs::read(&old).unwrap_or_default() {
            return Err(
                "Owned launcher was edited; preserve it and choose another config directory".into(),
            );
        }
        atomic_write(&launcher, script.as_bytes(), 0o700)?;
        atomic_write(&old, script.as_bytes(), 0o600)?;
    }
    let Some(target) = target else {
        if uninstall {
            return Err("Specify --config for uninstall; only recorded entries are removed".into());
        }
        return Ok(format!(
            "Merge this fragment into the provider's configuration. Existing settings were not changed.\n{fragment_text}"
        ));
    };
    let target = if target.exists() {
        fs::canonicalize(target)?
    } else {
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::canonicalize(parent)?.join(target.file_name().ok_or("Missing config filename")?)
    };
    let original = match fs::read(&target) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => b"{}".to_vec(),
        Err(e) => return Err(e.into()),
    };
    let mut document: Value = match serde_json::from_slice(&original) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => {
            return Ok(format!(
                "Configuration is not a JSON object. Merge manually to preserve JSONC/TOML formatting. No settings changed.\n{fragment_text}"
            ));
        }
    };
    if provider == Provider::Codex && !uninstall {
        let inline = target.parent().unwrap().join("config.toml");
        match fs::read_to_string(inline) {
            Ok(text) => {
                let config: toml::Value = toml::from_str(&text)
                    .map_err(|_| "Cannot inspect Codex config.toml safely. Merge hooks manually")?;
                if config.get("hooks").is_some() {
                    return Err("Codex inline hooks exist in this layer. Merge the fragment there; do not register hooks.json too".into());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if provider == Provider::Cursor && document.get("version").is_some_and(|v| v != &json!(1)) {
        return Err("Unsupported Cursor hook configuration version".into());
    }
    let key = crate::model::content_hash(target.as_os_str().as_encoded_bytes());
    let receipt_path = dir.join(format!("{}-{key}.json", provider.name()));
    let previous: Option<Receipt> = if receipt_path.exists() {
        Some(serde_json::from_slice(&fs::read(&receipt_path)?)?)
    } else {
        None
    };
    if uninstall && previous.is_none() {
        return Err("No ownership receipt for this configuration; nothing removed".into());
    }
    let mut owned = Vec::new();
    let mut preserved_edits = 0;
    if let Some(receipt) = previous {
        if receipt.target != target {
            return Err("Ownership receipt target mismatch".into());
        }
        if let Some(hooks) = document.get_mut("hooks").and_then(Value::as_object_mut) {
            for (event, entry) in receipt.entries {
                if let Some(array) = hooks.get_mut(&event).and_then(Value::as_array_mut) {
                    if let Some(index) = array.iter().position(|item| item == &entry) {
                        array.remove(index);
                        if !uninstall {
                            owned.push((event, entry));
                        }
                    } else if !uninstall {
                        return Err("An owned hook was edited. Preserve it and resolve the change before upgrading".into());
                    } else {
                        preserved_edits += 1;
                    }
                }
            }
        }
    }
    if !uninstall {
        if provider == Provider::Cursor {
            document["version"] = json!(1);
        }
        if document.get("hooks").is_none() {
            document["hooks"] = json!({});
        }
        let hooks = document["hooks"]
            .as_object_mut()
            .ok_or("hooks must be an object")?;
        for (event, entry) in desired {
            let array = hooks
                .entry(&event)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or("Hook event must be an array")?;
            if !array.contains(&entry) {
                array.push(entry.clone());
                if !owned
                    .iter()
                    .any(|pair| pair == &(event.clone(), entry.clone()))
                {
                    owned.push((event, entry));
                }
            }
        }
    }
    if dry_run {
        return Ok(format!(
            "Would {} owned {} hooks in {}. Existing entries are preserved.\n{}",
            if uninstall { "remove" } else { "install" },
            provider.name(),
            target.display(),
            if uninstall {
                String::new()
            } else {
                fragment_text
            }
        ));
    }
    fs::create_dir_all(&dir)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("registration.lock"))?;
    lock.lock()?;
    if target.exists() && fs::read(&target)? != original {
        return Err("Configuration changed during preparation; retry".into());
    }
    if !uninstall {
        let receipt = Receipt {
            target: target.clone(),
            entries: owned,
        };
        atomic_write(&receipt_path, &serde_json::to_vec_pretty(&receipt)?, 0o600)?;
    }
    let mut bytes = serde_json::to_vec_pretty(&document)?;
    bytes.push(b'\n');
    atomic_write(&target, &bytes, 0o600)?;
    if uninstall {
        fs::remove_file(receipt_path)?;
    }
    Ok(format!(
        "{} owned {} hooks in {}. Preserved {preserved_edits} edited entries. Provider trust and runtime verification remain required.",
        if uninstall { "Removed" } else { "Installed" },
        provider.name(),
        target.display()
    ))
}

fn opencode(paths: &Paths, dry_run: bool, uninstall: bool) -> Result<String> {
    let dir = directory(paths);
    let launcher = dir.join("opencode.ts");
    let ownership = dir.join("opencode-loader-owned");
    if uninstall {
        if !launcher.exists() {
            return Ok(
                "OpenCode loader is absent. Remove its file URL from your plugin array if present."
                    .into(),
            );
        }
        if fs::read(&launcher)? != fs::read(&ownership).unwrap_or_default() {
            return Err("OpenCode loader was edited; nothing removed".into());
        }
        if !dry_run {
            fs::remove_file(&launcher)?;
            fs::remove_file(&ownership)?;
        }
        return Ok(format!(
            "{} owned loader. Remove only {} from the OpenCode plugin array. Other settings are unchanged.",
            if dry_run { "Would remove" } else { "Removed" },
            launcher.display()
        ));
    }
    let plugin_root = std::env::var_os("HERDR_PLUGIN_ROOT").map(PathBuf::from)
        .ok_or("Set HERDR_PLUGIN_ROOT to the installed infobox directory containing adapters/opencode/index.ts")?;
    let bridge = fs::canonicalize(plugin_root.join("adapters/opencode/index.ts"))?;
    let binary = fs::canonicalize(std::env::current_exe()?)?;
    let bridge_url =
        url::Url::from_file_path(&bridge).map_err(|_| "Bridge path must be absolute")?;
    let source = format!(
        "import {{ createInfobox }} from {};\nexport const Infobox = createInfobox({}, {});\n",
        serde_json::to_string(bridge_url.as_str())?,
        serde_json::to_string(binary.to_str().ok_or("Binary path must be UTF-8")?)?,
        serde_json::to_string(paths.state.to_str().ok_or("State path must be UTF-8")?)?
    );
    if launcher.exists() && fs::read(&launcher)? != fs::read(&ownership).unwrap_or_default() {
        return Err("OpenCode loader was edited; preserve it before upgrading".into());
    }
    if !dry_run {
        atomic_write(&launcher, source.as_bytes(), 0o600)?;
        atomic_write(&ownership, source.as_bytes(), 0o600)?;
    }
    let loader_url =
        url::Url::from_file_path(&launcher).map_err(|_| "Loader path must be absolute")?;
    Ok(format!(
        "Merge this plugin entry into OpenCode JSON/JSONC configuration. Preserve existing plugin entries. Runtime compatibility is unverified.\n{}",
        serde_json::to_string_pretty(&json!({"plugin":[loader_url.as_str()]}))?
    ))
}

pub fn status(paths: &Paths) -> Result<Vec<String>> {
    let dir = directory(paths);
    if !dir.exists() {
        return Ok(vec![
            "Adapters not configured; hook execution not verified".into(),
        ]);
    }
    let mut result = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let receipt: Receipt = serde_json::from_slice(&fs::read(path)?)?;
            result.push(format!(
                "Registered settings {}. Hook execution not verified",
                receipt.target.display()
            ));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_preserves_foreign_and_edited_entries() -> Result<()> {
        let root = tempfile::tempdir()?;
        let paths = Paths::open(Some(root.path().join("state")))?;
        let target = root.path().join("settings.json");
        let foreign = json!({"hooks":[{"type":"command","command":"keep-me"}]});
        fs::write(
            &target,
            serde_json::to_vec(&json!({"other":42,"hooks":{"SessionStart":[foreign.clone()]}}))?,
        )?;
        configure(&paths, Provider::Claude, Some(&target), false, false)?;
        configure(&paths, Provider::Claude, Some(&target), false, false)?;
        let mut doc: Value = serde_json::from_slice(&fs::read(&target)?)?;
        assert_eq!(doc["hooks"]["SessionStart"].as_array().unwrap().len(), 2);
        doc["hooks"]["PostToolUse"][0]["hooks"][0]["command"] = json!("user-edited");
        fs::write(&target, serde_json::to_vec(&doc)?)?;
        configure(&paths, Provider::Claude, Some(&target), false, true)?;
        let doc: Value = serde_json::from_slice(&fs::read(target)?)?;
        assert_eq!(doc["other"], 42);
        assert_eq!(doc["hooks"]["SessionStart"], json!([foreign]));
        assert_eq!(
            doc["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
            "user-edited"
        );
        Ok(())
    }
    #[test]
    fn inline_codex_hooks_prevent_duplicate_layer_registration() -> Result<()> {
        for text in [
            "hooks = { SessionStart = [] }",
            "[hooks]\nSessionStart = []",
            "\"hooks\" = { SessionStart = [] }",
        ] {
            let root = tempfile::tempdir()?;
            let paths = Paths::open(Some(root.path().join("state")))?;
            fs::write(root.path().join("config.toml"), text)?;
            let target = root.path().join("hooks.json");
            assert!(configure(&paths, Provider::Codex, Some(&target), false, false).is_err());
            assert!(!target.exists());
        }
        Ok(())
    }
    #[test]
    fn jsonc_is_never_rewritten() -> Result<()> {
        let root = tempfile::tempdir()?;
        let paths = Paths::open(Some(root.path().join("state")))?;
        let target = root.path().join("settings.json");
        fs::write(&target, b"{ // preserve comments\n}")?;
        let output = configure(&paths, Provider::Claude, Some(&target), true, false)?;
        assert!(output.contains("Merge manually"));
        assert_eq!(fs::read(target)?, b"{ // preserve comments\n}");
        Ok(())
    }
}
