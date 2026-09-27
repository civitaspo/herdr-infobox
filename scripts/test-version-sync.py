#!/usr/bin/env python3
"""Exercise release version synchronization in disposable package fixtures."""
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib

SCRIPT = Path(__file__).with_name("sync-version.py")


def run(root, *arguments):
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--root", str(root), *arguments],
        capture_output=True,
        text=True,
        check=False,
    )


def fixture(root):
    (root / ".release-version").write_text("0.4.0\n")
    (root / "Cargo.toml").write_text(
        '[package]\nname = "herdr-infobox"\nversion = "0.0.0" # preserve me\n'
        '[dependencies]\nexample = "0.0.0"\n'
    )
    (root / "Cargo.lock").write_text(
        'version = 4\n\n[[package]]\nname = "example"\nversion = "0.0.0"\n'
        'source = "registry+https://github.com/rust-lang/crates.io-index"\n\n'
        '[[package]]\nname = "herdr-infobox"\nversion = "0.0.0"\n'
        'dependencies = ["example"]\n'
    )
    (root / "herdr-plugin.toml").write_text(
        'id = "herdr-infobox"\nversion = "0.0.0"\nmin_herdr_version = "0.9.1"\n'
        '[[panes]]\nid = "info"\ncommand = ["./bin/herdr-infobox", "ui"]\n'
    )


def versions(root):
    cargo = tomllib.loads((root / "Cargo.toml").read_text())
    lock = tomllib.loads((root / "Cargo.lock").read_text())
    manifest = tomllib.loads((root / "herdr-plugin.toml").read_text())
    package = next(p for p in lock["package"] if p["name"] == "herdr-infobox")
    dependency = next(p for p in lock["package"] if p["name"] == "example")
    assert dependency["version"] == "0.0.0"
    assert cargo["dependencies"]["example"] == "0.0.0"
    assert manifest["min_herdr_version"] == "0.9.1"
    assert "# preserve me" in (root / "Cargo.toml").read_text()
    return cargo["package"]["version"], package["version"], manifest["version"]


with tempfile.TemporaryDirectory(prefix="infobox-version-test-") as directory:
    root = Path(directory)
    fixture(root)
    before = {p.name: p.read_bytes() for p in root.iterdir()}
    result = run(root, "--check")
    assert result.returncode != 0 and "Version mismatch" in result.stderr
    assert before == {p.name: p.read_bytes() for p in root.iterdir()}
    assert run(root).returncode == 0
    assert versions(root) == ("0.4.0",) * 3
    assert run(root, "--check").returncode == 0
    before = {p.name: p.read_bytes() for p in root.iterdir()}
    assert run(root).returncode == 0
    assert before == {p.name: p.read_bytes() for p in root.iterdir()}
    (root / ".release-version").write_text("0.5.0-rc.2\n")
    assert run(root).returncode == 0
    assert versions(root) == ("0.5.0-rc.2",) * 3
    (root / ".release-version").write_text("0.5.0-rc.02\n")
    before = {p.name: p.read_bytes() for p in root.iterdir()}
    assert run(root).returncode != 0
    assert before == {p.name: p.read_bytes() for p in root.iterdir()}
    (root / ".release-version").write_text("0.6.0\n")
    (root / "Cargo.lock").write_text('version = 4\n')
    before = {p.name: p.read_bytes() for p in root.iterdir()}
    assert run(root).returncode != 0
    assert before == {p.name: p.read_bytes() for p in root.iterdir()}
print("Release version synchronization passed")
