#!/usr/bin/env python3
"""Synchronize the root Rust package and Herdr manifest with .release-version."""
import argparse
from pathlib import Path
import re
import sys


def synchronize(root, check):
    version = (root / ".release-version").read_text().strip()
    match = re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?", version)
    if not match or any(part.isdigit() and len(part) > 1 and part[0] == "0" for part in (match.group(4) or "").split(".")):
        raise ValueError("Invalid release version")
    replacements = {}
    for filename, pattern in (
        ("Cargo.toml", r'(?ms)(^\[package\]\n(?:(?!^\[).)*?^version\s*=\s*)"[^"]+"'),
        ("Cargo.lock", r'(?ms)(^\[\[package\]\]\nname = "herdr-infobox"\nversion = )"[^"]+"'),
        ("herdr-plugin.toml", r'(?ms)\A((?:(?!^\[).)*?^version\s*=\s*)"[^"]+"'),
    ):
        path = root / filename
        old = path.read_text()
        new, count = re.subn(pattern, lambda m: m.group(1) + '"' + version + '"', old)
        if count != 1:
            raise ValueError(f"Expected exactly one package version in {filename}, found {count}")
        if new != old:
            replacements[path] = new
    if check and replacements:
        raise ValueError("Version mismatch: " + ", ".join(path.name for path in replacements))
    for path, text in replacements.items():
        path.write_text(text)
    print(f"Package, lockfile, and Herdr manifest match {version}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    args = parser.parse_args()
    try:
        synchronize(args.root, args.check)
    except (OSError, ValueError) as error:
        sys.exit(str(error))
