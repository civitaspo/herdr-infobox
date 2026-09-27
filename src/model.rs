use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Claude,
    #[value(name = "opencode", alias = "open-code")]
    #[serde(rename = "opencode", alias = "open-code")]
    OpenCode,
    Codex,
    Cursor,
    Devin,
}
impl Provider {
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Devin => "devin",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionKey {
    pub host_id: String,
    pub provider: Provider,
    pub native_session_id: String,
    pub agent_scope: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    pub schema_version: u32,
    pub session: SessionKey,
    pub source: String,
    pub source_event_id: String,
    pub native_call_key: Option<String>,
    pub source_order: Option<u64>,
    pub events: Vec<Observation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Observation {
    Session {
        parent_native_id: Option<String>,
        ended: bool,
    },
    Path {
        path: PathBuf,
        cwd: PathBuf,
        relation: String,
    },
    Reference {
        url: String,
        title: Option<String>,
        title_source: Option<String>,
        relation: String,
    },
    FetchFailed {
        url: String,
        reason: String,
    },
    Plan {
        plan_key: String,
        markdown: String,
        source_path: Option<PathBuf>,
        phase: DocumentPhase,
    },
    Execution {
        plan_key: String,
        revision_hash: String,
        state: ExecutionState,
        evidence: String,
    },
    Tasks {
        namespace: String,
        replace: bool,
        tasks: Vec<Task>,
    },
    Capability {
        feature: String,
        state: String,
        reason: String,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum DocumentPhase {
    Draft,
    Proposed,
    Approved,
    Superseded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Unknown,
    Executing,
    Completed,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub text: String,
    pub status: String,
    pub parent_id: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub key: SessionKey,
    pub ended: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub id: String,
    pub repository_id: String,
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
    pub github_url: Option<String>,
    pub branch: Option<String>,
    pub relations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Discovery {
    pub root: PathBuf,
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
    pub github_url: Option<String>,
    pub branch: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub url: String,
    pub title: Option<String>,
    pub relations: Vec<String>,
    pub sources: Vec<String>,
    pub failure: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRevision {
    pub id: String,
    pub plan_key: String,
    pub hash: String,
    pub markdown: String,
    pub source_path: Option<PathBuf>,
    pub phase: DocumentPhase,
    pub execution: Option<ExecutionState>,
    pub selected: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub feature: String,
    pub state: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionView {
    pub session: SessionSummary,
    pub worktrees: Vec<Worktree>,
    pub references: Vec<Reference>,
    pub plans: Vec<PlanRevision>,
    pub tasks: Vec<Task>,
    pub capabilities: Vec<Capability>,
}
#[derive(Debug, Clone)]
pub struct PendingPath {
    pub event_id: i64,
    pub index: usize,
    pub session_id: String,
    pub path: PathBuf,
    pub cwd: PathBuf,
    pub relation: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCursor {
    pub session_id: String,
    pub source_path: PathBuf,
    pub file_identity: String,
    pub offset: u64,
    pub parser_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportChunk {
    pub batches: Vec<EventBatch>,
    pub cursor_before: Option<SourceCursor>,
    pub cursor_after: SourceCursor,
}
pub fn content_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
pub fn safe_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}
