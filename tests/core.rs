use herdr_infobox::{
    config::Paths,
    ingest,
    model::*,
    store::{Store, normalize_url},
};
fn setup() -> (tempfile::TempDir, Paths, Store, SessionSummary) {
    let temp = tempfile::tempdir().unwrap();
    let paths = Paths::open(Some(temp.path().into())).unwrap();
    let mut store = Store::open(&paths).unwrap();
    let key = SessionKey {
        host_id: paths.host_id.clone(),
        provider: Provider::Claude,
        native_session_id: "test-session".into(),
        agent_scope: "main".into(),
    };
    let id = store.register(&key).unwrap();
    let session = store.resolve(&id[..8]).unwrap();
    (temp, paths, store, session)
}
#[test]
fn session_identity_resume_and_scope_are_separate() {
    let (_temp, paths, mut store, s) = setup();
    assert_eq!(store.register(&s.key).unwrap(), s.id);
    let mut key = s.key.clone();
    key.provider = Provider::Codex;
    assert_ne!(store.register(&key).unwrap(), s.id);
    key.provider = Provider::Claude;
    key.agent_scope = "worker".into();
    assert_ne!(store.register(&key).unwrap(), s.id);
    drop(store);
    assert_eq!(Store::open(&paths).unwrap().sessions().unwrap().len(), 3);
}
#[test]
fn reference_dedup_title_provenance_and_failed_fetch_survive_resume() {
    let (_temp, paths, mut store, s) = setup();
    let batch = ingest::manual(
        s.key.clone(),
        vec![Observation::Reference {
            url: "HTTPS://EXAMPLE.COM:443/a?b=2&a=1#x".into(),
            title: Some("My title".into()),
            title_source: Some("manual".into()),
            relation: "manual".into(),
        }],
    );
    assert!(store.commit_batch(&batch).unwrap());
    assert!(!store.commit_batch(&batch).unwrap());
    store
        .commit_batch(&ingest::manual(
            s.key.clone(),
            vec![
                Observation::Reference {
                    url: "https://example.com/a?b=2&a=1#x".into(),
                    title: Some("Lower quality".into()),
                    title_source: Some("provider_result".into()),
                    relation: "search_result".into(),
                },
                Observation::FetchFailed {
                    url: "https://example.com/a?b=2&a=1#x".into(),
                    reason: "timeout".into(),
                },
            ],
        ))
        .unwrap();
    drop(store);
    let view = Store::open(&paths).unwrap().view(&s).unwrap();
    assert_eq!(view.references.len(), 1);
    assert_eq!(view.references[0].title.as_deref(), Some("My title"));
    assert!(!view.references[0].relations.contains(&"opened".into()));
    assert_eq!(view.references[0].failure.as_deref(), Some("timeout"));
}
#[test]
fn url_normalization_preserves_signed_path_query_and_anchor() {
    assert_eq!(
        normalize_url("HTTPS://EXAMPLE.COM:443/a/../b?sig=a%2Fb&x=1#part").unwrap(),
        "https://example.com/a/../b?sig=a%2Fb&x=1#part"
    );
    assert_ne!(
        normalize_url("https://example.com/a/../b").unwrap(),
        normalize_url("https://example.com/b").unwrap()
    );
    assert!(normalize_url("file:///etc/passwd").is_err());
}
#[test]
fn approved_revision_is_immutable_and_checklist_does_not_complete_execution() {
    let (_temp, _paths, mut store, s) = setup();
    let plan = |text: &str, phase| Observation::Plan {
        plan_key: "plan".into(),
        markdown: text.into(),
        source_path: None,
        phase,
    };
    store
        .commit_batch(&ingest::manual(
            s.key.clone(),
            vec![
                plan("first", DocumentPhase::Approved),
                Observation::Execution {
                    plan_key: "plan".into(),
                    revision_hash: content_hash(b"first"),
                    state: ExecutionState::Executing,
                    evidence: "manual".into(),
                },
            ],
        ))
        .unwrap();
    store
        .commit_batch(&ingest::manual(
            s.key.clone(),
            vec![
                plan("changed", DocumentPhase::Draft),
                Observation::Tasks {
                    namespace: "session".into(),
                    replace: true,
                    tasks: vec![Task {
                        id: "1".into(),
                        text: "done".into(),
                        status: "completed".into(),
                        parent_id: None,
                    }],
                },
            ],
        ))
        .unwrap();
    let view = store.view(&s).unwrap();
    assert_eq!(view.plans.len(), 2);
    let first = view.plans.iter().find(|p| p.markdown == "first").unwrap();
    assert!(first.selected);
    assert_eq!(first.execution, Some(ExecutionState::Executing));
    assert_eq!(first.phase, DocumentPhase::Approved);
    assert_eq!(
        view.plans
            .iter()
            .find(|p| p.markdown == "changed")
            .unwrap()
            .execution,
        None
    );
}
#[test]
fn batch_failure_rolls_back_every_observation() {
    let (_temp, _paths, mut store, s) = setup();
    let batch = ingest::manual(
        s.key.clone(),
        vec![
            Observation::Reference {
                url: "https://example.com".into(),
                title: None,
                title_source: None,
                relation: "manual".into(),
            },
            Observation::Execution {
                plan_key: "missing".into(),
                revision_hash: "missing".into(),
                state: ExecutionState::Executing,
                evidence: "manual".into(),
            },
        ],
    );
    assert!(store.commit_batch(&batch).is_err());
    assert!(store.view(&s).unwrap().references.is_empty());
}
#[test]
fn transcript_cursor_compare_and_swap_is_atomic() {
    let (_temp, _paths, mut store, s) = setup();
    let cursor = SourceCursor {
        session_id: s.id.clone(),
        source_path: "/tmp/source".into(),
        file_identity: "inode-1".into(),
        offset: 10,
        parser_version: "fixture-1".into(),
    };
    let chunk = ImportChunk {
        batches: vec![ingest::manual(
            s.key.clone(),
            vec![Observation::Reference {
                url: "https://example.com".into(),
                title: None,
                title_source: None,
                relation: "manual".into(),
            }],
        )],
        cursor_before: None,
        cursor_after: cursor.clone(),
    };
    store.commit_import(&chunk).unwrap();
    assert!(store.commit_import(&chunk).is_err());
    assert_eq!(
        store
            .cursor(&s.id, &cursor.source_path, "fixture-1")
            .unwrap()
            .unwrap()
            .offset,
        10
    );
    assert_eq!(store.view(&s).unwrap().references.len(), 1);
}
#[test]
fn repeated_busy_spool_delivery_has_one_projection() {
    let (_temp, paths, mut store, s) = setup();
    let db = futures::executor::block_on(
        turso::Builder::new_local(paths.db.to_str().unwrap())
            .experimental_multiprocess_wal(true)
            .build(),
    )
    .unwrap();
    let conn = db.connect().unwrap();
    futures::executor::block_on(conn.execute_batch("BEGIN IMMEDIATE")).unwrap();
    let batch = ingest::manual(
        s.key.clone(),
        vec![Observation::Reference {
            url: "https://example.com".into(),
            title: None,
            title_source: None,
            relation: "manual".into(),
        }],
    );
    ingest::persist_or_spool(&paths, &batch).unwrap();
    ingest::persist_or_spool(&paths, &batch).unwrap();
    futures::executor::block_on(conn.execute_batch("ROLLBACK")).unwrap();
    assert_eq!(ingest::drain(&paths, &mut store).unwrap(), 2);
    assert_eq!(store.view(&s).unwrap().references.len(), 1);
}
#[test]
fn concurrent_first_launch_shares_complete_host_id() {
    let temp = tempfile::tempdir().unwrap();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let path = temp.path().to_owned();
            std::thread::spawn(move || Paths::open(Some(path)).unwrap().host_id)
        })
        .collect();
    let ids: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(ids.iter().all(|id| id == &ids[0]));
}

