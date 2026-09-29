//! Readers for explicitly bound Cursor CLI transcripts and saved print streams.
use crate::{Result, model::*};
use serde_json::Value;
use std::{collections::HashMap, io::Read, path::Path};
pub const TRANSCRIPT_PARSER: &str = "cursor-transcript-2026.09.26-v1";
pub const STREAM_PARSER: &str = "cursor-stream-2026.09.26-v1";
const LIMIT: u64 = 16 * 1024 * 1024;
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing Cursor {key}").into())
}
fn capability(feature: &str, reason: &str) -> Observation {
    Observation::Capability {
        feature: feature.into(),
        state: "partial".into(),
        reason: reason.into(),
    }
}
pub fn read(
    path: &Path,
    session: &SessionSummary,
    previous: Option<SourceCursor>,
) -> Result<ImportChunk> {
    if session.key.provider != Provider::Cursor {
        return Err("Cursor reader requires a Cursor session".into());
    }
    let file = crate::transcripts::open(path)?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err("Cursor source exceeds 16 MiB".into());
    }
    let end = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    let lines: Vec<Value> = bytes[..end]
        .split_inclusive(|b| *b == b'\n')
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_slice(line)
                .map_err(|_| format!("Malformed Cursor JSON at line {}", index + 1))
        })
        .collect::<std::result::Result<_, _>>()?;
    let parser = match lines.first() {
        Some(v) if v["type"] == "system" && v["subtype"] == "init" => STREAM_PARSER,
        Some(v)
            if matches!(v["role"].as_str(), Some("user" | "assistant"))
                || v["type"] == "metadata" =>
        {
            TRANSCRIPT_PARSER
        }
        None => previous
            .as_ref()
            .map(|c| c.parser_version.as_str())
            .filter(|p| matches!(*p, TRANSCRIPT_PARSER | STREAM_PARSER))
            .ok_or("Cursor source has no complete first record")?,
        _ => return Err("Unknown Cursor source format".into()),
    };
    if previous.as_ref().is_some_and(|c| {
        c.parser_version != parser || c.session_id != session.id || c.source_path != path
    }) {
        return Err("Cursor source binding or parser changed".into());
    }
    let stream = parser == STREAM_PARSER;
    if !stream
        && (path.file_stem().and_then(|s| s.to_str()) != Some(&session.key.native_session_id)
            || path.extension().and_then(|s| s.to_str()) != Some("jsonl")
            || path
                .parent()
                .and_then(Path::file_name)
                .and_then(|s| s.to_str())
                != Some(&session.key.native_session_id))
    {
        return Err(
            "Cursor transcript filename and parent must match the selected native session ID"
                .into(),
        );
    }
    let source = if stream {
        "cursor-stream"
    } else {
        "cursor-transcript"
    };
    let mut batches = Vec::new();
    let mut occurrences = HashMap::<String, u64>::new();
    let mut add =
        |value: &Value, call: Option<String>, events: Vec<Observation>, order: u64| -> Result<()> {
            if events.is_empty() {
                return Ok(());
            }
            let digest = content_hash(&serde_json::to_vec(value)?);
            let occurrence = occurrences.entry(digest.clone()).or_default();
            let id = if let Some(ref call) = call {
                format!("{call}:{digest}")
            } else {
                format!("{digest}:{occurrence}")
            };
            *occurrence += 1;
            batches.push(EventBatch {
                schema_version: 1,
                session: session.key.clone(),
                source: source.into(),
                source_event_id: id,
                native_call_key: call,
                source_order: Some(order),
                events,
            });
            Ok(())
        };
    for (order, value) in lines.iter().enumerate() {
        let parsed = (|| -> Result<()> {
            if stream {
                if value["session_id"].as_str() != Some(&session.key.native_session_id) {
                    return Err(
                        "Cursor stream record belongs to another session or lacks identity".into(),
                    );
                }
                match required(value, "type")? {
                    "system" if value["subtype"] == "init" => {
                        let cwd = required(value, "cwd")?;
                        if !Path::new(cwd).is_absolute() {
                            return Err("Cursor cwd must be absolute".into());
                        }
                        add(
                            value,
                            None,
                            vec![Observation::Path {
                                path: cwd.into(),
                                cwd: cwd.into(),
                                relation: "workspace".into(),
                            }],
                            order as u64,
                        )?;
                    }
                    "user" | "assistant" | "thinking" | "result" | "interaction_query" => {}
                    "tool_call" => {
                        let call = required(value, "call_id")?;
                        let completed = match required(value, "subtype")? {
                            "started" => false,
                            "completed" => true,
                            _ => return Err("Unknown Cursor tool event".into()),
                        };
                        let tools = value["tool_call"]
                            .as_object()
                            .ok_or("Invalid Cursor tool_call")?;
                        let entries: Vec<_> = tools
                            .iter()
                            .filter(|(name, _)| name.ends_with("ToolCall"))
                            .collect();
                        if entries.len() != 1 {
                            return Err("Unknown Cursor tool call shape".into());
                        }
                        let (name, tool) = entries[0];
                        let events =
                            tool_events(name, &tool["args"], Some((&tool["result"], completed)))?;
                        add(tool, Some(call.into()), events, order as u64)?;
                    }
                    _ => return Err("Unknown Cursor stream record".into()),
                }
            } else {
                match value["type"].as_str() {
                    Some("metadata") => {
                        if !value["metadata"].is_object() {
                            return Err("Invalid Cursor metadata".into());
                        }
                        for key in ["session_id", "conversation_id"] {
                            if let Some(id) = value["metadata"].get(key)
                                && id.as_str() != Some(&session.key.native_session_id)
                            {
                                return Err("Cursor metadata identity mismatch".into());
                            }
                        }
                        return Ok(());
                    }
                    Some("turn_ended") => {
                        if !matches!(
                            value["status"].as_str(),
                            Some("success" | "error" | "aborted")
                        ) {
                            return Err("Unknown Cursor turn status".into());
                        }
                        return Ok(());
                    }
                    Some(_) => return Err("Unknown Cursor transcript record".into()),
                    None => {}
                }
                if !matches!(required(value, "role")?, "user" | "assistant") {
                    return Err("Unknown Cursor transcript role".into());
                }
                for item in value["message"]["content"]
                    .as_array()
                    .ok_or("Invalid Cursor message content")?
                {
                    match required(item, "type")? {
                        "text" => {
                            required(item, "text")?;
                        }
                        "tool_use" if value["role"] == "assistant" => {
                            let name = required(item, "name")?;
                            add(
                                item,
                                None,
                                tool_events(name, &item["input"], None)?,
                                order as u64,
                            )?;
                        }
                        _ => return Err("Unknown Cursor message content".into()),
                    }
                }
            }
            Ok(())
        })();
        parsed.map_err(|error| format!("Cursor record at line {}: {error}", order + 1))?;
    }
    let capabilities = vec![
        capability(
            if stream {
                "cursor_stream"
            } else {
                "cursor_transcript"
            },
            if stream {
                "Saved Cursor stream: tool results are collected when explicit; Plan approval/execution and page titles are unavailable."
            } else {
                "Cursor transcript records requests and Plan content only; fetch results, page titles and Plan approval/execution are unavailable."
            },
        ),
        capability(
            "web_and_plan",
            "Cursor source registered. Coverage and missing metadata are reported separately for each source format.",
        ),
    ];
    add(
        &serde_json::to_value(&capabilities)?,
        None,
        capabilities,
        lines.len() as u64,
    )?;
    Ok(ImportChunk {
        batches,
        cursor_before: previous.clone(),
        cursor_after: SourceCursor {
            session_id: session.id.clone(),
            source_path: path.into(),
            file_identity: content_hash(&bytes[..end]),
            offset: end as u64,
            parser_version: parser.into(),
        },
    })
}
fn tool_events(
    name: &str,
    input: &Value,
    result: Option<(&Value, bool)>,
) -> Result<Vec<Observation>> {
    if !input.is_object() {
        return Err("Invalid Cursor tool input".into());
    }
    let (name, input) = if name == "CallDynamicTool" && input["namespace"] == "cursor" {
        (required(input, "toolName")?, &input["arguments"])
    } else {
        (name, input)
    };
    let mut events = Vec::new();
    match name {
        "WebFetch" | "webFetchToolCall" => {
            let url = required(input, "url")?;
            let parsed = url::Url::parse(url)?;
            if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
                return Err("Unsupported Cursor reference URL".into());
            }
            let success = if let Some((r, true)) = result {
                if r.get("error").is_some() {
                    false
                } else {
                    required(&r["success"], "url")?;
                    if !r["success"]["markdown"].is_string() {
                        return Err("Missing Cursor WebFetch success markdown".into());
                    }
                    true
                }
            } else {
                false
            };
            events.push(Observation::Reference {
                url: url.into(),
                title: None,
                title_source: None,
                relation: if success { "opened" } else { "open_requested" }.into(),
            });
            if result.is_some_and(|(r, done)| done && r.get("error").is_some()) {
                events.push(Observation::FetchFailed {
                    url: url.into(),
                    reason: "Cursor WebFetch reported an error".into(),
                });
            }
        }
        "CreatePlan" | "createPlanToolCall" => {
            let key = format!(
                "cursor-plan:{}",
                content_hash(required(input, "plan")?.as_bytes())
            );
            events.push(Observation::Plan {
                plan_key: key.clone(),
                markdown: required(input, "plan")?.into(),
                source_path: None,
                phase: DocumentPhase::Proposed,
            });
            if let Some(todos) = input.get("todos") {
                let tasks = todos
                    .as_array()
                    .ok_or("Invalid Cursor todos")?
                    .iter()
                    .map(|t| {
                        Ok(Task {
                            id: required(t, "id")?.into(),
                            text: required(t, "content")?.into(),
                            status: match t["status"].as_str() {
                                None => "unknown",
                                Some("TODO_STATUS_PENDING" | "pending") => "pending",
                                Some("TODO_STATUS_IN_PROGRESS" | "in_progress") => "in_progress",
                                Some("TODO_STATUS_COMPLETED" | "completed") => "completed",
                                Some("TODO_STATUS_CANCELLED" | "cancelled") => "cancelled",
                                Some(_) => "unknown",
                            }
                            .into(),
                            parent_id: None,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                events.push(Observation::Tasks {
                    namespace: key,
                    replace: false,
                    tasks,
                });
            }
        }
        "Read" | "Write" | "Edit" | "GetDynamicTools" | "readToolCall" | "writeToolCall"
        | "editToolCall" => {}
        _ => events.push(Observation::Capability {
            feature: "cursor_tools".into(),
            state: "partial".into(),
            reason: "An unrecognized Cursor tool was skipped; some information may be unavailable."
                .into(),
        }),
    }
    Ok(events)
}
