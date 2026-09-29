use crate::{Result, config::Paths, herdr::Herdr, store::Store};
use serde_json::{Value, json};
use std::{path::Path, process::Command, time::Duration};

fn inspect_binary(binary: &Path) -> Value {
    let output = crate::git::bounded(
        Command::new(binary).arg("--version"),
        4096,
        Duration::from_secs(2),
    );
    let version = match output {
        Ok((true, bytes, _)) => crate::model::safe_text(String::from_utf8_lossy(&bytes).trim()),
        _ => {
            return json!({"state":"Unavailable","reason":"Herdr binary could not report its version"});
        }
    };
    let supported_version = version
        .split_whitespace()
        .find_map(|word| {
            let numbers: Option<Vec<u32>> = word.split('.').map(|part| part.parse().ok()).collect();
            numbers.filter(|n| n.len() == 3)
        })
        .is_some_and(|n| (n[0], n[1], n[2]) >= (0, 9, 1));
    let schema = crate::git::bounded(
        Command::new(binary).args(["api", "schema", "--json"]),
        2 * 1024 * 1024,
        Duration::from_secs(5),
    );
    let schema = match schema {
        Ok((true, bytes, _)) => serde_json::from_slice::<Value>(&bytes).ok(),
        _ => None,
    };
    json!({"version":version,"minimum_version_satisfied":supported_version,
        "schema_available":schema.is_some(),"protocol":schema.and_then(|v|v.get("protocol").cloned()),
        "runtime_compatibility":"Unverified. Schema inspection is not a pane smoke test"})
}

pub fn report(paths: &Paths, store: &Store) -> Result<Value> {
    let binary = std::env::var_os("HERDR_BIN_PATH").unwrap_or_else(|| "herdr".into());
    let mut herdr = inspect_binary(Path::new(&binary));
    let mut annotate = json!({"state":"Unavailable","reason":"No Herdr connection context. Markdown export is available"});
    if let Ok(client) = Herdr::from_env() {
        match client.snapshot() {
            Ok(snapshot) => {
                herdr["connection"] = json!("Connected");
                herdr["panes"] = json!(snapshot.panes.len());
                herdr["identified_agent_panes"] = json!(
                    snapshot
                        .panes
                        .iter()
                        .filter(|p| p.session_key(&paths.host_id).is_some())
                        .count()
                );
            }
            Err(_) => {
                herdr["connection"] = json!("Disconnected. Current pane snapshot unavailable");
            }
        }
        annotate = match crate::annotate::installation(&client) {
            Ok(_) => {
                json!({"state":"Version checked","annotate":"0.6.0","reviewer":"0.9.4","delivery":"Copy only","runtime_verified":false})
            }
            Err(_) => {
                json!({"state":"Unavailable","reason":"Enabled annotate 0.6.0 doc entrypoint and plannotator-tui 0.9.4 are required. Check plugin registration and executable version"})
            }
        };
    } else {
        herdr["connection"] = json!("Not configured. HERDR_SOCKET_PATH is absent");
    }
    let losses = std::fs::read_to_string(paths.state.join("collection-loss-count"))
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok());
    let opencode_sessions: Vec<_> = store
        .sessions()?
        .into_iter()
        .filter(|s| crate::opencode_sync::configured(paths, s))
        .map(|s| s.id)
        .collect();
    Ok(
        json!({"version":env!("CARGO_PKG_VERSION"),"database":store.health()?,
        "adapters":crate::adapters::status(paths)?,"opencode_v2_connected_sessions":opencode_sessions,"herdr":herdr,"annotate":annotate,
        "spool_loss_counter":losses,"provider_runtime_verification":"Version- and collection-route-specific; see docs/compatibility.md and fixture compatibility.json",
        "hook_registration_is_not_execution":true}),
    )
}
