use herdr_infobox::{
    config::Paths,
    ingest,
    model::{Provider, SessionKey},
    store::Store,
};
use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn independent_hook_processes_are_durable_while_ui_connection_remains_open() {
    let temp = tempfile::tempdir().unwrap();
    let paths = Paths::open(Some(temp.path().into())).unwrap();
    let mut ui = Store::open(&paths).unwrap();
    let key = SessionKey {
        host_id: paths.host_id.clone(),
        provider: Provider::Claude,
        native_session_id: "multi-process".into(),
        agent_scope: "main".into(),
    };
    let id = ui.register(&key).unwrap();
    let mut children = Vec::new();
    for index in 0..16 {
        let mut child = Command::new(env!("CARGO_BIN_EXE_herdr-infobox"))
            .arg("--state-dir")
            .arg(temp.path())
            .args(["ingest", "--provider", "claude"])
            .env_remove("DEVIN_PROJECT_DIR")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let event = serde_json::json!({"session_id":"multi-process","hook_event_name":"PreToolUse","tool_use_id":format!("call-{index}"),"tool_name":"WebFetch","tool_input":{"url":format!("https://example.org/{index}"),"prompt":"fixture"}});
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&event).unwrap())
            .unwrap();
        children.push(child);
    }
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    ingest::drain(&paths, &mut ui).unwrap();
    assert_eq!(
        ui.view(&ui.resolve(&id).unwrap()).unwrap().references.len(),
        16
    );
    drop(ui);
    let reopened = Store::open(&paths).unwrap();
    assert_eq!(
        reopened
            .view(&reopened.resolve(&id).unwrap())
            .unwrap()
            .references
            .len(),
        16
    );
}
