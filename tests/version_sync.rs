use std::{fs, path::Path, process::Command};

const FILES: [&str; 4] = [
    ".release-version",
    "Cargo.toml",
    "Cargo.lock",
    "herdr-plugin.toml",
];

fn contents(root: &Path) -> Vec<Vec<u8>> {
    FILES
        .iter()
        .map(|name| fs::read(root.join(name)).unwrap())
        .collect()
}

#[test]
fn version_command_repairs_stale_lock_without_cargo_and_preserves_unrelated_content() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let binary = root.join("sync-version");
    assert!(
        Command::new("rustc")
            .args(["--edition=2024", "-D", "warnings"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/sync-version.rs"))
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap()
            .success()
    );
    let original = [
        "0.4.0\n",
        "[package]\nname = \"herdr-infobox\"\nversion = \"0.0.0\" # preserve me\n[dependencies]\nexample = \"0.0.0\"\n",
        "version = 4\n\n[[package]]\nname = \"example\"\nversion = \"0.0.0\"\n\n[[package]]\nname = \"herdr-infobox\"\nversion = \"0.0.0\"\ndependencies = [\"example\"]\n",
        "id = \"herdr-infobox\"\nversion = \"0.0.0\"\nmin_herdr_version = \"0.9.1\"\n[[panes]]\nid = \"info\"\n",
    ];
    for (name, text) in FILES.iter().zip(original) {
        fs::write(root.join(name), text).unwrap();
    }
    let run = |check: bool| {
        let mut command = Command::new(&binary);
        command.arg("--root").arg(root);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    };
    let before = contents(root);
    let failed = run(true);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("Version mismatch"));
    assert_eq!(contents(root), before);
    assert!(run(false).status.success());
    assert_eq!(
        fs::read_to_string(root.join("Cargo.toml")).unwrap(),
        original[1].replacen("version = \"0.0.0\"", "version = \"0.4.0\"", 1)
    );
    assert_eq!(
        fs::read_to_string(root.join("Cargo.lock")).unwrap(),
        original[2].replace(
            "name = \"herdr-infobox\"\nversion = \"0.0.0\"",
            "name = \"herdr-infobox\"\nversion = \"0.4.0\""
        )
    );
    assert_eq!(
        fs::read_to_string(root.join("herdr-plugin.toml")).unwrap(),
        original[3].replacen("version = \"0.0.0\"", "version = \"0.4.0\"", 1)
    );
    let before = contents(root);
    assert!(run(true).status.success());
    assert!(run(false).status.success());
    assert_eq!(contents(root), before);
    for version in ["0.5.0-rc.2", "1.2.3-alpha-1.0"] {
        fs::write(root.join(".release-version"), version).unwrap();
        assert!(run(false).status.success());
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        assert_eq!(manifest["package"]["version"].as_str(), Some(version));
        assert!(run(true).status.success());
    }
    for version in [
        "0.5.0-rc.02",
        "01.2.3",
        "1.2",
        "1.2.3-",
        "1.2.3+build",
        "1.2.3-a..b",
    ] {
        fs::write(root.join(".release-version"), version).unwrap();
        let before = contents(root);
        assert!(!run(false).status.success(), "accepted {version}");
        assert_eq!(contents(root), before);
    }
    fs::write(root.join(".release-version"), "0.6.0").unwrap();
    for malformed in [
        "version = 4\n",
        "[[package]]\nname = \"herdr-infobox\"\nversion = \"0.1.0\"\nversion = \"0.2.0\"\n",
    ] {
        fs::write(root.join("Cargo.lock"), malformed).unwrap();
        let before = contents(root);
        assert!(!run(false).status.success());
        assert_eq!(contents(root), before);
    }
}
