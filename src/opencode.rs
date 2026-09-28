//! OpenCode 2.0.18 projected exports; no V1 bridge payloads or inferred approvals.
use crate::Result;
use crate::model::{DocumentPhase, EventBatch, Observation, Provider, SessionKey, content_hash};
use serde_json::Value;
use std::path::{Component, PathBuf};

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("OpenCode V2: missing {field}").into())
}

fn directory(value: &Value) -> Result<PathBuf> {
    let root = PathBuf::from(text(&value["location"], "directory")?);
    if !root.is_absolute() {
        return Err("OpenCode V2 location must be absolute".into());
    }
    if let Some(subpath) = value.get("subpath") {
        let subpath = PathBuf::from(subpath.as_str().ok_or("Invalid OpenCode subpath")?);
        if subpath
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err("OpenCode subpath must stay within its location".into());
        }
        return Ok(root.join(subpath));
    }
    Ok(root)
}

fn path(path: PathBuf, cwd: &std::path::Path, relation: &str) -> Observation {
    Observation::Path {
        path,
        cwd: cwd.to_owned(),
        relation: relation.into(),
    }
}

fn capability(feature: &str, reason: &str) -> Observation {
    Observation::Capability {
        feature: feature.into(),
        state: "partial".into(),
        reason: reason.into(),
    }
}

fn batch(
    session: &SessionKey,
    id: &str,
    value: &Value,
    events: Vec<Observation>,
) -> Result<EventBatch> {
    Ok(EventBatch {
        schema_version: 1,
        session: session.clone(),
        source: "opencode-v2".into(),
        source_event_id: format!("{id}:{}", content_hash(&serde_json::to_vec(value)?)),
        native_call_key: None,
        source_order: value["time"]["created"].as_u64(),
        events,
    })
}

/// Decode the raw `experimental.session.export` HTTP response, not CLI pretty output.
/// Every snapshot is validated before the caller persists any batch.
pub fn decode_export(input: &[u8], session: &SessionKey) -> Result<Vec<EventBatch>> {
    if session.provider != Provider::OpenCode {
        return Err("Expected an OpenCode session".into());
    }
    let value: Value = serde_json::from_slice(input)?;
    let data = value
        .get("data")
        .ok_or("Expected OpenCode V2 export response data")?;
    let info = &data["info"];
    if text(info, "id")? != session.native_session_id {
        return Err("OpenCode export belongs to another session".into());
    }
    text(info, "projectID")?;
    let current = directory(info)?;
    let messages = data["messages"]
        .as_array()
        .ok_or("OpenCode V2 export messages missing")?;
    let first_move = messages
        .iter()
        .find(|message| message["type"] == "location-switched");
    let mut cwd = match first_move {
        Some(message) => message.get("previous").map(directory).transpose()?,
        None => Some(current.clone()),
    };
    let mut events = vec![
        Observation::Session {
            parent_native_id: info["parentID"].as_str().map(str::to_owned),
            ended: false,
        },
        path(current.clone(), &current, "cwd"),
        capability(
            "runtime",
            "OpenCode 2.0.18 export schema is source-derived and fixture-tested; live delivery is unverified.",
        ),
        capability(
            "plan_execution",
            "Plan-agent text is a proposed document. Agent switches and session outcomes do not prove plan approval or execution.",
        ),
        capability(
            "web",
            "Completed webfetch supplies its requested URL but no page title. Search result schemas are not collected.",
        ),
        capability(
            "tasks",
            "No verified OpenCode V2 task-checklist schema is collected.",
        ),
    ];
    if let Some(cwd) = &cwd {
        events.push(path(cwd.clone(), cwd, "session-history"));
    } else {
        events.push(capability("repositories", "The first location change has no previous directory; earlier relative tool paths are unavailable."));
    }
    let mut batches = vec![batch(session, "session", info, events)?];
    for message in messages {
        let id = text(message, "id")?;
        let kind = text(message, "type")?;
        let mut events = Vec::new();
        match kind {
            "location-switched" => {
                if let Some(previous) = message.get("previous") {
                    let previous = directory(previous)?;
                    events.push(path(previous.clone(), &previous, "session-history"));
                }
                let next = directory(message)?;
                events.push(path(next.clone(), &next, "session-history"));
                cwd = Some(next);
            }
            "assistant" => {
                let agent = text(message, "agent")?;
                let content = message["content"]
                    .as_array()
                    .ok_or("OpenCode assistant content missing")?;
                let mut markdown = Vec::new();
                for item in content {
                    match text(item, "type")? {
                        "text" => {
                            markdown.push(item["text"].as_str().ok_or("Invalid assistant text")?);
                        }
                        "reasoning" => {}
                        "tool" => {
                            let mut tool_events = Vec::new();
                            tool(item, cwd.as_deref(), &mut tool_events)?;
                            if !tool_events.is_empty() {
                                let call = text(item, "id")?;
                                let mut record =
                                    batch(session, &format!("{id}:{call}"), item, tool_events)?;
                                record.native_call_key = Some(format!("{id}:{call}"));
                                batches.push(record);
                            }
                        }
                        _ => return Err("Unknown OpenCode assistant content schema".into()),
                    }
                }
                if agent == "plan"
                    && message["time"]["completed"].is_number()
                    && message.get("error").is_none()
                    && !markdown.join("\n\n").trim().is_empty()
                {
                    events.push(Observation::Plan {
                        plan_key: id.into(),
                        markdown: markdown.join("\n\n"),
                        source_path: None,
                        phase: DocumentPhase::Proposed,
                    });
                }
            }
            "agent-switched" | "model-switched" | "user" | "synthetic" | "system" | "skill"
            | "shell" | "compaction" => {}
            _ => return Err("Unknown OpenCode message schema".into()),
        }
        if !events.is_empty() {
            batches.push(batch(session, id, message, events)?);
        }
    }
    Ok(batches)
}

