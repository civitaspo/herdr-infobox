use herdr_infobox::model::{DocumentPhase, Observation, Provider, SessionKey};
use herdr_infobox::opencode::decode_export;
use serde_json::{Value, json};

fn session() -> SessionKey {
    SessionKey {
        host_id: "test".into(),
        provider: Provider::OpenCode,
        native_session_id: "ses_fixture".into(),
        agent_scope: "main".into(),
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/opencode/v2.0.18/export.json")).unwrap()
}
fn decode(value: &Value) -> Vec<herdr_infobox::model::EventBatch> {
    decode_export(&serde_json::to_vec(value).unwrap(), &session()).unwrap()
}

#[test]
fn export_preserves_repositories_fetch_outcomes_and_proposed_plan() {
    let batches = decode(&fixture());
    let events: Vec<_> = batches.iter().flat_map(|batch| &batch.events).collect();
    assert!(events.iter().any(|event| matches!(event, Observation::Path { path, cwd, .. } if path.to_str() == Some("src/main.rs") && cwd.to_str() == Some("/workspace/first"))));
    assert!(events.iter().any(|event| matches!(event, Observation::Path { path, .. } if path.to_str() == Some("/workspace/second"))));
    assert!(events.iter().any(|event| matches!(event, Observation::Reference { url, relation, title, .. } if url.ends_with("/docs") && relation == "opened" && title.is_none())));
    assert!(!events.iter().any(|event| matches!(event, Observation::Reference { url, relation, .. } if url.ends_with("/missing") && relation == "opened")));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Observation::FetchFailed { .. }))
    );
    assert!(events.iter().any(|event| matches!(event, Observation::Plan { plan_key, phase: DocumentPhase::Proposed, .. } if plan_key == "msg_plan")));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Observation::Execution { .. }))
    );
}

#[test]
fn replay_is_stable_and_updated_messages_are_not_deduplicated_away() {
    let value = fixture();
    let first = decode(&value);
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(decode(&value)).unwrap()
    );
    let mut running = value.clone();
    running["data"]["messages"][2]["content"][0]["state"]["status"] = json!("running");
    let before = decode(&running);
    assert_ne!(before[3].source_event_id, first[3].source_event_id);
    assert!(before[3].events.iter().any(|event| matches!(event, Observation::Reference { url, relation, .. } if url.ends_with("/docs") && relation == "requested")));
}

#[test]
fn unknown_wrong_session_partial_and_unsafe_location_fail_closed() {
    assert!(decode_export(b"{", &session()).is_err());
    assert!(
        decode_export(
            br#"{"infobox_schema":1,"sessionID":"ses_fixture"}"#,
            &session()
        )
        .is_err()
    );
    for (pointer, replacement) in [
        ("/data/info/id", json!("other")),
        ("/data/info/subpath", json!("../escape")),
        ("/data/messages/0/type", json!("future-message")),
        (
            "/data/messages/0/content/0/state/status",
            json!("future-status"),
        ),
    ] {
        let mut value = fixture();
        if pointer.ends_with("subpath") {
            value["data"]["info"]["subpath"] = replacement;
        } else {
            *value.pointer_mut(pointer).unwrap() = replacement;
        }
        assert!(decode_export(&serde_json::to_vec(&value).unwrap(), &session()).is_err());
    }
}

#[test]
fn incomplete_plan_and_task_tools_do_not_become_documents() {
    let mut value = fixture();
    value["data"]["messages"][3]["time"]
        .as_object_mut()
        .unwrap()
        .remove("completed");
    value["data"]["messages"][2]["content"] = json!([{"type":"tool","id":"task","name":"todowrite","state":{"status":"completed","input":{"todos":[{"content":"do work","status":"pending"}]}}}]);
    assert!(
        !decode(&value)
            .iter()
            .flat_map(|batch| &batch.events)
            .any(|event| matches!(event, Observation::Plan { .. } | Observation::Tasks { .. }))
    );
}

#[test]
fn identical_plan_text_keeps_distinct_message_identity_and_tool_call_provenance() {
    let mut value = fixture();
    let mut second = value["data"]["messages"][3].clone();
    second["id"] = json!("msg_plan_second");
    value["data"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(second);
    let batches = decode(&value);
    let keys: Vec<_> = batches
        .iter()
        .flat_map(|batch| &batch.events)
        .filter_map(|event| {
            if let Observation::Plan { plan_key, .. } = event {
                Some(plan_key.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(keys, ["msg_plan", "msg_plan_second"]);
    assert!(
        batches
            .iter()
            .any(|batch| batch.native_call_key.as_deref() == Some("msg_web:call_web"))
    );
    value["data"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"malformed","type":"assistant"}));
    assert!(decode_export(&serde_json::to_vec(&value).unwrap(), &session()).is_err());
}

#[test]
fn missing_previous_location_does_not_attribute_old_relative_paths_to_current_worktree() {
    let mut value = fixture();
    value["data"]["messages"][1]
        .as_object_mut()
        .unwrap()
        .remove("previous");
    let batches = decode(&value);
    assert!(!batches.iter().flat_map(|batch| &batch.events).any(|event| matches!(event, Observation::Path { path, .. } if path.to_str() == Some("src/main.rs"))));
    assert!(batches.iter().flat_map(|batch| &batch.events).any(|event| matches!(event, Observation::Capability { feature, .. } if feature == "repositories")));
}

#[test]
fn live_2_0_15_export_with_idle_markers_preserves_collected_information() {
    let batches = decode_export(
        include_bytes!("../fixtures/opencode/v2.0.15/export.json"),
        &session(),
    )
    .unwrap();
    let events: Vec<_> = batches.iter().flat_map(|batch| &batch.events).collect();
    assert!(events.iter().any(
        |event| matches!(event, Observation::Path { path, .. } if path.ends_with("sample.txt"))
    ));
    assert!(events.iter().any(|event| matches!(event, Observation::Reference { url, relation, .. } if url == "https://example.com" && relation == "opened")));
    assert!(events.iter().any(|event| matches!(
        event,
        Observation::Plan {
            phase: DocumentPhase::Proposed,
            ..
        }
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Observation::Execution { .. }))
    );
}
