use crate::{Result, model::Discovery};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::os::unix::{
    ffi::{OsStrExt, OsStringExt},
    fs::MetadataExt,
    process::CommandExt,
};
use std::{
    ffi::{OsStr, OsString},
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub const PATCH_LIMIT: usize = 2 * 1024 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Unstaged,
    Staged,
    Untracked,
    Branch,
    Conflicts,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub index: char,
    pub worktree: char,
    pub kind: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub changes: Vec<Change>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedDiff {
    pub root: PathBuf,
    pub scope: Scope,
    pub path: Option<PathBuf>,
    pub patch: Vec<u8>,
    pub head: Option<String>,
    pub base: Option<String>,
    pub merge_base: Option<String>,
    pub captured_at: i64,
    pub warnings: Vec<String>,
}

pub(crate) fn bounded(
    command: &mut Command,
    limit: usize,
    timeout: Duration,
) -> Result<(bool, Vec<u8>, Vec<u8>)> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn()?;
    let (tx, rx) = mpsc::channel();
    let out = child.stdout.take().ok_or("missing stdout")?;
    let err = child.stderr.take().ok_or("missing stderr")?;
    for (stream, mut reader) in [
        (0, Box::new(out) as Box<dyn Read + Send>),
        (1, Box::new(err) as Box<dyn Read + Send>),
    ] {
        let tx = tx.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = reader
                .by_ref()
                .take((limit + 1) as u64)
                .read_to_end(&mut bytes);
            let _ = tx.send((stream, result.map(|_| bytes)));
        });
    }
    drop(tx);
    let start = Instant::now();
    let mut outputs = [None, None];
    let mut status = None;
    loop {
        while let Ok((stream, result)) = rx.try_recv() {
            outputs[stream] = Some(result?);
        }
        let exceeded = outputs.iter().flatten().any(|v| v.len() > limit);
        if exceeded || start.elapsed() > timeout {
            #[cfg(unix)]
            {
                let _ = Command::new("/bin/kill")
                    .args(["-KILL", "--", &format!("-{}", child.id())])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(if exceeded {
                "command output limit exceeded"
            } else {
                "command timed out"
            }
            .into());
        }
        if status.is_none() {
            status = child.try_wait()?;
        }
        if let Some(status) = status
            && outputs.iter().all(Option::is_some)
        {
            return Ok((
                status.success(),
                outputs[0].take().unwrap_or_default(),
                outputs[1].take().unwrap_or_default(),
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
}
fn command(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
        "GIT_SUPER_PREFIX",
        "GIT_CONFIG",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_PARAMETERS",
        "GIT_GRAFT_FILE",
        "GIT_SHALLOW_FILE",
        "GIT_NAMESPACE",
        "GIT_REPLACE_REF_BASE",
        "GIT_IMPLICIT_WORK_TREE",
        "GIT_CEILING_DIRECTORIES",
    ] {
        cmd.env_remove(key);
    }
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("GIT_CONFIG_KEY_") || name.starts_with("GIT_CONFIG_VALUE_") {
            cmd.env_remove(key);
        }
    }

    cmd.args([
        "--no-pager",
        "--literal-pathspecs",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "color.ui=false",
        "-C",
    ])
    .arg(root)
    .env("GIT_OPTIONAL_LOCKS", "0")
    .env("GIT_TERMINAL_PROMPT", "0");
    cmd
}
fn run(root: &Path, args: &[&OsStr], limit: usize) -> Result<Vec<u8>> {
    let (ok, out, err) = bounded(command(root).args(args), limit, Duration::from_secs(5))?;
    if !ok {
        return Err(format!("Git failed: {}", String::from_utf8_lossy(&err).trim()).into());
    }
    Ok(out)
}
fn simple(root: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(run(
        root,
        &args.iter().map(OsStr::new).collect::<Vec<_>>(),
        PATCH_LIMIT,
    )?)?
    .trim_end_matches('\n')
    .to_owned())
}
fn path_output(root: &Path, args: &[&str]) -> Result<PathBuf> {
    let mut bytes = run(
        root,
        &args.iter().map(OsStr::new).collect::<Vec<_>>(),
        PATCH_LIMIT,
    )?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    let path = PathBuf::from(OsString::from_vec(bytes));
    Ok(fs::canonicalize(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })?)
}
pub fn discover(path: &Path, cwd: &Path) -> Result<Discovery> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let mut ancestor = joined.as_path();
    while !ancestor.is_dir() {
        ancestor = ancestor.parent().ok_or("no existing parent directory")?;
    }
    let root = path_output(ancestor, &["rev-parse", "--show-toplevel"])?;
    let git_dir = path_output(&root, &["rev-parse", "--absolute-git-dir"])?;
    let common_dir = path_output(&root, &["rev-parse", "--git-common-dir"])?;
    let branch = simple(&root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok();
    let remotes = simple(&root, &["remote"])?;
    let names: Vec<_> = remotes.lines().collect();
    let upstream = branch
        .as_ref()
        .and_then(|b| simple(&root, &["config", "--get", &format!("branch.{b}.remote")]).ok())
        .filter(|r| r != ".");
    let configured = simple(&root, &["config", "--get", "infobox.remote"])
        .ok()
        .filter(|r| !r.is_empty());
    let remote = configured
        .or(upstream)
        .or_else(|| names.contains(&"origin").then(|| "origin".into()))
        .or_else(|| (names.len() == 1).then(|| names[0].into()));
    let mappings = simple(&root, &["config", "--get-all", "infobox.github-host"])
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(alias, host)| (alias.to_owned(), host.to_owned()))
        .collect();
    let github_url = remote
        .and_then(|r| simple(&root, &["remote", "get-url", &r]).ok())
        .and_then(|u| github_url_with_hosts(&u, &mappings));
    Ok(Discovery {
        root,
        git_dir,
        common_dir,
        github_url,
        branch,
    })
}
pub fn github_url(remote: &str) -> Option<String> {
    github_url_with_hosts(remote, &std::collections::BTreeMap::new())
}
pub fn github_url_with_hosts(
    remote: &str,
    mappings: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    let parsed = if remote.contains("://") {
        url::Url::parse(remote).ok()?
    } else {
        let (host, path) = remote.split_once(':')?;
        url::Url::parse(&format!("ssh://{host}/{path}")).ok()?
    };
    if !matches!(parsed.scheme(), "ssh" | "git" | "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?;
    let target = if host == "github.com" {
        "github.com"
    } else {
        mappings.get(host)?.as_str()
    };
    let mut url = url::Url::parse(&format!("https://{target}")).ok()?;
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let path = parsed.path().trim_start_matches('/').trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let components: Vec<_> = path.split('/').collect();
    if components.len() != 2
        || components
            .iter()
            .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return None;
    }
    url.set_path(path);
    Some(url.into())
}
pub fn status(root: &Path) -> Result<Status> {
    parse_status(&run(
        root,
        &[
            OsStr::new("status"),
            OsStr::new("--porcelain=v2"),
            OsStr::new("-z"),
            OsStr::new("--untracked-files=normal"),
        ],
        PATCH_LIMIT,
    )?)
}
pub fn parse_status(bytes: &[u8]) -> Result<Status> {
    let mut records = bytes.split(|b| *b == 0);
    let mut changes = Vec::new();
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let fields = match record[0] {
            b'1' => 9,
            b'2' => 10,
            b'u' => 11,
            b'?' => 2,
            b'!' => continue,
            _ => return Err("unknown Git status record".into()),
        };
        let pieces: Vec<_> = record.splitn(fields, |b| *b == b' ').collect();
        if pieces.len() != fields {
            return Err("incomplete Git status record".into());
        }
        let path = PathBuf::from(OsString::from_vec(pieces[fields - 1].to_vec()));
        let (index, worktree) = if record[0] == b'?' {
            ('?', '?')
        } else {
            let xy = pieces[1];
            if xy.len() != 2 {
                return Err("invalid Git XY status".into());
            }
            (xy[0] as char, xy[1] as char)
        };
        let old_path = if record[0] == b'2' {
            Some(PathBuf::from(OsString::from_vec(
                records.next().ok_or("missing rename source")?.to_vec(),
            )))
        } else {
            None
        };
        changes.push(Change {
            path,
            old_path,
            index,
            worktree,
            kind: match record[0] {
                b'?' => "untracked",
                b'u' => "conflict",
                b'2' => "rename",
                _ if pieces.get(2).is_some_and(|s| s.starts_with(b"S")) => "submodule",
                _ if pieces
                    .get(3..6)
                    .is_some_and(|modes| modes.contains(&b"120000".as_slice())) =>
                {
                    "symlink"
                }
                _ if pieces
                    .get(3..6)
                    .is_some_and(|modes| modes[0] != modes[1] || modes[1] != modes[2]) =>
                {
                    "mode"
                }
                _ => "tracked",
            }
            .into(),
        });
    }
    Ok(Status { changes })
}
fn fingerprint(root: &Path, path: Option<&Path>) -> Result<Vec<u8>> {
    let status_bytes = run(
        root,
        &[
            OsStr::new("status"),
            OsStr::new("--porcelain=v2"),
            OsStr::new("-z"),
            OsStr::new("--untracked-files=normal"),
        ],
        PATCH_LIMIT,
    )?;
    let mut stamp = status_bytes.clone();
    stamp.extend(
        simple(root, &["rev-parse", "--verify", "HEAD"])
            .unwrap_or_default()
            .as_bytes(),
    );
    let index = path_output(root, &["rev-parse", "--absolute-git-dir"])?.join("index");
    let paths = if let Some(path) = path {
        vec![root.join(path), index]
    } else {
        let mut paths: Vec<_> = parse_status(&status_bytes)?
            .changes
            .into_iter()
            .map(|c| root.join(c.path))
            .collect();
        paths.push(index);
        paths
    };
    for path in paths {
        if let Ok(meta) = fs::symlink_metadata(path) {
            stamp.extend(
                format!(
                    "{}:{}:{}:{}:{}",
                    meta.ino(),
                    meta.len(),
                    meta.mtime(),
                    meta.mtime_nsec(),
                    meta.ctime_nsec()
                )
                .as_bytes(),
            );
        }
    }
    Ok(stamp)
}
pub fn capture(
    root: &Path,
    scope: Scope,
    path: Option<&Path>,
    base: Option<&str>,
) -> Result<CapturedDiff> {
    if let Some(path) = path
        && (path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir)))
    {
        return Err("diff path must be relative to worktree".into());
    }
    for attempt in 0..2 {
        let before = fingerprint(root, path)?;
        let head = simple(root, &["rev-parse", "--verify", "HEAD"]).ok();
        let mut args: Vec<OsString> = ["diff", "--no-color", "--no-ext-diff", "--no-textconv"]
            .iter()
            .map(OsString::from)
            .collect();
        let mut resolved_base = None;
        let mut merge_base = None;
        let mut warnings = Vec::new();
        match scope {
            Scope::Staged => {
                args.push("--cached".into());
                if head.is_none() {
                    warnings.push("Unborn HEAD".into());
                }
            }
            Scope::Branch => {
                let base = base.ok_or("branch diff requires an explicit base")?;
                if base.starts_with('-') {
                    return Err("invalid base reference".into());
                }
                let base_sha = simple(
                    root,
                    &[
                        "rev-parse",
                        "--verify",
                        "--end-of-options",
                        &format!("{base}^{{commit}}"),
                    ],
                )?;
                let head_sha = head.as_deref().ok_or("branch diff requires HEAD")?;
                let merge = simple(root, &["merge-base", "--all", &base_sha, head_sha])?;
                if merge.lines().count() != 1 {
                    return Err("ambiguous merge-base".into());
                }
                merge_base = Some(merge.clone());
                args.push(merge.into());
                args.push(head_sha.into());
                resolved_base = Some(base_sha);
            }
            Scope::Conflicts => {
                args.push("--cc".into());
                warnings
                    .push("Unmerged conflict. Combined diff is not a two-way comparison.".into());
            }
            Scope::Unstaged | Scope::Untracked => {}
        }
        args.push("--".into());
        if let Some(path) = path {
            args.push(path.as_os_str().into());
        }
        let patch = if scope == Scope::Untracked {
            untracked(root, path.ok_or("select one untracked file")?)?
        } else {
            run(
                root,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                PATCH_LIMIT,
            )?
        };
        if patch.windows(13).any(|s| s == b"Binary files ") {
            warnings.push("Binary content is not rendered as text".into());
        }
        let after = fingerprint(root, path)?;
        if before != after {
            if attempt == 0 {
                continue;
            }
            warnings.push("Changed while reading".into());
        }
        return Ok(CapturedDiff {
            root: root.to_owned(),
            scope,
            path: path.map(Path::to_owned),
            patch,
            head,
            base: resolved_base,
            merge_base,
            captured_at: crate::model::now(),
            warnings,
        });
    }
    unreachable!()
}
fn untracked(root: &Path, path: &Path) -> Result<Vec<u8>> {
    let status = status(root)?;
    if !status
        .changes
        .iter()
        .any(|c| c.kind == "untracked" && (c.path == path || path.starts_with(&c.path)))
    {
        return Err("path is not untracked".into());
    }
    let full = root.join(path);
    let meta = fs::symlink_metadata(&full)?;
    if meta.is_dir() {
        return Err("select an untracked file inside this directory".into());
    }
    let content = if meta.file_type().is_symlink() {
        fs::read_link(&full)?.as_os_str().as_bytes().to_vec()
    } else if meta.is_file() {
        let canonical = fs::canonicalize(&full)?;
        if !canonical.starts_with(fs::canonicalize(root)?) {
            return Err("untracked path escapes worktree".into());
        }
        let mut bytes = Vec::new();
        fs::File::open(full)?
            .take((PATCH_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)?;
        bytes
    } else {
        return Err("untracked special file is not readable".into());
    };
    if content.len() > PATCH_LIMIT {
        return Err("untracked file exceeds patch limit".into());
    }
    let label = display_path(path);
    if content.contains(&0) {
        return Ok(
            format!("Binary untracked file {label} ({} bytes)\n", content.len()).into_bytes(),
        );
    }
    let lines = content.split_inclusive(|b| *b == b'\n').count();
    let mut patch=format!("diff --git a/{label} b/{label}\nnew file mode {}\n--- /dev/null\n+++ b/{label}\n@@ -0,0 +1,{lines} @@\n",if meta.file_type().is_symlink(){"120000"}else if meta.mode()&0o111!=0{"100755"}else{"100644"}).into_bytes();
    for line in content.split_inclusive(|b| *b == b'\n') {
        patch.push(b'+');
        patch.extend(line);
    }
    if !content.is_empty() && !content.ends_with(b"\n") {
        patch.extend(b"\n\\ No newline at end of file\n");
    }
    if patch.len() > PATCH_LIMIT {
        return Err("untracked patch exceeds limit".into());
    }
    Ok(patch)
}
pub fn display_path(path: &Path) -> String {
    let mut out = String::new();
    for b in path.as_os_str().as_bytes() {
        if (32..127).contains(b) && *b != b'\\' {
            out.push(*b as char);
        } else {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subprocess_timeout_and_output_limits_are_enforced() {
        let started = Instant::now();
        let error = bounded(
            Command::new("/bin/sh").args(["-c", "sleep 5"]),
            1024,
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(error.to_string().contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(2));
        let error = bounded(
            Command::new("/bin/sh").args(["-c", "while :; do printf 0123456789; done"]),
            128,
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(error.to_string().contains("output limit"));
    }
}
