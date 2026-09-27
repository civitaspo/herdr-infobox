use std::{
    io::Write,
    process::{Command, Stdio},
};
#[test]
fn bridge_provider_name_enters_registered_session() {
    let temp = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_herdr-infobox");
    let output = Command::new(binary)
        .arg("--state-dir")
        .arg(temp.path())
        .args([
            "session",
            "add",
            "--provider",
            "opencode",
            "--native-id",
            "opencode-example",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut child = Command::new(binary)
        .arg("--state-dir")
        .arg(temp.path())
        .args(["ingest", "--provider", "opencode"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(br#"{"infobox_schema":1,"sessionID":"opencode-example","event":"tool.execute.after","callID":"call-1","tool":"webfetch","args":{"url":"https://example.org/docs"},"output":{"output":"text","title":"https://example.org/docs (text/plain)","metadata":{}},"directory":"/tmp"}"#).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    let output = Command::new(binary)
        .arg("--state-dir")
        .arg(temp.path())
        .args(["ui", "--once", "--session", "opencode-example"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("https://example.org/docs"), "{text}");
}
