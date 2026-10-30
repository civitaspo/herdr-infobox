use herdr_infobox::{
    annotate,
    config::Paths,
    herdr::{self, Herdr, Pane, Snapshot},
};
use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
fn pane(id: &str, terminal: &str, agent: bool) -> serde_json::Value {
    json!({"pane_id":id,"terminal_id":terminal,"workspace_id":"w","tab_id":"t","focused":true,"agent":if agent{Some("claude")}else{None},"agent_session":if agent{Some(json!({"source":"herdr:claude","agent":"claude","kind":"id","value":"session-one"}))}else{None}})
}
fn write(path: &Path, value: serde_json::Value) {
    fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
}
fn fixture() -> (tempfile::TempDir, Herdr, Paths) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join("socket"), "").unwrap();
    let initial = json!({"result":{"snapshot":{"panes":[pane("agent","terminal-agent",true)],"focused_pane_id":"agent","focused_tab_id":"t"}}});
    write(&root.join("snapshot.json"), initial.clone());
    write(&root.join("initial.json"), initial);
    write(
        &root.join("opened.json"),
        json!({"result":{"plugin_pane":{"pane":pane("info","terminal-info",false)}}}),
    );
    write(
        &root.join("after.json"),
        json!({"result":{"snapshot":{"panes":[pane("agent","terminal-agent",true),pane("info","terminal-info",false)],"focused_pane_id":"agent","focused_tab_id":"t"}}}),
    );
    write(
        &root.join("pane.json"),
        json!({"result":{"pane":pane("agent","terminal-agent",true)}}),
    );
    let binary = root.join("herdr");
    fs::write(&binary,format!("#!/bin/sh\ncd '{}' || exit 1\ncase \"$1 $2 $3\" in\n  'api snapshot ') cat snapshot.json;;\n  'pane get agent') cat pane.json;;\n  'plugin pane open') echo open >> calls; cp after.json snapshot.json; cat opened.json;;\n  'plugin pane close') echo close >> calls; cp initial.json snapshot.json; echo '{{\"result\":{{}}}}';;\n  'plugin list --plugin') cat plugins.json;;\n  *) exit 1;;\nesac\n",root.display())).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let paths = Paths::open(Some(root.join("state"))).unwrap();
    let herdr = Herdr {
        binary,
        socket: root.join("socket"),
    };
    (temp, herdr, paths)
}
#[test]
fn toggle_ensure_and_manual_close_do_not_duplicate_or_resurrect() {
    let (temp, herdr, paths) = fixture();
    assert_eq!(herdr::ensure(&paths, &herdr, false).unwrap(), None);
    assert_eq!(
        herdr::ensure(&paths, &herdr, true).unwrap(),
        Some("info".into())
    );
    assert_eq!(
        herdr::ensure(&paths, &herdr, false).unwrap(),
        Some("info".into())
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("calls")).unwrap(),
        "open\n"
    );
    fs::copy(
        temp.path().join("initial.json"),
        temp.path().join("snapshot.json"),
    )
    .unwrap();
    assert_eq!(herdr::ensure(&paths, &herdr, false).unwrap(), None);
    assert_eq!(herdr::ensure(&paths, &herdr, false).unwrap(), None);
    assert_eq!(
        fs::read_to_string(temp.path().join("calls")).unwrap(),
        "open\n"
    );
}

