use herdr_infobox::Result;
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn command(state: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_herdr-infobox"));
    cmd.arg("--state-dir")
        .arg(state)
        .env_remove("DEVIN_PROJECT_DIR")
        .env_remove("HERDR_SOCKET_PATH")
        .env_remove("HERDR_PLUGIN_CONFIG_DIR");
    cmd
}
fn cli(state: &Path, args: &[&str]) -> String {
    let output = command(state).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn git(path: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn fixture(state: &Path, root: &Path, filename: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/claude/docs-2026-09-28")
        .join(filename);
    let mut value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    value["cwd"] = Value::String(root.to_str().unwrap().into());
    let mut child = command(state)
        .args(["ingest", "--provider", "claude"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&value).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn claude_fixture_survives_restart_with_two_repositories_and_fixed_snapshot() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let state = dir.path().join("state");
    let api = dir.path().join("api");
    let infra = dir.path().join("infra");
    for root in [&api, &infra] {
        fs::create_dir(root)?;
        git(root, &["init", "-q"]);
        git(root, &["config", "user.name", "Fixture"]);
        git(root, &["config", "user.email", "fixture@example.invalid"]);
        fs::write(root.join("file.txt"), "before\n")?;
        git(root, &["add", "--", "file.txt"]);
        git(
            root,
            &["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"],
        );
    }
    git(
        &api,
        &["remote", "add", "origin", "git@github.com:example/api.git"],
    );
    git(
        &infra,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/example/infra.git",
        ],
    );
    cli(
        &state,
        &[
            "session",
            "add",
            "--provider",
            "claude",
            "--native-id",
            "session-example",
        ],
    );
    cli(
        &state,
        &[
            "repo",
            "add",
            "--session",
            "session-example",
            "--path",
            infra.to_str().unwrap(),
        ],
    );
    fixture(&state, &api, "webfetch-request.json");
    fixture(&state, &api, "webfetch-success.json");
    fixture(&state, &api, "search.json");
    fixture(&state, &api, "plan-proposed.json");
    fixture(&state, &api, "plan-approved.json");
    fixture(&state, &api, "plan-approved.json");
    cli(&state, &["reconcile", "--session", "session-example"]);
    let rendered = cli(&state, &["ui", "--session", "session-example", "--once"]);
    assert!(
        rendered.contains("https://github.com/example/api"),
        "{rendered}"
    );
    assert!(
        rendered.contains("https://github.com/example/infra"),
        "{rendered}"
    );
    assert!(rendered.contains("Approved"), "{rendered}");
    assert!(rendered.contains("Unknown"), "{rendered}");
    assert!(rendered.contains("https://example.com"), "{rendered}");
    assert_eq!(
        rendered,
        cli(&state, &["ui", "--session", "session-example", "--once"])
    );
    cli(
        &state,
        &[
            "session",
            "add",
            "--provider",
            "codex",
            "--native-id",
            "session-example",
        ],
    );
    let other = cli(
        &state,
        &["ui", "--session", "codex:session-example", "--once"],
    );
    assert!(!other.contains("https://example.com"));
    let paths = herdr_infobox::config::Paths::open(Some(state.clone()))?;
    let store = herdr_infobox::store::Store::open(&paths)?;
    let session = store.resolve("claude:session-example")?;
    let view = store.view(&session)?;
    let worktree = view
        .worktrees
        .iter()
        .find(|w| w.root == api.canonicalize().unwrap())
        .unwrap();
    assert_eq!(view.plans.len(), 2);
    fs::write(api.join("file.txt"), "displayed\n```\n")?;
    let snapshot = cli(
        &state,
        &[
            "snapshot",
            "export",
            "--session",
            "claude:session-example",
            "--worktree",
            &worktree.id,
            "--scope",
            "unstaged",
        ],
    );
    let snapshot = Path::new(snapshot.trim());
    let first = fs::read(snapshot)?;
    assert!(String::from_utf8_lossy(&first).contains("+displayed"));
    fs::write(api.join("file.txt"), "later unrelated edit\n")?;
    assert_eq!(first, fs::read(snapshot)?);
    assert_eq!(
        view.plans
            .iter()
            .filter(|p| p.phase == herdr_infobox::model::DocumentPhase::Approved)
            .count(),
        1
    );
    Ok(())
}

#[test]
fn collector_returns_without_waiting_for_producer_to_close_stdin() -> Result<()> {
    let dir = tempfile::tempdir()?;
    cli(
        dir.path(),
        &[
            "session",
            "add",
            "--provider",
            "claude",
            "--native-id",
            "example",
        ],
    );
    let mut child = command(dir.path())
        .args(["ingest", "--provider", "claude"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let input = child.stdin.take().unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(2) {
            let _ = child.kill();
            panic!("collector waited for an open producer");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(input);
    let output = child.wait_with_output()?;
    assert!(output.stdout.is_empty());
    Ok(())
}

#[test]
fn cursor_registered_transcript_recovers_without_resupplying_file() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let state = dir.path().join("state");
    let native_id = "cursor-recovery";
    let folder = dir.path().join(native_id);
    fs::create_dir(&folder)?;
    let file = folder.join(format!("{native_id}.jsonl"));
    let request = serde_json::json!({"role":"assistant","message":{"content":[{
        "type":"tool_use","name":"CallDynamicTool","input":{
            "namespace":"cursor","toolName":"WebFetch","arguments":{"url":"https://example.com/recovery"}
        }
    }]}});
    fs::write(&file, format!("{request}\n"))?;
    cli(
        &state,
        &[
            "session",
            "add",
            "--provider",
            "cursor",
            "--native-id",
            native_id,
        ],
    );
    cli(
        &state,
        &[
            "reconcile",
            "--session",
            native_id,
            "--file",
            file.to_str().unwrap(),
        ],
    );
    let before = cli(&state, &["ui", "--session", native_id, "--once"]);
    assert!(before.contains("https://example.com/recovery"), "{before}");
    assert!(before.contains("open_requested"), "{before}");

    let plan = serde_json::json!({"role":"assistant","message":{"content":[{
        "type":"tool_use","name":"CreatePlan","input":{
            "name":"Recovery plan","plan":"# Recovery plan\nKeep this exact text.",
            "todos":[{"id":"verify","content":"Verify recovery"}]
        }
    }]}});
    fs::OpenOptions::new()
        .append(true)
        .open(&file)?
        .write_all(format!("{plan}\n").as_bytes())?;
    let failed_import = command(&state)
        .args(["reconcile", "--session", native_id, "--file"])
        .arg(dir.path().join("missing.jsonl"))
        .output()?;
    assert!(!failed_import.status.success());
    let after = cli(&state, &["ui", "--session", native_id, "--once"]);
    assert!(after.contains("Plan revisions 1"), "{after}");
    assert!(after.contains("Proposed"), "{after}");
    assert!(after.contains("Verify recovery"), "{after}");
    cli(&state, &["reconcile", "--session", native_id]);
    assert_eq!(
        after,
        cli(&state, &["ui", "--session", native_id, "--once"])
    );

    fs::remove_file(&file)?;
    let failed = command(&state)
        .args(["reconcile", "--session", native_id])
        .output()?;
    assert!(!failed.status.success());
    let cached = cli(&state, &["ui", "--session", native_id, "--once"]);
    assert!(cached.contains("Plan revisions 1"), "{cached}");
    assert!(cached.contains("transcript_sync: unavailable"), "{cached}");
    Ok(())
}

#[test]
fn doctor_distinguishes_registration_from_runtime_without_exposing_content() -> Result<()> {
    let dir = tempfile::tempdir()?;
    cli(
        dir.path(),
        &[
            "session",
            "add",
            "--provider",
            "claude",
            "--native-id",
            "private-id",
        ],
    );
    cli(
        dir.path(),
        &[
            "ref",
            "add",
            "--session",
            "private-id",
            "--url",
            "https://example.com/?token=private",
            "--title",
            "Private title",
        ],
    );
    let output = command(dir.path())
        .args(["doctor", "--json"])
        .env("HERDR_BIN_PATH", dir.path().join("missing-herdr"))
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    let report: Value = serde_json::from_str(&text)?;
    assert!(!text.contains("private"));
    assert_eq!(report["hook_registration_is_not_execution"], true);
    assert_eq!(report["herdr"]["state"], "Unavailable");
    Ok(())
}

#[cfg(unix)]
#[test]
fn transcript_fifo_without_writer_does_not_block() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let fifo = dir.path().join("pipe");
    assert!(Command::new("mkfifo").arg(&fifo).status()?.success());
    cli(
        dir.path(),
        &[
            "session",
            "add",
            "--provider",
            "cursor",
            "--native-id",
            "fifo",
        ],
    );
    let mut child = command(dir.path())
        .args(["reconcile", "--session", "fifo", "--file"])
        .arg(&fifo)
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            assert!(!status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(2) {
            child.kill()?;
            let _ = child.wait();
            panic!("transcript reader blocked on a FIFO");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output()?;
    assert!(String::from_utf8_lossy(&output.stderr).contains("regular file"));
    Ok(())
}
