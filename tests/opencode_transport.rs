#![cfg(unix)]

use herdr_infobox::opencode_transport::export;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Instant};
use tempfile::TempDir;

fn executable(body: &str) -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let binary = directory.path().join("opencode");
    fs::write(&binary, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    (directory, binary)
}

#[test]
fn explicit_server_and_encoded_session_are_passed_without_a_shell() {
    let (directory, binary) = executable(
        r#"if [ "$1" = --version ]; then printf 'opencode v2.0.18\n'; exit; fi
if [ "$5" = /api/info ]; then printf '{"version":"2.0.18"}\n'; exit; fi
printf '%s\n' "$@" > "$0.args"
printf '{"data":{"info":{"id":"ses_fixture"},"messages":[]}}\n'"#,
    );
    let bytes = export(&binary, "http://localhost:4096", "ses/a?b#c%20").unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["data"]["info"]["id"],
        "ses_fixture"
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("opencode.args")).unwrap(),
        "api\n--server\nhttp://localhost:4096/\nGET\n/api/experimental/session/ses%2Fa%3Fb%23c%2520/export\n"
    );
}

#[test]
fn invalid_servers_and_empty_ids_are_rejected_before_execution() {
    let (_directory, binary) = executable("exit 99");
    for server in [
        "",
        "relative",
        "file:///tmp/server",
        "http://user:secret@localhost",
        "http://localhost?secret=value",
        "http://localhost/#secret",
    ] {
        assert!(export(&binary, server, "ses_fixture").is_err());
    }
    assert!(export(&binary, "http://localhost", "").is_err());
}

#[test]
fn unknown_versions_and_failed_commands_do_not_expose_output() {
    let (_directory, binary) = executable("printf 'opencode v2.0.19\\n'");
    assert!(
        export(&binary, "http://localhost", "ses_fixture")
            .unwrap_err()
            .to_string()
            .contains("unverified")
    );
    let (_directory, binary) = executable(
        r#"if [ "$1" = --version ]; then printf 'opencode v2.0.18\n'; exit; fi
if [ "$5" = /api/info ]; then printf '{"version":"2.0.18"}\n'; exit; fi
printf 'private-session-body'; printf 'secret-token' >&2; exit 1"#,
    );
    assert_eq!(
        export(&binary, "http://localhost", "ses_fixture")
            .unwrap_err()
            .to_string(),
        "OpenCode command failed"
    );
}

#[test]
fn unknown_server_versions_are_rejected_before_export() {
    let (directory, binary) = executable(
        r#"if [ "$1" = --version ]; then printf 'opencode v2.0.18\n'; exit; fi
if [ "$5" = /api/info ]; then printf '{"version":"2.0.19"}\n'; exit; fi
touch "$0.exported""#,
    );
    assert!(
        export(&binary, "http://localhost", "ses_fixture")
            .unwrap_err()
            .to_string()
            .contains("server version is unverified")
    );
    assert!(!directory.path().join("opencode.exported").exists());
}

#[test]
fn excessive_output_and_hanging_commands_are_bounded() {
    let (_directory, binary) = executable(
        r#"if [ "$1" = --version ]; then printf 'opencode v2.0.18\n'; exit; fi
if [ "$5" = /api/info ]; then printf '{"version":"2.0.18"}\n'; exit; fi
exec dd if=/dev/zero bs=1048576 count=17 2>/dev/null"#,
    );
    assert!(export(&binary, "http://localhost", "ses_fixture").is_err());
    let (_directory, binary) = executable(
        r#"if [ "$1" = --version ]; then printf 'opencode v2.0.18\n'; exit; fi
if [ "$5" = /api/info ]; then printf '{"version":"2.0.18"}\n'; exit; fi
sleep 30"#,
    );
    let started = Instant::now();
    assert!(export(&binary, "http://localhost", "ses_fixture").is_err());
    assert!(started.elapsed().as_secs() < 10);
}

#[test]
#[ignore = "requires an explicitly supplied OpenCode 2.0.18 binary; never starts a coding session"]
fn released_cli_reads_a_fixture_http_server() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };
    let binary = PathBuf::from(
        std::env::var_os("INFOBOX_TEST_OPENCODE_BIN").expect("supply the released binary"),
    );
    let home = tempfile::tempdir().unwrap();
    let quote =
        |path: &std::path::Path| format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"));
    let (_wrapper_directory, binary) = executable(&format!(
        "export HOME={}\nexport XDG_CONFIG_HOME=\"$HOME/config\"\nexport XDG_DATA_HOME=\"$HOME/data\"\nexport XDG_STATE_HOME=\"$HOME/state\"\nexport XDG_CACHE_HOME=\"$HOME/cache\"\nunset OPENCODE_PASSWORD OPENCODE_CONFIG OPENCODE_CONFIG_CONTENT OPENCODE_CONFIG_DIR\nexec {} \"$@\"",
        quote(home.path()),
        quote(&binary),
    ));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let fixture = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut requests = Vec::new();
        while !stopped.load(Ordering::Relaxed) && Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let request = String::from_utf8(request).unwrap();
            let path = request.split_whitespace().nth(1).unwrap().to_owned();
            let body = match path.as_str() {
                "/api/info" => r#"{"version":"2.0.18","pid":1,"urls":[],"paths":{"tmp":"/tmp"}}"#,
                "/api/experimental/session/ses_fixture/export" => {
                    r#"{"data":{"info":{"id":"ses_fixture"},"messages":[]}}"#
                }
                other => panic!("unexpected request {other}"),
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            requests.push(path);
        }
        requests
    });
    let result = export(&binary, &server, "ses_fixture");
    stop.store(true, Ordering::Relaxed);
    let requests = fixture.join().unwrap();
    let bytes = result.unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["data"]["info"]["id"],
        "ses_fixture"
    );
    assert!(
        requests
            .iter()
            .any(|path| path == "/api/experimental/session/ses_fixture/export")
    );
}
