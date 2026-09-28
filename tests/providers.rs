use herdr_infobox::model::{DocumentPhase, Observation, Provider, SessionKey};
use herdr_infobox::providers::{IngressContext, decode, decode_codex_completed};

fn context() -> IngressContext {
    IngressContext {
        host_id: "host-example".into(),
        ingress_id: "ingress-example".into(),
        devin_project_dir: None,
    }
}

#[test]
fn claude_keeps_requests_approval_and_execution_distinct() {
    let request = decode(
        Provider::Claude,
        include_bytes!("../fixtures/claude/docs-2026-09-28/webfetch-request.json"),
        &context(),
    )
    .unwrap();
    assert!(request.events.iter().any(|e| matches!(e, Observation::Reference { url, title: None, relation, .. } if url == "https://example.com/docs" && relation == "requested")));
    let approved = decode(
        Provider::Claude,
        include_bytes!("../fixtures/claude/docs-2026-09-28/plan-approved.json"),
        &context(),
    )
    .unwrap();
    assert!(approved.events.iter().any(|e| matches!(e, Observation::Plan { markdown, phase: DocumentPhase::Approved, .. } if markdown == "# Plan\n\nImplement and test the API.")));
    assert!(
        !approved
            .events
            .iter()
            .any(|e| matches!(e, Observation::Execution { .. }))
    );
    let proposed = decode(
        Provider::Claude,
        include_bytes!("../fixtures/claude/docs-2026-09-28/plan-proposed.json"),
        &context(),
    )
    .unwrap();
    assert_ne!(proposed.source_event_id, approved.source_event_id);
}

#[test]
fn codex_checklist_is_not_a_plan_and_unknown_tools_do_not_supply_urls() {
    let batch = decode(
        Provider::Codex,
        include_bytes!("../fixtures/codex/source-88235f88/checklist.json"),
        &context(),
    )
    .unwrap();
    assert!(batch.events.iter().any(|e| matches!(e, Observation::Tasks { replace: true, tasks, .. } if tasks.len() == 1 && tasks[0].text == "Implement the API" && tasks[0].status == "in_progress")));
    assert!(
        !batch
            .events
            .iter()
            .any(|e| matches!(e, Observation::Plan { .. }))
    );
    let unknown = decode(
        Provider::Codex,
        include_bytes!("../fixtures/codex/source-88235f88/unknown-tool.json"),
        &context(),
    )
    .unwrap();
    assert!(
        !unknown
            .events
            .iter()
            .any(|e| matches!(e, Observation::Reference { .. }))
    );
}

#[test]
fn cursor_reports_malformed_nested_json_and_separates_workspace_paths() {
    let batch = decode(
        Provider::Cursor,
        include_bytes!("../fixtures/cursor/docs-2026-09-28/malformed-output.json"),
        &context(),
    )
    .unwrap();
    assert!(batch.events.iter().any(|e| matches!(e, Observation::Capability { feature, reason, .. } if feature == "parser" && reason.contains("Malformed"))));
    let batch = decode(
        Provider::Cursor,
        include_bytes!("../fixtures/cursor/docs-2026-09-28/post-tool.json"),
        &context(),
    )
    .unwrap();
    assert_eq!(batch.session.native_session_id, "conversation-example");
    assert_eq!(
        batch
            .events
            .iter()
            .filter(|e| matches!(e, Observation::Path { relation, .. } if relation == "workspace"))
            .count(),
        2
    );
}

#[test]
fn devin_claude_import_is_not_misattributed_and_failure_is_not_opened() {
    let mut ctx = context();
    ctx.devin_project_dir = Some("/example/api".into());
    let batch = decode(
        Provider::Claude,
        include_bytes!("../fixtures/devin/docs-2026-09-28/failed-tool.json"),
        &ctx,
    )
    .unwrap();
    assert_eq!(batch.session.provider, Provider::Devin);
    assert!(
        !batch
            .events
            .iter()
            .any(|e| matches!(e, Observation::Reference { .. }))
    );
    assert_eq!(batch.source_event_id, "ingress-example");
}

