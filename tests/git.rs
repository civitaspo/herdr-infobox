use herdr_infobox::{
    annotate,
    config::Paths,
    git::{self, Scope},
    model::{Provider, SessionKey, SessionSummary, Worktree},
};
use std::{fs, path::Path, process::Command};
fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(dir.path(), &["config", "user.email", "test@example.test"]);
    git(dir.path(), &["config", "user.name", "Fixture"]);
    dir
}
fn commit(root: &Path) {
    git(root, &["add", "."]);
    git(
        root,
        &["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"],
    );
}
#[test]
fn staged_unstaged_untracked_snapshots_are_exact_and_read_only() {
    let dir = repo();
    let root = dir.path();
    fs::write(root.join("file"), "old\n").unwrap();
    commit(root);
    fs::write(root.join("file"), "staged\n").unwrap();
    git(root, &["add", "file"]);
    fs::write(root.join("file"), "working ```\n").unwrap();
    fs::write(root.join("new file"), "untracked\n").unwrap();
    let index = fs::read(root.join(".git/index")).unwrap();
    let changes = git::status(root).unwrap();
    assert!(
        changes
            .changes
            .iter()
            .any(|c| c.index == 'M' && c.worktree == 'M')
    );
    let staged = git::capture(root, Scope::Staged, Some(Path::new("file")), None).unwrap();
    assert!(String::from_utf8_lossy(&staged.patch).contains("+staged"));
    let diff = git::capture(root, Scope::Unstaged, Some(Path::new("file")), None).unwrap();
    assert!(String::from_utf8_lossy(&diff.patch).contains("+working ```"));
    let untracked =
        git::capture(root, Scope::Untracked, Some(Path::new("new file")), None).unwrap();
    assert!(String::from_utf8_lossy(&untracked.patch).contains("+untracked"));
    assert_eq!(index, fs::read(root.join(".git/index")).unwrap());
    let paths = Paths::open(Some(root.join("state"))).unwrap();
    let session = SessionSummary {
        id: "session".into(),
        key: SessionKey {
            host_id: paths.host_id.clone(),
            provider: Provider::Claude,
            native_session_id: "native".into(),
            agent_scope: "main".into(),
        },
        ended: false,
    };
    let worktree = Worktree {
        id: "w".into(),
        repository_id: "r".into(),
        root: root.into(),
        git_dir: root.join(".git"),
        common_dir: root.join(".git"),
        github_url: None,
        branch: None,
        relations: vec![],
    };
    let snapshot = annotate::export(&paths, &session, &worktree, &diff).unwrap();
    let bytes = fs::read(&snapshot.path).unwrap();
    assert!(bytes.windows(diff.patch.len()).any(|w| w == diff.patch));
    assert!(String::from_utf8_lossy(&bytes).contains("````diff"));
    fs::write(root.join("file"), "later\n").unwrap();
    assert_eq!(bytes, fs::read(&snapshot.path).unwrap());
    assert_eq!(
        snapshot.patch_hash,
        herdr_infobox::model::content_hash(&diff.patch)
    );
}
#[test]
fn linked_worktrees_have_distinct_git_dirs_and_same_common_dir() {
    let dir = repo();
    fs::write(dir.path().join("file"), "x").unwrap();
    commit(dir.path());
    let other = tempfile::tempdir().unwrap();
    let linked = other.path().join("linked");
    git(
        dir.path(),
        &["worktree", "add", "-qb", "other", linked.to_str().unwrap()],
    );
    let a = git::discover(dir.path(), dir.path()).unwrap();
    let b = git::discover(&linked, &linked).unwrap();
    assert_eq!(a.common_dir, b.common_dir);
    assert_ne!(a.git_dir, b.git_dir);
}
#[test]
fn unborn_binary_symlink_and_non_utf8_paths() {
    use std::os::unix::{ffi::OsStringExt, fs::symlink};
    let dir = repo();
    let root = dir.path();
    let name = std::ffi::OsString::from_vec(vec![b'x', 255, b'\n']);
    let status = git::parse_status(b"? x\xff\n\0").unwrap();
    assert_eq!(status.changes[0].path, Path::new(&name));
    fs::write(root.join("file"), "bytes").unwrap();
    git(root, &["add", "."]);
    assert!(
        git::capture(root, Scope::Staged, Some(Path::new("file")), None)
            .unwrap()
            .warnings
            .iter()
            .any(|w| w == "Unborn HEAD")
    );
    fs::write(root.join("binary"), [0, 1, 2]).unwrap();
    assert!(
        String::from_utf8_lossy(
            &git::capture(root, Scope::Untracked, Some(Path::new("binary")), None)
                .unwrap()
                .patch
        )
        .contains("Binary untracked")
    );
    symlink("/not/readable/target", root.join("link")).unwrap();
    let patch = git::capture(root, Scope::Untracked, Some(Path::new("link")), None)
        .unwrap()
        .patch;
    assert!(String::from_utf8_lossy(&patch).contains("+/not/readable/target"));
}
#[test]
fn status_rename_preserves_old_path_and_newline() {
    let parsed =
        git::parse_status(b"2 R. N... 100644 100644 100644 aaa bbb R100 new\nname\0old name\0")
            .unwrap();
    assert_eq!(parsed.changes[0].path, Path::new("new\nname"));
    assert_eq!(
        parsed.changes[0].old_path.as_deref(),
        Some(Path::new("old name"))
    );
}
#[test]
fn external_diff_driver_never_runs_and_branch_uses_commit_ids() {
    let dir = repo();
    let root = dir.path();
    fs::write(root.join("file"), "one\n").unwrap();
    fs::write(root.join(".gitattributes"), "file diff=unsafe\n").unwrap();
    commit(root);
    git(root, &["config", "diff.unsafe.command", "touch CALLED"]);
    git(root, &["config", "diff.unsafe.textconv", "touch CALLED"]);
    fs::write(root.join("file"), "two\n").unwrap();
    let diff = git::capture(root, Scope::Unstaged, None, None).unwrap();
    assert!(String::from_utf8_lossy(&diff.patch).contains("+two"));
    assert!(!root.join("CALLED").exists());
    commit(root);
    let branch = git::capture(root, Scope::Branch, None, Some("HEAD~1")).unwrap();
    assert!(branch.base.as_ref().is_some_and(|b| b.len() == 40));
    assert!(String::from_utf8_lossy(&branch.patch).contains("+two"));
}
#[test]
fn remote_credentials_are_removed_and_aliases_unresolved() {
    assert_eq!(
        git::github_url("https://user:secret@github.com/org/repo.git"),
        Some("https://github.com/org/repo".into())
    );
    assert_eq!(
        git::github_url("git@github.com:org/repo.git"),
        Some("https://github.com/org/repo".into())
    );
    assert_eq!(git::github_url("git@work:org/repo.git"), None);
}

