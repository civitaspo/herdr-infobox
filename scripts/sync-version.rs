use std::{env, fs, path::PathBuf, process};

fn valid_version(version: &str) -> bool {
    let (core, prerelease) = version
        .split_once('-')
        .map_or((version, None), |(core, pre)| (core, Some(pre)));
    let numeric = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|byte| byte.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    core.split('.').count() == 3
        && core.split('.').all(numeric)
        && prerelease.is_none_or(|pre| {
            pre.split('.').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                    && (!part.bytes().all(|byte| byte.is_ascii_digit()) || numeric(part))
            })
        })
}

fn replace_version(text: &str, filename: &str, version: &str) -> Result<String, String> {
    let mut sections = vec![0];
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with('[') && offset != 0 {
            sections.push(offset);
        }
        offset += line.len();
    }
    sections.push(text.len());
    let mut replacements = Vec::new();
    for bounds in sections.windows(2) {
        let section = &text[bounds[0]..bounds[1]];
        let selected = match filename {
            "Cargo.toml" => section.lines().next() == Some("[package]"),
            "Cargo.lock" => {
                section.lines().next() == Some("[[package]]")
                    && section
                        .lines()
                        .any(|line| line == "name = \"herdr-infobox\"")
            }
            "herdr-plugin.toml" => bounds[0] == 0 && !section.starts_with('['),
            _ => unreachable!(),
        };
        if !selected {
            continue;
        }
        let mut offset = bounds[0];
        for line in section.split_inclusive('\n') {
            if let Some(value) = line
                .strip_prefix("version")
                .and_then(|tail| tail.trim_start().strip_prefix('=').map(str::trim_start))
            {
                let quoted = value
                    .strip_prefix('"')
                    .and_then(|value| value.find('"').map(|end| &value[..end]));
                let Some(quoted) = quoted.filter(|quoted| !quoted.is_empty()) else {
                    return Err(format!("Invalid package version in {filename}"));
                };
                let start = offset + line.len() - value.len() + 1;
                replacements.push(start..start + quoted.len());
            }
            offset += line.len();
        }
    }
    if replacements.len() != 1 {
        return Err(format!(
            "Expected exactly one package version in {filename}, found {}",
            replacements.len()
        ));
    }
    let mut result = text.to_owned();
    result.replace_range(replacements.remove(0), version);
    Ok(result)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut root = env::current_dir()?;
    let mut check = false;
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--check") => check = true,
            Some("--root") => root = PathBuf::from(args.next().ok_or("--root requires a path")?),
            _ => return Err("Usage: sync-version [--root PATH] [--check]".into()),
        }
    }
    let version = fs::read_to_string(root.join(".release-version"))?;
    let version = version.trim();
    if !valid_version(version) {
        return Err("Invalid release version".into());
    }
    let mut changes = Vec::new();
    for filename in ["Cargo.toml", "Cargo.lock", "herdr-plugin.toml"] {
        let old = fs::read_to_string(root.join(filename))?;
        let new = replace_version(&old, filename, version)?;
        if old != new {
            changes.push((filename, new));
        }
    }
    if check && !changes.is_empty() {
        return Err(format!(
            "Version mismatch: {}",
            changes
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>()
                .join(", ")
        )
        .into());
    }
    for (filename, text) in changes {
        fs::write(root.join(filename), text)?;
    }
    println!("Package, lockfile, and Herdr manifest match {version}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(1);
    }
}
