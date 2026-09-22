use super::*;

#[test]
fn begin_is_project_bound_and_reports_created_or_replayed_atomically() {
    let fixture = Fixture::new();
    let created = fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let mut retry = fixture.begin("run-key");
    retry.run_id = "newly-generated-retry-id".into();
    retry.created_at_ms = 999;
    let replayed = fixture.store.begin_run(&retry).expect("semantic replay");

    assert_eq!(created.disposition, BeginRunDisposition::Created);
    assert_eq!(replayed.disposition, BeginRunDisposition::Replayed);
    assert_eq!(replayed.run, created.run);
    assert_eq!(replayed.prompt.content, "inspect README");
    assert_eq!(
        fixture.store.list_runs(None, 10).expect("list Runs"),
        [created.run]
    );
}

#[test]
fn begin_rejects_non_project_scope_and_mismatched_prompt() {
    let fixture = Fixture::new();
    let global = fixture
        .store
        .create_conversation(&ConversationScope::Global, "Global", "global-key")
        .expect("Global Conversation");
    let global_prompt = fixture
        .store
        .append_prompt(&global.id, "user", "global", "global-prompt")
        .expect("Global Prompt");
    let mut invalid = fixture.begin("invalid-run");
    invalid.conversation_id = global.id;
    invalid.prompt_id = global_prompt.id;
    assert!(matches!(
        fixture.store.begin_run(&invalid),
        Err(RunStoreError::Conflict { .. })
    ));

    let second = fixture
        .store
        .append_prompt(
            &fixture.conversation_id,
            "assistant",
            "not user input",
            "assistant-prompt",
        )
        .expect("assistant Prompt");
    invalid = fixture.begin("assistant-run");
    invalid.prompt_id = second.id;
    assert!(matches!(
        fixture.store.begin_run(&invalid),
        Err(RunStoreError::Conflict { .. })
    ));
}

#[test]
fn append_is_contiguous_idempotent_and_terminal() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let started = fixture.event(1, run_started());
    let skipped = fixture.event(2, RuntimeEventKind::TurnStarted { turn: 1 });
    assert!(fixture.store.append_event(&skipped).is_err());
    fixture.store.append_event(&started).expect("first event");
    fixture.store.append_event(&started).expect("exact replay");

    let divergent = fixture.event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: "different".into(),
        },
    );
    assert!(matches!(
        fixture.store.append_event(&divergent),
        Err(RunStoreError::Conflict { .. })
    ));
    commit_completed(&fixture);
    assert!(fixture.store.append_event(&skipped).is_err());
    let inspection = fixture.store.inspect_run("run-1").expect("inspect");
    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { .. }
        }
    ));
}

#[test]
fn first_event_must_repeat_the_bound_prompt_exactly() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let mismatched = fixture.event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: "different prompt".into(),
        },
    );

    assert!(matches!(
        fixture.store.append_event(&mismatched),
        Err(RunStoreError::Conflict { .. })
    ));
    assert!(
        fixture
            .store
            .inspect_run("run-1")
            .expect("Run remains inspectable")
            .events
            .is_empty()
    );
}

#[test]
fn oversized_event_is_rejected_before_it_reaches_sqlite() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let oversized = fixture.event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: "x".repeat(MAX_RUN_EVENT_JSON_BYTES + 1),
        },
    );

    assert!(matches!(
        fixture.store.append_event(&oversized),
        Err(RunStoreError::Conflict { .. })
    ));
    assert!(
        fixture
            .store
            .inspect_run("run-1")
            .expect("Run remains inspectable")
            .events
            .is_empty()
    );
}
