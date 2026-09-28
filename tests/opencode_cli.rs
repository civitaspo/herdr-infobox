#![cfg(unix)]
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn cli(state: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_herdr-infobox"))
        .arg("--state-dir")
        .arg(state)
        .args(args)
        .output()
        .unwrap()
}
fn success(state: &Path, args: &[&str]) -> String {
    let output = cli(state, args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
#[test]
fn v2_connection_recovers_history_and_preserves_state_on_failure() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .arg(&repo)
            .status()
            .unwrap()
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/project.git"
            ])
            .status()
            .unwrap()
            .success()
    );
    fs::write(repo.join("new.rs"), "fn main() {}\n").unwrap();
    let mut export: Value =
        serde_json::from_slice(include_bytes!("../fixtures/opencode/v2.0.18/export.json")).unwrap();
    export["data"]["info"]["location"]["directory"] = json!(repo);
    for message in export["data"]["messages"].as_array_mut().unwrap() {
        if message["type"] == "location-switched" {
            message["location"]["directory"] = json!(repo);
            message["previous"]["location"]["directory"] = json!(repo);
        }
    }
    let input = temp.path().join("export.json");
    fs::write(&input, serde_json::to_vec(&export).unwrap()).unwrap();
    let executable = temp.path().join("opencode");
    fs::write(&executable, format!("#!/bin/sh\ncase \"$1\" in --version) echo opencode v2.0.18;; *) case \"$5\" in /api/info) echo '{{\"version\":\"2.0.18\"}}';; *) cat '{}' ;; esac;; esac\n", input.display())).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    success(
        &state,
        &[
            "session",
            "add",
            "--provider",
            "opencode",
            "--native-id",
            "ses_fixture",
        ],
    );
    let connected = Command::new(env!("CARGO_BIN_EXE_herdr-infobox"))
        .current_dir(temp.path())
        .arg("--state-dir")
        .arg(&state)
        .args([
            "opencode",
            "connect",
            "--session",
            "ses_fixture",
            "--server",
            "http://127.0.0.1:4096",
            "--binary",
            "./opencode",
        ])
        .output()
        .unwrap();
    assert!(
        connected.status.success(),
        "{}",
        String::from_utf8_lossy(&connected.stderr)
    );

    {
        let paths = herdr_infobox::config::Paths::open(Some(state.clone())).unwrap();
        let store = herdr_infobox::store::Store::open(&paths).unwrap();
        let session = store.resolve("ses_fixture").unwrap();
        let view = store.view(&session).unwrap();
        assert_eq!(
            view.plans[0].markdown,
            "# Proposed work\n\nImplement the feature."
        );
    }
    let first = success(&state, &["ui", "--session", "ses_fixture", "--once"]);
    assert!(first.contains("https://github.com/example/project"));
    assert!(first.contains("https://example.com/docs"));
    assert!(first.contains("msg_plan"));
    success(&state, &["reconcile", "--session", "ses_fixture"]);
    assert_eq!(
        first,
        success(&state, &["ui", "--session", "ses_fixture", "--once"])
    );
    let diff = success(
        &state,
        &[
            "diff",
            "--session",
            "ses_fixture",
            "--scope",
            "untracked",
            "--path",
            "new.rs",
        ],
    );
    assert!(diff.contains("+fn main() {}"));
    export["data"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "id":"msg_closed_ui", "type":"assistant", "agent":"plan",
            "time":{"created":2000,"completed":2001},
            "content":[{"type":"text","text":"# While UI was closed"}]
        }));
    fs::write(&input, serde_json::to_vec(&export).unwrap()).unwrap();
    success(&state, &["reconcile", "--session", "ses_fixture"]);
    assert!(
        success(&state, &["ui", "--session", "ses_fixture", "--once"]).contains("msg_closed_ui")
    );
    {
        use herdr_infobox::{config::Paths, model::*, store::Store};
        let paths = Paths::open(Some(state.clone())).unwrap();
        let mut store = Store::open(&paths).unwrap();
        let key = SessionKey {
            host_id: paths.host_id.clone(),
            provider: Provider::Claude,
            native_session_id: "other-provider".into(),
            agent_scope: "main".into(),
        };
        store.register(&key).unwrap();
        let batch = herdr_infobox::ingest::manual(
            key,
            vec![Observation::Reference {
                url: "https://example.com/independent".into(),
                title: None,
                title_source: None,
                relation: "manual".into(),
            }],
        );
        fs::write(
            paths.spool.join("pending.json"),
            serde_json::to_vec(&batch).unwrap(),
        )
        .unwrap();
    }
    fs::write(&input, "{partial").unwrap();
    assert!(!cli(&state, &["reconcile"]).status.success());
    assert!(
        success(&state, &["ui", "--session", "other-provider", "--once"])
            .contains("https://example.com/independent")
    );

    assert!(
        !cli(&state, &["reconcile", "--session", "ses_fixture"])
            .status
            .success()
    );
    assert!(
        success(&state, &["ui", "--session", "ses_fixture", "--once"]).contains("msg_closed_ui")
    );
    success(
        &state,
        &["opencode", "disconnect", "--session", "ses_fixture"],
    );
    success(&state, &["reconcile", "--session", "ses_fixture"]);
    assert!(success(&state, &["ui", "--session", "ses_fixture", "--once"]).contains("msg_plan"));
}