#[test]
fn transcript_plan_has_session_guard_and_no_execution_inference() {
    let session = SessionKey {
        host_id: "host".into(),
        provider: Provider::Codex,
        native_session_id: "thread-example".into(),
        agent_scope: "main".into(),
    };
    let bytes = include_bytes!("../fixtures/codex/source-88235f88/completed-plan.json");
    let batch = decode_codex_completed(bytes, &session, "file", 42).unwrap();
    assert!(
        matches!(&batch.events[0], Observation::Plan { markdown, phase: DocumentPhase::Proposed, .. } if markdown == "# Plan\nBuild and test.")
    );
    assert_eq!(batch.source_order, Some(42));
    let mut wrong = session;
    wrong.native_session_id = "another-thread".into();
    assert!(decode_codex_completed(bytes, &wrong, "file", 42).is_err());
}

#[test]
fn claude_sdk_results_are_structured_and_tasks_remain_separate() {
    let batch = decode(
        Provider::Claude,
        include_bytes!("../fixtures/claude/docs-2026-09-28/search.json"),
        &context(),
    )
    .unwrap();
    let refs = batch
        .events
        .iter()
        .filter(|e| matches!(e, Observation::Reference { .. }))
        .collect::<Vec<_>>();
    assert_eq!(refs.len(), 1);
    assert!(
        matches!(refs[0], Observation::Reference { url, title: Some(title), relation, .. } if url == "https://example.com/docs" && title == "Example docs" && relation == "search_result")
    );
    let batch = decode(
        Provider::Claude,
        include_bytes!("../fixtures/claude/docs-2026-09-28/todos.json"),
        &context(),
    )
    .unwrap();
    assert!(batch.events.iter().any(|e| matches!(e, Observation::Tasks { replace: true, tasks, .. } if tasks[0].text == "Implement API" && tasks[0].status == "in_progress")));
    assert!(
        !batch
            .events
            .iter()
            .any(|e| matches!(e, Observation::Plan { .. } | Observation::Execution { .. }))
    );
}

#[test]
fn transcript_cursor_waits_for_complete_lines_and_recovers_truncation() {
    use herdr_infobox::model::SessionSummary;
    use herdr_infobox::providers::read_codex_transcript;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollout.jsonl");
    let session = SessionSummary {
        id: "local-session".into(),
        key: SessionKey {
            host_id: "host".into(),
            provider: Provider::Codex,
            native_session_id: "thread-example".into(),
            agent_scope: "main".into(),
        },
        ended: false,
    };
    let meta = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"thread-example\"}}\n";
    let complete = serde_json::to_string(
        &serde_json::from_slice::<serde_json::Value>(include_bytes!(
            "../fixtures/codex/source-88235f88/completed-plan.json"
        ))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(&path, format!("{meta}{complete}")).unwrap();
    let first = read_codex_transcript(&path, &session, None).unwrap();
    assert_eq!(first.cursor_after.offset, meta.len() as u64);
    assert!(first.batches.is_empty());
    std::fs::write(&path, format!("{meta}{complete}\n")).unwrap();
    let second = read_codex_transcript(&path, &session, Some(first.cursor_after)).unwrap();
    assert_eq!(second.batches.len(), 1);
    std::fs::write(&path, meta).unwrap();
    let third = read_codex_transcript(&path, &session, Some(second.cursor_after)).unwrap();
    assert_eq!(third.cursor_after.offset, meta.len() as u64);
    assert!(third.batches.is_empty());
}

#[test]
fn missing_fetch_status_does_not_claim_opened() {
    let payload = br#"{"session_id":"s","cwd":"/example","hook_event_name":"PostToolUse","tool_name":"WebFetch","tool_input":{"url":"https://example.com/a/../b"},"tool_response":{}}"#;
    let batch = decode(Provider::Claude, payload, &context()).unwrap();
    assert!(batch.events.iter().any(|e| matches!(e, Observation::Reference { url, relation, .. } if url == "https://example.com/a/../b" && relation == "requested")));
    assert!(
        !batch
            .events
            .iter()
            .any(|e| matches!(e, Observation::Reference { relation, .. } if relation == "opened"))
    );
}
