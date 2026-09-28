use clap::{Parser, Subcommand};
use herdr_infobox::{Result, config::Paths, ingest, model::*, store::Store};
use std::path::PathBuf;
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(long, global = true)]
    state_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
    Ref {
        #[command(subcommand)]
        command: RefCommand,
    },
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    Ui {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        once: bool,
    },
    Ingest {
        #[arg(long)]
        provider: Provider,
    },
    Reconcile {
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    #[command(name = "opencode")]
    OpenCode {
        #[command(subcommand)]
        command: OpenCodeCommand,
    },
    Toggle,
    Ensure,
    #[command(hide = true)]
    AnnotateView,
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
    Adapters {
        #[command(subcommand)]
        command: AdapterCommand,
    },
    Doctor {
        #[arg(long)]
        json: bool,
    },
    Diff {
        #[arg(long)]
        session: String,
        #[arg(long)]
        worktree: Option<String>,
        #[arg(long, value_enum, default_value = "unstaged")]
        scope: herdr_infobox::git::Scope,
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        base: Option<String>,
    },
}
#[derive(Subcommand)]
enum SessionCommand {
    Add {
        #[arg(long)]
        provider: Provider,
        #[arg(long)]
        native_id: String,
        #[arg(long, default_value = "main")]
        scope: String,
    },
    List,
}
#[derive(Subcommand)]
enum RepoCommand {
    Add {
        #[arg(long)]
        session: String,
        #[arg(long)]
        path: PathBuf,
    },
}
#[derive(Subcommand)]
enum RefCommand {
    Add {
        #[arg(long)]
        session: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        title: Option<String>,
    },
}
#[derive(Subcommand)]
enum PlanCommand {
    Attach {
        #[arg(long)]
        session: String,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    Select {
        #[arg(long)]
        session: String,
        #[arg(long)]
        revision: String,
        #[arg(long, value_enum, default_value = "executing")]
        state: ExecutionState,
    },
}
#[derive(Subcommand)]
enum SnapshotCommand {
    List,
    Purge {
        #[arg(long)]
        hash: String,
    },
    Export {
        #[arg(long)]
        session: String,
        #[arg(long)]
        worktree: Option<String>,
        #[arg(long, value_enum, default_value = "unstaged")]
        scope: herdr_infobox::git::Scope,
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long)]
        base: Option<String>,
    },
}
#[derive(Subcommand)]
enum OpenCodeCommand {
    Connect {
        #[arg(long)]
        session: String,
        #[arg(long)]
        server: String,
        #[arg(long, default_value = "opencode")]
        binary: PathBuf,
    },
    Disconnect {
        #[arg(long)]
        session: String,
    },
}
#[derive(Subcommand)]
enum AdapterCommand {
    Install {
        provider: Provider,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
    Uninstall {
        provider: Provider,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
    },
}
fn main() {
    let cli = Cli::parse();
    let hook = matches!(cli.command, Command::Ingest { .. });
    if let Err(error) = run(cli)
        && !hook
    {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
fn run(cli: Cli) -> Result<()> {
    let paths = Paths::open(cli.state_dir)?;
    if let Command::Ingest { provider } = cli.command {
        let result = (|| {
            let input = ingest::read_stdin()?;
            let context = herdr_infobox::providers::IngressContext {
                host_id: paths.host_id.clone(),
                ingress_id: uuid::Uuid::new_v4().to_string(),
                devin_project_dir: std::env::var_os("DEVIN_PROJECT_DIR").map(PathBuf::from),
            };
            let batch = herdr_infobox::providers::decode(provider, &input, &context)?;
            ingest::persist_or_spool(&paths, &batch)
        })();
        if result.is_err()
            && let Ok(store) = Store::open_existing(&paths)
        {
            let _ = store.diagnostic(
                "collection_failure",
                "Collector input, schema, enrollment, or persistence unavailable",
            );
        }
        return result;
    }
    if matches!(cli.command, Command::AnnotateView) {
        return herdr_infobox::annotate::view_from_env();
    }
    let mut store = Store::open(&paths)?;
    match cli.command {
        Command::Session {
            command:
                SessionCommand::Add {
                    provider,
                    native_id,
                    scope,
                },
        } => {
            let key = SessionKey {
                host_id: paths.host_id.clone(),
                provider,
                native_session_id: native_id,
                agent_scope: scope,
            };
            println!("{}", store.register(&key)?);
        }
        Command::Session {
            command: SessionCommand::List,
        } => {
            for s in store.sessions()? {
                println!(
                    "{}  {}  {} [{}]{}",
                    s.id,
                    s.key.provider.name(),
                    s.key.native_session_id,
                    s.key.agent_scope,
                    if s.ended { " Ended" } else { "" }
                );
            }
        }
        Command::Repo {
            command: RepoCommand::Add { session, path },
        } => {
            let s = store.resolve(&session)?;
            let found = herdr_infobox::git::discover(&path, &std::env::current_dir()?)?;
            println!("{}", store.add_worktree(&s.id, &found, "manual")?);
        }
        Command::Ref {
            command:
                RefCommand::Add {
                    session,
                    url,
                    title,
                },
        } => {
            let s = store.resolve(&session)?;
            store.commit_batch(&ingest::manual(
                s.key,
                vec![Observation::Reference {
                    url,
                    title,
                    title_source: Some("manual".into()),
                    relation: "manual".into(),
                }],
            ))?;
        }
        Command::Plan {
            command:
                PlanCommand::Attach {
                    session,
                    file,
                    name,
                },
        } => {
            let s = store.resolve(&session)?;
            let file = std::fs::canonicalize(file)?;
            let markdown = read_plan(&file)?;
            let plan_key = name.unwrap_or_else(|| file.to_string_lossy().into_owned());
            let hash = content_hash(markdown.as_bytes());
            store.commit_batch(&ingest::manual(
                s.key,
                vec![Observation::Plan {
                    plan_key,
                    markdown,
                    source_path: Some(file),
                    phase: DocumentPhase::Draft,
                }],
            ))?;
            println!("{hash}");
        }
        Command::Plan {
            command:
                PlanCommand::Select {
                    session,
                    revision,
                    state,
                },
        } => {
            let s = store.resolve(&session)?;
            let view = store.view(&s)?;
            let matches: Vec<_> = view
                .plans
                .iter()
                .filter(|p| p.id.starts_with(&revision) || p.hash.starts_with(&revision))
                .collect();
            if matches.len() != 1 {
                return Err("Revision selector must match exactly one revision".into());
            }
            let p = matches[0];
            store.commit_batch(&ingest::manual(
                s.key,
                vec![Observation::Execution {
                    plan_key: p.plan_key.clone(),
                    revision_hash: p.hash.clone(),
                    state,
                    evidence: "manual".into(),
                }],
            ))?;
        }
        Command::Ui { session, once } => {
            herdr_infobox::ui::run(&paths, &mut store, session.as_deref(), once)?
        }
        Command::OpenCode { command } => match command {
            OpenCodeCommand::Connect {
                session,
                server,
                binary,
            } => {
                let session = store.resolve(&session)?;
                let count = herdr_infobox::opencode_sync::connect(
                    &paths, &mut store, &session, server, binary,
                )?;
                println!("Connected OpenCode V2; reconciled {count} records");
            }
            OpenCodeCommand::Disconnect { session } => {
                let session = store.resolve(&session)?;
                herdr_infobox::opencode_sync::disconnect(&paths, &session)?;
                println!("Disconnected OpenCode V2; cached history preserved");
            }
        },
        Command::Reconcile { session, file } => {
            let mut sync_error = None;
            let selected = session.as_deref().map(|id| store.resolve(id)).transpose()?;
            for entry in store.sessions()? {
                if entry.key.provider == Provider::OpenCode
                    && selected.as_ref().is_none_or(|s| s.id == entry.id)
                    && let Err(error) =
                        herdr_infobox::opencode_sync::reconcile(&paths, &mut store, &entry)
                {
                    sync_error = Some(error);
                }
            }

            if let Some(file) = file {
                let session = store.resolve(
                    session
                        .as_deref()
                        .ok_or("--session is required with --file")?,
                )?;
                if session.key.provider != Provider::Codex {
                    return Err(
                        "Only the verified Codex legacy transcript parser is available".into(),
                    );
                }
                let file = std::fs::canonicalize(file)?;
                let cursor = store.cursor(
                    &session.id,
                    &file,
                    herdr_infobox::providers::CODEX_TRANSCRIPT_PARSER,
                )?;
                let chunk =
                    herdr_infobox::providers::read_codex_transcript(&file, &session, cursor)?;
                store.commit_import(&chunk)?;
            }
            let count = ingest::drain(&paths, &mut store)?;
            for pending in store.pending_paths()? {
                let result = herdr_infobox::git::discover(&pending.path, &pending.cwd)
                    .map_err(|e| e.to_string());
                store.finish_path(&pending, &result)?;
            }
            println!("Reconciled {count} spool batches");
            if let Some(error) = sync_error {
                return Err(error);
            }
        }
        Command::Doctor { json } => {
            let report = herdr_infobox::doctor::report(&paths, &store)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!(
                    "herdr-infobox {}\nDatabase ready. {} registered sessions.\nProvider runtime verification pending. Annotation delivery is copy only.",
                    env!("CARGO_PKG_VERSION"),
                    store.sessions()?.len()
                );
                for (name, reason, count) in store.diagnostics()? {
                    println!("{name}: {reason} ({count})");
                }
                for status in herdr_infobox::adapters::status(&paths)? {
                    println!("{status}");
                }
            }
        }
        Command::Diff {
            session,
            worktree,
            scope,
            path,
            base,
        } => {
            let s = store.resolve(&session)?;
            let view = store.view(&s)?;
            let w = select_worktree(&view, worktree.as_deref())?;
            let diff =
                herdr_infobox::git::capture(&w.root, scope, path.as_deref(), base.as_deref())?;
            println!("{}", safe_text(&String::from_utf8_lossy(&diff.patch)));
            for warning in diff.warnings {
                eprintln!("{warning}");
            }
        }
        Command::Toggle | Command::Ensure => {
            let herdr = herdr_infobox::herdr::Herdr::from_env()?;
            herdr_infobox::herdr::ensure(&paths, &herdr, matches!(cli.command, Command::Toggle))?;
        }
        Command::Snapshot {
            command: SnapshotCommand::List,
        } => {
            for entry in std::fs::read_dir(&paths.snapshots)? {
                let path = entry?.path();
                if matches!(
                    path.extension().and_then(|x| x.to_str()),
                    Some("md" | "patch")
                ) {
                    println!("{}", path.display());
                }
            }
        }
        Command::Snapshot {
            command: SnapshotCommand::Purge { hash },
        } => {
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("Supply the complete snapshot SHA-256".into());
            }
            let mut removed = false;
            for extension in ["md", "patch"] {
                let path = paths.snapshots.join(format!("{hash}.{extension}"));
                match std::fs::remove_file(path) {
                    Ok(()) => removed = true,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            if !removed {
                return Err("Snapshot hash not found".into());
            }
            println!("Snapshot removed. Reviewer annotations were not changed.");
        }
        Command::Snapshot {
            command:
                SnapshotCommand::Export {
                    session,
                    worktree,
                    scope,
                    path,
                    base,
                },
        } => {
            let session = store.resolve(&session)?;
            let view = store.view(&session)?;
            let worktree = select_worktree(&view, worktree.as_deref())?;
            let diff = herdr_infobox::git::capture(
                &worktree.root,
                scope,
                path.as_deref(),
                base.as_deref(),
            )?;
            let snapshot = herdr_infobox::annotate::export(&paths, &session, worktree, &diff)?;
            println!("{}", snapshot.path.display());
        }
        Command::Adapters { command } => {
            let (provider, config, dry_run, uninstall) = match command {
                AdapterCommand::Install {
                    provider,
                    config,
                    dry_run,
                } => (provider, config, dry_run, false),
                AdapterCommand::Uninstall {
                    provider,
                    config,
                    dry_run,
                } => (provider, config, dry_run, true),
            };
            println!(
                "{}",
                herdr_infobox::adapters::configure(
                    &paths,
                    provider,
                    config.as_deref(),
                    dry_run,
                    uninstall
                )?
            );
        }
        Command::Ingest { .. } | Command::AnnotateView => unreachable!(),
    }
    Ok(())
}
fn select_worktree<'a>(view: &'a SessionView, selector: Option<&str>) -> Result<&'a Worktree> {
    match selector {
        Some(id) => view
            .worktrees
            .iter()
            .find(|w| w.id.starts_with(id))
            .ok_or_else(|| "Worktree not found".into()),
        None if view.worktrees.len() == 1 => Ok(&view.worktrees[0]),
        _ => Err("Select --worktree from the session's repository list".into()),
    }
}

fn read_plan(path: &std::path::Path) -> Result<String> {
    use std::io::Read;
    let mut data = Vec::new();
    std::fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut data)?;
    if data.len() > 1024 * 1024 {
        return Err("Plan exceeds 1 MiB".into());
    }
    Ok(String::from_utf8(data)?)
}
