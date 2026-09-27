use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn cli(state: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_herdr-infobox"))
        .arg("--state-dir")
        .arg(state)
        .args(args)
        .env_remove("HERDR_SOCKET_PATH")
        .env_remove("INFOBOX_CREATION_TOKEN")
        .env_remove("HERDR_TAB_ID")
        .env_remove("INFOBOX_TARGET_PANE")
        .output()
        .unwrap()
}
#[test]
fn explicit_missing_session_never_displays_another_session() {
    let temp = tempfile::tempdir().unwrap();
    let empty = cli(temp.path(), &["ui", "--session", "missing", "--once"]);
    assert!(!empty.status.success());
    assert!(empty.stdout.is_empty());
    assert!(
        cli(
            temp.path(),
            &[
                "session",
                "add",
                "--provider",
                "claude",
                "--native-id",
                "actual"
            ]
        )
        .status
        .success()
    );
    let output = cli(temp.path(), &["ui", "--session", "missing", "--once"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown session"));
}
#[test]
fn raw_snapshots_are_listed_and_purged_without_touching_annotations() {
    let temp = tempfile::tempdir().unwrap();
    assert!(cli(temp.path(), &["session", "list"]).status.success());
    let hash = "a".repeat(64);
    let raw = temp.path().join("snapshots").join(format!("{hash}.patch"));
    fs::write(&raw, b"invalid UTF-8 patch\xff").unwrap();
    let annotation = temp
        .path()
        .join("snapshots")
        .join(format!("{hash}.annotations.json"));
    fs::write(&annotation, "saved annotation").unwrap();
    let output = cli(temp.path(), &["snapshot", "list"]);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("{hash}.patch")));
    assert!(
        cli(temp.path(), &["snapshot", "purge", "--hash", &hash])
            .status
            .success()
    );
    assert!(!raw.exists());
    assert_eq!(fs::read_to_string(annotation).unwrap(), "saved annotation");
}