#[test]
fn split_pane_targets_a_pane_without_a_workspace_selector() {
    let (temp, herdr, _) = fixture();
    let binary = &herdr.binary;
    fs::write(
        binary,
        format!(
            "#!/bin/sh\nfor arg in \"$@\"; do\n  [ \"$arg\" != --workspace ] || exit 1\ndone\nprintf '%s\\n' \"$@\" > '{}'/args\ncat '{}'/opened.json\n",
            temp.path().display(),
            temp.path().display(),
        ),
    )
    .unwrap();
    let target: Pane = serde_json::from_value(pane("agent", "terminal-agent", true)).unwrap();
    assert_eq!(
        herdr.open("info", &target, &[], false).unwrap().pane_id,
        "info"
    );
    let args = fs::read_to_string(temp.path().join("args")).unwrap();
    assert!(args.contains("--target-pane\nagent\n"));
    assert!(args.contains("--placement\nsplit\n"));
}
#[test]
fn pane_reuse_and_plugin_focus_do_not_change_native_identity() {
    let agent: Pane = serde_json::from_value(pane("agent", "one", true)).unwrap();
    let plugin: Pane = serde_json::from_value(pane("info", "two", false)).unwrap();
    let snapshot = Snapshot {
        panes: vec![agent.clone(), plugin],
        focused_pane_id: Some("info".into()),
        focused_tab_id: Some("t".into()),
    };
    assert_eq!(
        herdr::follow(&snapshot, Some("agent"), "host")
            .unwrap()
            .1
            .native_session_id,
        "session-one"
    );
    let mut reused = agent;
    reused.agent_session.as_mut().unwrap().value = "session-two".into();
    assert_eq!(
        reused.session_key("host").unwrap().native_session_id,
        "session-two"
    );
    reused.agent_session.as_mut().unwrap().kind = "path".into();
    assert!(reused.session_key("host").is_none());
}
#[test]
fn copy_launch_has_no_delivery_context() {
    let command = annotate::copy_command(Path::new("/reviewer"), Path::new("/snapshot"));
    let removed: Vec<_> = command
        .get_envs()
        .filter(|(_, v)| v.is_none())
        .map(|(k, _)| k.to_str().unwrap())
        .collect();
    for required in [
        "HERDR_ENV",
        "HERDR_PLUGIN_CONTEXT_JSON",
        "PLANNOTATOR_TUI_DELIVER_TO",
        "PLANNOTATOR_TUI_DELIVER_AGENT",
    ] {
        assert!(removed.contains(&required));
    }
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        vec![std::ffi::OsStr::new("/snapshot")]
    );
}
#[test]
fn annotate_installation_requires_registered_full_plugin() {
    let (temp, herdr, _) = fixture();
    let root = temp.path().join("annotate");
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(
        root.join("bin/plannotator-tui.exe"),
        "#!/bin/sh\necho plannotator-tui 0.9.4\n",
    )
    .unwrap();
    fs::set_permissions(
        root.join("bin/plannotator-tui.exe"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    write(
        &temp.path().join("plugins.json"),
        json!({"result":{"plugins":[{"plugin_id":"annotate","enabled":true,"version":"0.6.0","plugin_root":root,"panes":[{"id":"doc"}]}]}}),
    );
    assert_eq!(
        annotate::installation(&herdr).unwrap(),
        root.join("bin/plannotator-tui.exe")
    );
    write(
        &temp.path().join("plugins.json"),
        json!({"result":{"plugins":[{"plugin_id":"annotate","enabled":true,"version":"0.6.0","plugin_root":root,"panes":[]}]}}),
    );
    assert!(annotate::installation(&herdr).is_err());
}

#[test]
fn review_rejects_reused_session_before_opening_pane() {
    let (temp, herdr, _) = fixture();
    let root = temp.path().join("annotate");
    fs::create_dir_all(root.join("bin")).unwrap();
    let binary = root.join("bin/plannotator-tui.exe");
    fs::write(&binary, "#!/bin/sh\necho plannotator-tui 0.9.4\n").unwrap();
    fs::set_permissions(binary, fs::Permissions::from_mode(0o700)).unwrap();
    write(
        &temp.path().join("plugins.json"),
        json!({"result":{"plugins":[{"plugin_id":"annotate","enabled":true,"version":"0.6.0","plugin_root":root,"panes":[{"id":"doc"}]}]}}),
    );
    let file = temp.path().join("snapshot.md");
    fs::write(&file, "retained diff").unwrap();
    let snapshot = annotate::Snapshot {
        hash: "test".into(),
        path: file,
        patch_hash: "patch".into(),
        content_hash: herdr_infobox::model::content_hash(b"retained diff"),
    };
    let original: Pane = serde_json::from_value(pane("agent", "terminal-agent", true)).unwrap();
    let mut changed = pane("agent", "terminal-agent", true);
    changed["agent_session"]["value"] = json!("other-session");
    write(
        &temp.path().join("pane.json"),
        json!({"result":{"pane":changed}}),
    );
    assert!(
        annotate::open_copy_review(&herdr, &original, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("reused")
    );
    assert!(!temp.path().join("calls").exists());
    fs::write(&snapshot.path, "changed after export").unwrap();
    assert!(
        annotate::open_copy_review(&herdr, &original, &snapshot)
            .unwrap_err()
            .to_string()
            .contains("modified")
    );
}

#[test]
fn lost_open_response_cannot_duplicate_and_ui_registration_recovers() {
    let (temp, herdr, paths) = fixture();
    let binary = fs::read_to_string(&herdr.binary)
        .unwrap()
        .replace("cat opened.json;;", "exit 1;;")
        .replace(
            "'pane get agent') cat pane.json;;",
            "'pane get agent') cat pane.json;;\n  'pane get info') cat info-pane.json;;",
        );
    fs::write(&herdr.binary, binary).unwrap();
    write(
        &temp.path().join("info-pane.json"),
        json!({"result":{"pane":pane("info","terminal-info",false)}}),
    );
    assert!(herdr::ensure(&paths, &herdr, true).is_err());
    assert!(
        herdr::ensure(&paths, &herdr, true)
            .unwrap_err()
            .to_string()
            .contains("unresolved")
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("calls")).unwrap(),
        "open\n"
    );
    let record = fs::read_dir(paths.state.join("panes"))
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.path().extension().is_some_and(|s| s == "json"))
        .unwrap()
        .path();
    let state: serde_json::Value = serde_json::from_slice(&fs::read(record).unwrap()).unwrap();
    let token = state["creation_token"].as_str().unwrap();
    herdr::register_pane(&paths, &herdr, token, "info").unwrap();
    assert_eq!(
        herdr::ensure(&paths, &herdr, false).unwrap(),
        Some("info".into())
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("calls")).unwrap(),
        "open\n"
    );
}
