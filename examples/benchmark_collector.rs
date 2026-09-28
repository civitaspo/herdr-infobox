use herdr_infobox::Result;
use serde_json::json;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

fn run(command: &mut Command, input: &[u8]) -> Result<Output> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let err = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let status = (|| -> Result<_> {
        child.stdin.take().unwrap().write_all(input)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err("Collector process exceeded 10 seconds".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    })();
    if status.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = out.join().map_err(|_| "stdout reader panicked")??;
    let stderr = err.join().map_err(|_| "stderr reader panicked")??;
    let output = Output {
        status: status?,
        stdout,
        stderr,
    };
    if !output.status.success() {
        return Err(format!(
            "Collector failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output)
}

fn main() -> Result<()> {
    let binary = PathBuf::from(std::env::args_os().nth(1).ok_or(
        "Usage: cargo run --example benchmark_collector -- /absolute/path/to/herdr-infobox",
    )?)
    .canonicalize()?;
    let state = tempfile::tempdir()?;
    let command = || {
        let mut command = Command::new(&binary);
        command.arg("--state-dir").arg(state.path());
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if ["HERDR_", "INFOBOX_", "DEVIN_"]
                .iter()
                .any(|p| name.starts_with(p))
            {
                command.env_remove(key);
            }
        }
        command
    };
    let session = run(
        command().args([
            "session",
            "add",
            "--provider",
            "claude",
            "--native-id",
            "synthetic-benchmark",
        ]),
        &[],
    )?;
    let session = String::from_utf8(session.stdout)?;
    let mut samples = Vec::new();
    for index in 0..105 {
        let event = json!({
            "session_id": "synthetic-benchmark", "cwd": state.path(),
            "hook_event_name": "PreToolUse", "tool_name": "WebFetch",
            "tool_use_id": format!("benchmark-{index}"),
            "tool_input": {
                "url": format!("https://example.test/benchmark/{index}"),
                "prompt": "Synthetic benchmark"
            }
        });
        let payload = serde_json::to_vec(&event)?;
        let start = Instant::now();
        let output = run(command().args(["ingest", "--provider", "claude"]), &payload)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        if !output.stdout.is_empty() || !output.stderr.is_empty() {
            return Err("Collector was not silent".into());
        }
        if index >= 5 {
            samples.push(elapsed);
        }
    }
    let display = run(
        command().args(["ui", "--session", session.trim(), "--once"]),
        &[],
    )?;
    let display = String::from_utf8(display.stdout)?;
    if !display.contains("References 105")
        || !(0..105).all(|i| display.contains(&format!("https://example.test/benchmark/{i}\n")))
    {
        return Err("Collector did not persist all 105 distinct references".into());
    }
    samples.sort_by(f64::total_cmp);
    let round = |value: f64| (value * 1000.0).round() / 1000.0;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "binary": binary,
            "host": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            "warmup": 5, "samples": samples.len(),
            "p50_ms": round((samples[49] + samples[50]) / 2.0),
            "p95_ms": round(samples[94]), "max_ms": round(samples[99]),
            "silent_processes": 105, "persisted_references": 105
        }))?
    );
    Ok(())
}