fn tool(item: &Value, cwd: Option<&std::path::Path>, events: &mut Vec<Observation>) -> Result<()> {
    text(item, "id")?;
    let name = text(item, "name")?;
    let state = &item["state"];
    let status = text(state, "status")?;
    if !matches!(status, "streaming" | "running" | "completed" | "error") {
        return Err("Unknown OpenCode tool state".into());
    }
    if status == "streaming" {
        return Ok(());
    }
    let input = state["input"]
        .as_object()
        .ok_or("OpenCode tool input must be an object")?;
    if name == "webfetch" {
        let url = input
            .get("url")
            .and_then(Value::as_str)
            .ok_or("webfetch URL missing")?;
        let parsed = url::Url::parse(url)?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err("Invalid webfetch URL".into());
        }
        events.push(Observation::Reference {
            url: url.into(),
            title: None,
            title_source: None,
            relation: if status == "completed" {
                "opened"
            } else {
                "requested"
            }
            .into(),
        });
        if status == "error" {
            events.push(Observation::FetchFailed {
                url: url.into(),
                reason: "OpenCode webfetch failed".into(),
            });
        }
    }
    if matches!(name, "read" | "write" | "edit") {
        let file = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or("OpenCode file tool path missing")?;
        let file = std::path::Path::new(file);
        if let Some(cwd) = cwd.or_else(|| file.is_absolute().then_some(file)) {
            events.push(path(file.to_owned(), cwd, "tool-input"));
        }
    }
    if matches!(name, "write" | "edit" | "patch")
        && status == "completed"
        && let Some(files) = state["metadata"].get("files")
    {
        for file in files.as_array().ok_or("Invalid OpenCode file metadata")? {
            let file = std::path::Path::new(text(file, "file")?);
            if let Some(cwd) = cwd.or_else(|| file.is_absolute().then_some(file)) {
                events.push(path(file.to_owned(), cwd, "tool-result"));
            }
        }
    }
    Ok(())
}
