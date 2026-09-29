use herdr_infobox::{cursor, model::*};
fn session() -> SessionSummary {
    SessionSummary {
        id: "internal".into(),
        key: SessionKey {
            host_id: "test".into(),
            provider: Provider::Cursor,
            native_session_id: "session-test".into(),
            agent_scope: "main".into(),
        },
        ended: false,
    }
}
fn fixture(name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("session-test");
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("session-test.jsonl");
    std::fs::copy(
        format!("fixtures/cursor/2026.09.26-dd393fe/{name}.jsonl"),
        &path,
    )
    .unwrap();
    (tmp, path)
}
#[test]
fn durable_is_conservative_and_separates_tasks() {
    let (_tmp, path) = fixture("transcript");
    let chunk = cursor::read(&path, &session(), None).unwrap();
    let events: Vec<_> = chunk.batches.iter().flat_map(|b| &b.events).collect();
    assert!(events.iter().any(
        |e| matches!(e,Observation::Reference{relation,title:None,..} if relation=="open_requested")
    ));
    assert!(events.iter().any(|e| matches!(
        e,
        Observation::Plan {
            phase: DocumentPhase::Proposed,
            ..
        }
    )));
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Observation::Tasks{tasks,..} if tasks[0].status=="unknown"))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Observation::Execution { .. }))
    );
    let again = cursor::read(&path, &session(), Some(chunk.cursor_after)).unwrap();
    assert_eq!(
        chunk
            .batches
            .iter()
            .map(|b| &b.source_event_id)
            .collect::<Vec<_>>(),
        again
            .batches
            .iter()
            .map(|b| &b.source_event_id)
            .collect::<Vec<_>>()
    );
}
#[test]
fn stream_requires_identity_and_success_evidence() {
    let (_tmp, path) = fixture("stream");
    let chunk = cursor::read(&path, &session(), None).unwrap();
    assert!(
        chunk
            .batches
            .iter()
            .flat_map(|b| &b.events)
            .any(|e| matches!(e,Observation::Reference{relation,..}if relation=="opened"))
    );
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("session-test", "wrong");
    std::fs::write(&path, text).unwrap();
    assert!(cursor::read(&path, &session(), None).is_err());
}
#[test]
fn partial_and_rewritten_records_recover() {
    let (_tmp, path) = fixture("transcript");
    let original = std::fs::read(&path).unwrap();
    let first = cursor::read(&path, &session(), None).unwrap();
    let mut partial = original.clone();
    partial.extend_from_slice(b"{\"role\":");
    std::fs::write(&path, &partial).unwrap();
    let second = cursor::read(&path, &session(), Some(first.cursor_after.clone())).unwrap();
    assert_eq!(second.cursor_after.offset, original.len() as u64);
    std::fs::write(
        &path,
        String::from_utf8(original)
            .unwrap()
            .replace("example.com", "example.org"),
    )
    .unwrap();
    let third = cursor::read(&path, &session(), Some(second.cursor_after)).unwrap();
    assert_ne!(
        first.cursor_after.file_identity,
        third.cursor_after.file_identity
    );
    std::fs::write(&path,b"{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"resume\"}]}}\n").unwrap();
    assert!(cursor::read(&path, &session(), Some(third.cursor_after)).is_ok());
}
#[test]
fn rejects_unknown_schema_and_wrong_filename() {
    let (tmp, path) = fixture("transcript");
    let wrong = tmp.path().join("wrong.jsonl");
    std::fs::copy(&path, &wrong).unwrap();
    assert!(cursor::read(&wrong, &session(), None).is_err());
    std::fs::write(path.clone(), b"{\"future\":true}\n").unwrap();
    assert!(cursor::read(&path, &session(), None).is_err());
}
#[test]
fn replay_and_truncation_preserve_cached_plan() {
    use herdr_infobox::{config::Paths, store::Store};
    let (tmp, path) = fixture("transcript");
    let paths = Paths::open(Some(tmp.path().join("state"))).unwrap();
    let mut store = Store::open(&paths).unwrap();
    let id = store.register(&session().key).unwrap();
    let session = store.resolve(&id).unwrap();
    let first = cursor::read(&path, &session, None).unwrap();
    store.commit_import(&first).unwrap();
    let before = store.view(&session).unwrap();
    assert_eq!(before.plans.len(), 1);
    assert_eq!(before.tasks.len(), 2);
    let again = cursor::read(&path, &session, Some(first.cursor_after)).unwrap();
    store.commit_import(&again).unwrap();
    assert_eq!(store.view(&session).unwrap().plans.len(), 1);
    std::fs::write(&path, b"").unwrap();
    let empty = cursor::read(&path, &session, Some(again.cursor_after)).unwrap();
    store.commit_import(&empty).unwrap();
    assert_eq!(store.view(&session).unwrap().plans.len(), 1);
}
#[test]
fn failed_stream_fetch_is_not_opened() {
    let (_tmp, path) = fixture("stream");
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = text
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).unwrap();
            if value["subtype"] == "completed"
                && value["tool_call"].get("webFetchToolCall").is_some()
            {
                value["tool_call"]["webFetchToolCall"]["result"] =
                    serde_json::json!({"error":{"message":"failed"}});
            }
            serde_json::to_string(&value).unwrap()
        })
        .collect();
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let chunk = cursor::read(&path, &session(), None).unwrap();
    assert!(
        !chunk
            .batches
            .iter()
            .flat_map(|b| &b.events)
            .any(|e| matches!(e,Observation::Reference{relation,..} if relation=="opened"))
    );
}
#[test]
fn durable_and_stream_share_plan_without_status_downgrade() {
    use herdr_infobox::{config::Paths, store::Store};
    let (tmp, path) = fixture("transcript");
    let stream = tmp.path().join("stream.jsonl");
    std::fs::copy("fixtures/cursor/2026.09.26-dd393fe/stream.jsonl", &stream).unwrap();
    let paths = Paths::open(Some(tmp.path().join("state"))).unwrap();
    let mut store = Store::open(&paths).unwrap();
    let id = store.register(&session().key).unwrap();
    let session = store.resolve(&id).unwrap();
    store
        .commit_import(&cursor::read(&stream, &session, None).unwrap())
        .unwrap();
    store
        .commit_import(&cursor::read(&path, &session, None).unwrap())
        .unwrap();
    let view = store.view(&session).unwrap();
    assert_eq!(view.plans.len(), 1);
    assert_eq!(view.tasks.len(), 2);
    assert!(view.tasks.iter().all(|task| task.status == "pending"));
}
#[test]
fn missing_success_payload_is_not_inferred() {
    let (_tmp, path) = fixture("stream");
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<_> = text
        .lines()
        .map(|line| {
            let mut v: serde_json::Value = serde_json::from_str(line).unwrap();
            if v["subtype"] == "completed" && v["tool_call"].get("webFetchToolCall").is_some() {
                v["tool_call"]["webFetchToolCall"]["result"] = serde_json::json!({"success":{}});
            }
            v.to_string()
        })
        .collect();
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    let error = cursor::read(&path, &session(), None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("line"));
}
