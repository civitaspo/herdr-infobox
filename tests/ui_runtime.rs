#![cfg(unix)]

use rustix::{
    fs::{Mode, OFlags, fcntl_setfl, open},
    io::{FdFlags, fcntl_setfd},
    pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt},
    termios::{Winsize, tcsetwinsize},
};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Pane {
    child: Child,
    master: File,
    capture: Vec<u8>,
}

impl Pane {
    fn start(mut command: Command, session: &str) -> Self {
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
        fcntl_setfd(&master, FdFlags::CLOEXEC).unwrap();
        grantpt(&master).unwrap();
        unlockpt(&master).unwrap();
        let slave_name = ptsname(&master, Vec::new()).unwrap();
        let slave = File::from(
            open(
                slave_name.as_c_str(),
                OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap(),
        );
        tcsetwinsize(
            &slave,
            Winsize {
                ws_row: 12,
                ws_col: 30,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .unwrap();
        fcntl_setfl(&master, OFlags::NONBLOCK).unwrap();
        let child = command
            .args(["ui", "--session", session])
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave))
            .spawn()
            .unwrap();
        Self {
            child,
            master: File::from(master),
            capture: Vec::new(),
        }
    }

    fn send(&mut self, keys: &[u8]) {
        self.master.write_all(keys).unwrap();
    }

    fn until(&mut self, condition: impl Fn(&[u8]) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let mut bytes = [0; 65536];
            if let Ok(size) = self.master.read(&mut bytes) {
                self.capture.extend_from_slice(&bytes[..size]);
            }
            if condition(&self.capture) {
                return;
            }
            let status = self.child.try_wait().unwrap();
            assert!(
                Instant::now() < deadline && status.is_none(),
                "TUI condition not reached; status={status:?}; output={:?}",
                String::from_utf8_lossy(&self.capture)
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn close(mut self) {
        self.send(b"q");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "TUI exit status={status}");
                return;
            }
            assert!(Instant::now() < deadline, "TUI did not exit");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-infobox"));
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("HERDR_") || name.starts_with("INFOBOX_") {
            command.env_remove(key);
        }
    }
    let mut paths = vec![root.to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    command
        .arg("--state-dir")
        .arg(root.join("state"))
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("TERM", "xterm-256color")
        .env("INFOBOX_TEST_OPENED", root.join("opened"));
    command
}

fn cli(root: &Path, args: &[&str]) -> String {
    let output = command(root).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

fn executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn pinned_count(state: &Path) -> usize {
    fs::read_dir(state)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("ui-") && name.ends_with(".json")
        })
        .filter_map(|entry| {
            serde_json::from_slice::<serde_json::Value>(&fs::read(entry.path()).ok()?).ok()
        })
        .filter(|value| value["pinned"] == true)
        .count()
}

#[test]
fn narrow_terminal_plan_actions_and_instance_pins() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let plan = root.join("plan.md");
    fs::write(&plan, "# Expected plan\nDo the work.\n").unwrap();
    for name in ["open", "xdg-open"] {
        executable(
            &root.join(name),
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$INFOBOX_TEST_OPENED\"\n",
        );
    }
    let session = cli(
        root,
        &[
            "session",
            "add",
            "--provider",
            "claude",
            "--native-id",
            "ui-proof",
        ],
    );
    cli(
        root,
        &[
            "ref",
            "add",
            "--session",
            &session,
            "--url",
            "https://unrelated.example.test",
        ],
    );
    cli(
        root,
        &[
            "plan",
            "attach",
            "--session",
            &session,
            "--file",
            plan.to_str().unwrap(),
        ],
    );
    let mut pane = Pane::start(command(root), &session);
    pane.until(|output| output.windows(8).any(|bytes| bytes == b"ui-proof"));
    pane.send(b"\t\ty");
    let expected = b"\x1b]52;c;IyBFeHBlY3RlZCBwbGFuCkRvIHRoZSB3b3JrLgo=\x07";
    pane.until(|output| {
        output
            .windows(expected.len())
            .any(|bytes| bytes == expected)
    });
    pane.send(b"o");
    let canonical = plan.canonicalize().unwrap();
    pane.until(|_| {
        fs::read_to_string(root.join("opened"))
            .is_ok_and(|text| text.trim() == canonical.to_str().unwrap())
    });
    pane.close();

    let herdr = root.join("herdr");
    executable(
        &herdr,
        "#!/bin/sh\nprintf '%s\\n' '{\"result\":{\"snapshot\":{\"panes\":[],\"focused_tab_id\":\"same-tab\",\"focused_pane_id\":null}}}'\n",
    );
    for index in 1..=2 {
        let socket = root.join(format!("socket{index}"));
        File::create(&socket).unwrap();
        let mut command = command(root);
        command
            .env("HERDR_BIN_PATH", &herdr)
            .env("HERDR_SOCKET_PATH", socket)
            .env("HERDR_TAB_ID", "same-tab");
        let mut pane = Pane::start(command, &session);
        pane.until(|output| output.windows(8).any(|bytes| bytes == b"ui-proof"));
        pane.send(b"p");
        pane.until(|_| pinned_count(&root.join("state")) == index);
        pane.close();
    }
    assert_eq!(pinned_count(&root.join("state")), 2);
}