#[test]
fn explicit_remote_and_host_mapping_are_read_only_configuration() {
    let dir = repo();
    let root = dir.path();
    git(
        root,
        &[
            "remote",
            "add",
            "origin",
            "git@github.com:wrong/project.git",
        ],
    );
    git(
        root,
        &["remote", "add", "work", "git@work-alias:team/project.git"],
    );
    git(root, &["config", "infobox.remote", "work"]);
    git(
        root,
        &[
            "config",
            "--add",
            "infobox.github-host",
            "work-alias=github.example.test",
        ],
    );
    let before = fs::read(root.join(".git/config")).unwrap();
    assert_eq!(
        git::discover(root, root).unwrap().github_url.as_deref(),
        Some("https://github.example.test/team/project")
    );
    assert_eq!(before, fs::read(root.join(".git/config")).unwrap());
    let mappings = [("work-alias".into(), "user:secret@evil.test".into())]
        .into_iter()
        .collect();
    assert!(git::github_url_with_hosts("git@work-alias:team/project", &mappings).is_none());
}
#[test]
fn conflicts_are_combined_and_oversized_untracked_is_rejected() {
    let dir = repo();
    let root = dir.path();
    fs::write(root.join("file"), "initial\n").unwrap();
    commit(root);
    git(root, &["checkout", "-qb", "left"]);
    fs::write(root.join("file"), "left\n").unwrap();
    commit(root);
    git(root, &["checkout", "-qb", "right", "HEAD~1"]);
    fs::write(root.join("file"), "right\n").unwrap();
    commit(root);
    let merge = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "commit.gpgsign=false", "merge", "left"])
        .output()
        .unwrap();
    assert!(!merge.status.success());
    assert!(
        git::status(root)
            .unwrap()
            .changes
            .iter()
            .any(|c| c.kind == "conflict")
    );
    let diff = git::capture(root, Scope::Conflicts, Some(Path::new("file")), None).unwrap();
    assert!(diff.warnings.iter().any(|w| w.contains("not a two-way")));
    assert!(String::from_utf8_lossy(&diff.patch).contains("diff --cc"));
    fs::write(root.join("large"), vec![b'x'; git::PATCH_LIMIT + 1]).unwrap();
    assert!(
        git::capture(root, Scope::Untracked, Some(Path::new("large")), None)
            .unwrap_err()
            .to_string()
            .contains("limit")
    );
}

