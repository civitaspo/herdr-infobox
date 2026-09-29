use crate::Result;
use crate::model::{DocumentPhase, EventBatch, Observation, Provider, SessionKey, Task};
use serde_json::Value;
use std::path::PathBuf;

pub struct IngressContext {
    pub host_id: String,
    pub ingress_id: String,
    pub devin_project_dir: Option<PathBuf>,
}

pub fn decode(provider: Provider, input: &[u8], context: &IngressContext) -> Result<EventBatch> {
    if provider == Provider::OpenCode {
        return Err(
            "OpenCode V1 hooks are unsupported; use opencode connect with a V2 server".into(),
        );
    }
    let value: Value = serde_json::from_slice(input)?;
    let provider = if provider == Provider::Claude && context.devin_project_dir.is_some() {
        Provider::Devin
    } else {
        provider
    };
    let id_field = match provider {
        Provider::Cursor => "conversation_id",
        _ => "session_id",
    };
    let native_session_id = required(&value, id_field)?.to_owned();
    let event = required(&value, "hook_event_name")?;
    let call_id = value["tool_use_id"].as_str();
    let mut batch = EventBatch {
        schema_version: 1,
        session: SessionKey {
            host_id: context.host_id.clone(),
            provider,
            native_session_id,
            agent_scope: value["agent_id"].as_str().unwrap_or("main").to_owned(),
        },
        source: "hook".into(),
        source_event_id: call_id
            .map(|id| format!("{id}:{event}"))
            .unwrap_or_else(|| context.ingress_id.clone()),
        native_call_key: call_id.map(str::to_owned),
        source_order: None,
        events: vec![Observation::Session {
            parent_native_id: None,
            ended: matches!(event, "SessionEnd" | "sessionEnd"),
        }],
    };
    let cwd = value["cwd"].as_str().map(PathBuf::from).or_else(|| {
        if provider == Provider::Devin {
            context.devin_project_dir.clone()
        } else {
            None
        }
    });
    if let Some(cwd) = &cwd {
        batch.events.push(Observation::Path {
            path: cwd.clone(),
            cwd: cwd.clone(),
            relation: "cwd".into(),
        });
    }
    match provider {
        Provider::Claude => claude(&value, event, cwd.as_ref(), &mut batch.events)?,
        Provider::Codex => codex(&value, event, &mut batch.events)?,
        Provider::Cursor => cursor(&value, event, &mut batch.events)?,
        Provider::OpenCode => unreachable!(),
        Provider::Devin => unavailable(
            &mut batch.events,
            "web_and_plan",
            "Devin tool argument schemas have not been runtime verified; use ref add and plan attach.",
        ),
    }
    Ok(batch)
}

fn required<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing or invalid {field}").into())
}

fn unavailable(events: &mut Vec<Observation>, feature: &str, reason: &str) {
    events.push(Observation::Capability {
        feature: feature.into(),
        state: "partial".into(),
        reason: reason.into(),
    });
}

fn reference(
    events: &mut Vec<Observation>,
    url: &str,
    title: Option<&str>,
    relation: &str,
) -> Result<()> {
    let parsed = url::Url::parse(url)?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("Reference must be an HTTP or HTTPS URL".into());
    }
    events.push(Observation::Reference {
        url: url.to_owned(),
        title: title.map(str::to_owned),
        title_source: title.map(|_| "provider_result".into()),
        relation: relation.into(),
    });
    Ok(())
}