#[test]
fn opencode_spelling_is_consistent_across_cli_store_and_spool() {
    use clap::ValueEnum;
    let provider = Provider::from_str("opencode", false).unwrap();
    assert_eq!(provider, Provider::OpenCode);
    assert_eq!(serde_json::to_string(&provider).unwrap(), "\"opencode\"");
    assert_eq!(Provider::from_str("open-code", false).unwrap(), provider);
    let (_temp, _paths, mut store, mut s) = setup();
    s.key.provider = provider;
    let id = store.register(&s.key).unwrap();
    assert_eq!(store.resolve(&id).unwrap().key.provider, provider);
}

#[test]
fn execution_selection_uses_comparable_source_order_not_arrival() {
    let (_temp, _paths, mut store, s) = setup();
    for (text, order) in [("new", 20), ("old", 10), ("newer", 30)] {
        let mut batch = ingest::manual(
            s.key.clone(),
            vec![
                Observation::Plan {
                    plan_key: "ordered".into(),
                    markdown: text.into(),
                    source_path: None,
                    phase: DocumentPhase::Approved,
                },
                Observation::Execution {
                    plan_key: "ordered".into(),
                    revision_hash: content_hash(text.as_bytes()),
                    state: ExecutionState::Executing,
                    evidence: "provider".into(),
                },
            ],
        );
        batch.source = "transcript-file-1".into();
        batch.source_order = Some(order);
        store.commit_batch(&batch).unwrap();
        if text == "old" {
            assert_eq!(
                store
                    .view(&s)
                    .unwrap()
                    .plans
                    .iter()
                    .find(|p| p.selected)
                    .unwrap()
                    .markdown,
                "new"
            );
        }
    }
    assert_eq!(
        store
            .view(&s)
            .unwrap()
            .plans
            .iter()
            .find(|p| p.selected)
            .unwrap()
            .markdown,
        "newer"
    );
}

#[test]
fn fresh_view_reads_session_end_and_titles_can_fill_manual_empty_title() {
    let (_temp, _paths, mut store, s) = setup();
    store
        .commit_batch(&ingest::manual(
            s.key.clone(),
            vec![Observation::Reference {
                url: "https://example.com".into(),
                title: None,
                title_source: Some("manual".into()),
                relation: "manual".into(),
            }],
        ))
        .unwrap();
    store
        .commit_batch(&ingest::manual(
            s.key.clone(),
            vec![
                Observation::Session {
                    parent_native_id: None,
                    ended: true,
                },
                Observation::Reference {
                    url: "https://example.com".into(),
                    title: Some("Actual page".into()),
                    title_source: Some("provider_result".into()),
                    relation: "search_result".into(),
                },
            ],
        ))
        .unwrap();
    let view = store.view(&s).unwrap();
    assert!(view.session.ended);
    assert_eq!(view.references[0].title.as_deref(), Some("Actual page"));
}
#[test]
fn late_draft_cannot_downgrade_identical_proposed_revision() {
    let (_temp, _paths, mut store, s) = setup();
    for phase in [DocumentPhase::Proposed, DocumentPhase::Draft] {
        store
            .commit_batch(&ingest::manual(
                s.key.clone(),
                vec![Observation::Plan {
                    plan_key: "p".into(),
                    markdown: "unchanged".into(),
                    source_path: None,
                    phase,
                }],
            ))
            .unwrap();
    }
    assert_eq!(
        store.view(&s).unwrap().plans[0].phase,
        DocumentPhase::Proposed
    );
}