#[test]
fn inherited_git_environment_cannot_redirect_collection() {
    let first = repo();
    let second = repo();
    let state = tempfile::tempdir().unwrap();
    let binary = env!("CARGO_BIN_EXE_herdr-infobox");
    let output = Command::new(binary)
        .arg("--state-dir")
        .arg(state.path())
        .args([
            "session",
            "add",
            "--provider",
            "claude",
            "--native-id",
            "isolation",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = Command::new(binary)
        .arg("--state-dir")
        .arg(state.path())
        .args(["repo", "add", "--session", "isolation", "--path"])
        .arg(first.path())
        .env("GIT_DIR", second.path().join(".git"))
        .env("GIT_WORK_TREE", second.path())
        .env("GIT_COMMON_DIR", second.path().join(".git"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "core.bare")
        .env("GIT_CONFIG_VALUE_0", "true")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let paths = Paths::open(Some(state.path().into())).unwrap();
    let store = herdr_infobox::store::Store::open(&paths).unwrap();
    let session = store.resolve("isolation").unwrap();
    let view = store.view(&session).unwrap();
    assert_eq!(view.worktrees.len(), 1);
    assert_eq!(view.worktrees[0].root, first.path().canonicalize().unwrap());
}

#[test]
fn untracked_executable_has_executable_mode() {
    use std::os::unix::fs::PermissionsExt;
    let dir = repo();
    let path = dir.path().join("script");
    fs::write(&path, "#!/bin/sh\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    let diff = git::capture(
        dir.path(),
        Scope::Untracked,
        Some(Path::new("script")),
        None,
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&diff.patch).contains("new file mode 100755"));
}

#[test]
fn nested_repository_discovery_uses_nearest_root_for_existing_and_missing_files() {
    let outer = repo();
    let inner = outer.path().join("nested");
    fs::create_dir(&inner).unwrap();
    git(&inner, &["init", "-q"]);
    fs::write(inner.join("file"), "nested\n").unwrap();
    let enclosing = git::discover(outer.path(), outer.path()).unwrap();
    for path in ["nested/file", "nested/not-created/yet"] {
        let discovered = git::discover(Path::new(path), outer.path()).unwrap();
        assert_eq!(discovered.root, inner.canonicalize().unwrap());
        assert_ne!(discovered.common_dir, enclosing.common_dir);
        assert_ne!(discovered.git_dir, enclosing.git_dir);
    }
}

#[test]
fn absorbed_submodule_discovery_keeps_its_repository_identity() {
    let source = repo();
    fs::write(source.path().join("file"), "original\n").unwrap();
    commit(source.path());
    let outer = repo();
    git(
        outer.path(),
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--",
            source.path().to_str().unwrap(),
            "modules/child",
        ],
    );
    commit(outer.path());
    let child = outer.path().join("modules/child");
    assert!(child.join(".git").is_file());
    let enclosing = git::discover(outer.path(), outer.path()).unwrap();
    let discovered = git::discover(Path::new("modules/child/file"), outer.path()).unwrap();
    assert_eq!(discovered.root, child.canonicalize().unwrap());
    assert_eq!(discovered.git_dir, discovered.common_dir);
    assert!(
        discovered
            .git_dir
            .starts_with(enclosing.git_dir.join("modules"))
    );
    assert_ne!(discovered.common_dir, enclosing.common_dir);
    fs::write(child.join("file"), "modified child\n").unwrap();
    let diff = git::capture(
        &discovered.root,
        Scope::Unstaged,
        Some(Path::new("file")),
        None,
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&diff.patch).contains("+modified child"));
}

#[test]
fn relocated_linked_worktree_rediscovers_new_root_and_same_git_directories() {
    let main = repo();
    fs::write(main.path().join("file"), "original\n").unwrap();
    commit(main.path());
    let location = tempfile::tempdir().unwrap();
    let old = location.path().join("old");
    let new = location.path().join("new");
    git(
        main.path(),
        &["worktree", "add", "-qb", "relocated", old.to_str().unwrap()],
    );
    let before = git::discover(&old, main.path()).unwrap();
    git(
        main.path(),
        &[
            "worktree",
            "move",
            old.to_str().unwrap(),
            new.to_str().unwrap(),
        ],
    );
    assert!(!old.exists());
    let after = git::discover(&new.join("missing/file"), main.path()).unwrap();
    assert_eq!(after.root, new.canonicalize().unwrap());
    assert_ne!(after.root, before.root);
    assert_eq!(after.git_dir, before.git_dir);
    assert_eq!(after.common_dir, before.common_dir);
    fs::write(new.join("file"), "moved worktree\n").unwrap();
    let diff = git::capture(&after.root, Scope::Unstaged, Some(Path::new("file")), None).unwrap();
    assert!(String::from_utf8_lossy(&diff.patch).contains("+moved worktree"));
}