fn claude(
    value: &Value,
    event: &str,
    cwd: Option<&PathBuf>,
    events: &mut Vec<Observation>,
) -> Result<()> {
    let tool = value["tool_name"].as_str().unwrap_or("");
    let input = &value["tool_input"];
    if matches!(tool, "Read" | "Write" | "Edit")
        && event == "PostToolUse"
        && let (Some(path), Some(cwd)) = (input["file_path"].as_str(), cwd)
    {
        events.push(Observation::Path {
            path: path.into(),
            cwd: cwd.clone(),
            relation: if tool == "Read" { "read" } else { "write" }.into(),
        });
    }
    if tool == "WebFetch" {
        let url = required(input, "url")?;
        match event {
            "PreToolUse" => reference(events, url, None, "requested")?,
            "PostToolUse" => {
                let response = &value["tool_response"];
                if response["code"]
                    .as_u64()
                    .is_some_and(|code| (200..300).contains(&code))
                {
                    reference(events, required(response, "url")?, None, "opened")?;
                } else {
                    reference(events, url, None, "requested")?;
                    unavailable(
                        events,
                        "webfetch",
                        "Fetch success unavailable or HTTP response was unsuccessful.",
                    );
                }
            }
            "PostToolUseFailure" => events.push(Observation::FetchFailed {
                url: url.into(),
                reason: "Provider reported fetch failure".into(),
            }),
            _ => (),
        }
    }
    if tool == "ExitPlanMode" {
        let (plan, path, phase) = match event {
            "PreToolUse" => (
                &input["plan"],
                &input["planFilePath"],
                DocumentPhase::Proposed,
            ),
            "PostToolUse" => (
                &value["tool_response"]["plan"],
                &value["tool_response"]["filePath"],
                DocumentPhase::Approved,
            ),
            _ => return Ok(()),
        };
        let text = plan.as_str().ok_or("ExitPlanMode plan text unavailable")?;
        let path = path.as_str().ok_or("ExitPlanMode plan path unavailable")?;
        events.push(Observation::Plan {
            plan_key: path.into(),
            markdown: text.into(),
            source_path: Some(path.into()),
            phase,
        });
    }
    if event == "PostToolUse" && tool == "WebSearch" {
        let results = value["tool_response"]["results"]
            .as_array()
            .ok_or("Unsupported Claude WebSearch response")?;
        for group in results {
            if group.is_string() {
                continue;
            }
            let hits = group["content"]
                .as_array()
                .ok_or("Unsupported Claude search result group")?;
            for hit in hits {
                reference(
                    events,
                    required(hit, "url")?,
                    Some(required(hit, "title")?),
                    "search_result",
                )?;
            }
        }
    }
    if event == "PostToolUse" && matches!(tool, "TodoWrite" | "TaskList" | "TaskGet") {
        let response = &value["tool_response"];
        let values: Vec<&Value> = match tool {
            "TodoWrite" => response["newTodos"]
                .as_array()
                .ok_or("Unsupported TodoWrite response")?
                .iter()
                .collect(),
            "TaskList" => response["tasks"]
                .as_array()
                .ok_or("Unsupported TaskList response")?
                .iter()
                .collect(),
            _ if response["task"].is_null() => Vec::new(),
            _ => vec![&response["task"]],
        };
        let tasks = values
            .iter()
            .enumerate()
            .map(|(i, task)| {
                let status = required(task, "status")?;
                if !matches!(status, "pending" | "in_progress" | "completed") {
                    return Err("Unknown Claude task status".into());
                }
                Ok(Task {
                    id: if tool == "TodoWrite" {
                        i.to_string()
                    } else {
                        required(task, "id")?.into()
                    },
                    text: required(
                        task,
                        if tool == "TodoWrite" {
                            "content"
                        } else {
                            "subject"
                        },
                    )?
                    .into(),
                    status: status.into(),
                    parent_id: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        events.push(Observation::Tasks {
            namespace: if tool == "TodoWrite" { "todo" } else { "tasks" }.into(),
            replace: tool != "TaskGet",
            tasks,
        });
    }
    unavailable(
        events,
        "runtime",
        "Claude schemas are fixture-tested against SDK 0.3.283. Live CLI hook delivery remains unverified.",
    );
    Ok(())
}

fn codex(value: &Value, event: &str, events: &mut Vec<Observation>) -> Result<()> {
    if event == "PostToolUse" && value["tool_name"] == "update_plan" {
        let items = value["tool_input"]["plan"]
            .as_array()
            .ok_or("Invalid update_plan checklist")?;
        let tasks = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let status = required(item, "status")?;
                if !matches!(status, "pending" | "in_progress" | "completed") {
                    return Err("Unknown checklist status".into());
                }
                Ok(Task {
                    id: i.to_string(),
                    text: required(item, "step")?.into(),
                    status: status.into(),
                    parent_id: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        events.push(Observation::Tasks {
            namespace: "update_plan".into(),
            replace: true,
            tasks,
        });
    }
    unavailable(
        events,
        "web",
        "Hosted WebSearch bypasses hooks; only known transcript records or manual references can supplement it.",
    );
    unavailable(
        events,
        "plan",
        "update_plan is a checklist, not a Plan document. Attach a document or reconcile a verified transcript format.",
    );
    Ok(())
}

fn cursor(value: &Value, event: &str, events: &mut Vec<Observation>) -> Result<()> {
    if let Some(roots) = value["workspace_roots"].as_array() {
        for root in roots {
            if let Some(root) = root.as_str() {
                events.push(Observation::Path {
                    path: root.into(),
                    cwd: root.into(),
                    relation: "workspace".into(),
                });
            }
        }
    }
    let encoded = match event {
        "postToolUse" => value["tool_output"].as_str(),
        "afterMCPExecution" => value["result_json"].as_str(),
        _ => None,
    };
    if let Some(encoded) = encoded
        && serde_json::from_str::<Value>(encoded).is_err()
    {
        unavailable(
            events,
            "parser",
            "Malformed JSON-stringified Cursor tool result; no result facts were inferred.",
        );
    }
    unavailable(
        events,
        "cursor_hooks",
        "Built-in Web and Plan calls require a registered Cursor transcript or saved stream-json file. Use reconcile --session SESSION --file PATH.",
    );
    Ok(())
}

pub fn decode_codex_completed(
    input: &[u8],
    session: &SessionKey,
    source_id: &str,
    offset: u64,
) -> Result<EventBatch> {
    let value: Value = serde_json::from_slice(input)?;
    if session.provider != Provider::Codex
        || value["type"] != "event_msg"
        || value["payload"]["type"] != "item_completed"
    {
        return Err("Unsupported Codex transcript record".into());
    }
    let payload = &value["payload"];
    if required(payload, "thread_id")? != session.native_session_id {
        return Err("Transcript belongs to another session".into());
    }
    let item = &payload["item"];
    let mut events = Vec::new();
    match required(item, "type")? {
        "Plan" => events.push(Observation::Plan {
            plan_key: "codex-plan".into(),
            markdown: required(item, "text")?.into(),
            source_path: None,
            phase: DocumentPhase::Proposed,
        }),
        "WebSearch" => {
            if let Some(results) = item["results"].as_array() {
                for result in results.iter().filter(|r| r["type"] == "text_result") {
                    reference(
                        &mut events,
                        required(result, "url")?,
                        result["title"].as_str(),
                        "search_result",
                    )?;
                }
            } else {
                unavailable(
                    &mut events,
                    "web",
                    "Completed WebSearch has no structured results; result URLs and titles unavailable.",
                );
            }
        }
        _ => return Err("Unsupported completed Codex item".into()),
    }
    Ok(EventBatch {
        schema_version: 1,
        session: session.clone(),
        source: "transcript".into(),
        source_event_id: format!("{source_id}:{offset}"),
        native_call_key: item["id"].as_str().map(str::to_owned),
        source_order: Some(offset),
        events,
    })
}

pub const CODEX_TRANSCRIPT_PARSER: &str = "codex-completed-88235f88-v1";

pub fn read_codex_transcript(
    path: &std::path::Path,
    session: &crate::model::SessionSummary,
    previous: Option<crate::model::SourceCursor>,
) -> Result<crate::model::ImportChunk> {
    use crate::model::{ImportChunk, SourceCursor};
    use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
    const LIMIT: u64 = 8 * 1024 * 1024;
    if session.key.provider != Provider::Codex {
        return Err("Only the source-verified Codex JSONL reader is available".into());
    }
    let file = crate::transcripts::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err("Transcript must be a regular file".into());
    }
    #[cfg(unix)]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        format!("{}:{}", metadata.dev(), metadata.ino())
    };
    #[cfg(not(unix))]
    let identity = format!("{:?}", metadata.created()?);
    let mut reader = BufReader::new(file);
    let mut first = Vec::new();
    (&mut reader)
        .take(LIMIT + 1)
        .read_until(b'\n', &mut first)?;
    if first.len() as u64 > LIMIT || first.last() != Some(&b'\n') {
        return Err("Session metadata line is incomplete or exceeds limit".into());
    }
    let first: Value = serde_json::from_slice(&first)?;
    if first["type"] != "session_meta"
        || first["payload"]["id"].as_str() != Some(&session.key.native_session_id)
    {
        return Err("Transcript session metadata does not match selected session".into());
    }
    let start = previous
        .as_ref()
        .filter(|c| {
            c.file_identity == identity
                && c.offset <= metadata.len()
                && c.parser_version == CODEX_TRANSCRIPT_PARSER
        })
        .map_or(0, |c| c.offset);
    reader.seek(SeekFrom::Start(start))?;
    let mut offset = start;
    let mut batches = Vec::new();
    loop {
        let mut line = Vec::new();
        let size = (&mut reader).take(LIMIT + 1).read_until(b'\n', &mut line)? as u64;
        if size == 0 || line.last() != Some(&b'\n') {
            break;
        }
        if size > LIMIT {
            return Err(format!("Transcript line exceeds limit at byte {offset}").into());
        }
        if offset - start + size > LIMIT {
            break;
        }
        let value: Value = serde_json::from_slice(&line)
            .map_err(|_| format!("Malformed transcript JSON at byte {offset}"))?;
        let kind = value["type"]
            .as_str()
            .ok_or_else(|| format!("Unknown transcript schema at byte {offset}"))?;
        if kind == "session_meta"
            && value["payload"]["id"].as_str() != Some(&session.key.native_session_id)
        {
            return Err(format!("Different session metadata at byte {offset}").into());
        }
        if kind == "event_msg" && value["payload"]["type"] == "item_completed" {
            match decode_codex_completed(&line, &session.key, &identity, offset) {
                Ok(batch) => batches.push(batch),
                Err(error) => {
                    let item_kind = value["payload"]["item"]["type"].as_str().unwrap_or("");
                    if matches!(item_kind, "Plan" | "WebSearch") {
                        return Err(
                            format!("Invalid transcript item at byte {offset}: {error}").into()
                        );
                    }
                    let mut events = Vec::new();
                    unavailable(
                        &mut events,
                        "transcript",
                        "Completed transcript item type is not parsed; some information may be unavailable.",
                    );
                    batches.push(EventBatch {
                        schema_version: 1,
                        session: session.key.clone(),
                        source: "transcript".into(),
                        source_event_id: format!("{identity}:{offset}"),
                        native_call_key: None,
                        source_order: Some(offset),
                        events,
                    });
                }
            }
        } else if !matches!(
            kind,
            "session_meta"
                | "event_msg"
                | "response_item"
                | "turn_context"
                | "compacted"
                | "token_usage_record"
                | "world_state"
                | "retained_context"
                | "security_risk_score"
                | "inter_agent_communication"
                | "inter_agent_communication_metadata"
                | "realtime_item"
        ) {
            return Err(format!("Unknown transcript record at byte {offset}").into());
        }
        offset += size;
    }
    let after = reader.get_ref().metadata()?;
    if after.len() < offset {
        return Err("Transcript truncated while reading; retry".into());
    }
    Ok(ImportChunk {
        batches,
        cursor_before: previous,
        cursor_after: SourceCursor {
            session_id: session.id.clone(),
            source_path: path.into(),
            file_identity: identity,
            offset,
            parser_version: CODEX_TRANSCRIPT_PARSER.into(),
        },
    })
}
