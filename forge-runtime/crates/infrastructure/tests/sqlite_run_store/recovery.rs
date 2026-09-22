use super::*;

#[test]
fn unmatched_tool_start_remains_pending_and_never_becomes_terminal() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    append_event(&fixture, 1, run_started());
    append_event(&fixture, 2, current_user());
    append_event(&fixture, 3, RuntimeEventKind::TurnStarted { turn: 1 });
    append_event(&fixture, 4, assistant("reading", vec![tool_call()]));
    append_event(
        &fixture,
        5,
        RuntimeEventKind::ToolStarted { call: tool_call() },
    );
    let inspection = fixture.store.inspect_run("run-1").expect("inspect");
    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::PendingTool { ref calls } if calls == &[tool_call()]
    ));
    assert!(
        fixture
            .store
            .append_event(&fixture.event(
                6,
                RuntimeEventKind::RunFinished {
                    outcome: RunOutcome::Cancelled,
                },
            ))
            .is_err()
    );
}

#[test]
fn local_execution_evidence_survives_run_event_storage_and_recovery() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let call = ToolCall {
        id: "exec-call".into(),
        name: "exec_command".into(),
        arguments: serde_json::json!({}),
    };
    append_event(&fixture, 1, run_started());
    append_event(&fixture, 2, current_user());
    append_event(&fixture, 3, RuntimeEventKind::TurnStarted { turn: 1 });
    append_event(&fixture, 4, assistant("verify", vec![call.clone()]));
    append_event(
        &fixture,
        5,
        RuntimeEventKind::ToolStarted { call: call.clone() },
    );
    let evidence = local_execution_evidence(&fixture.conversation_id);
    append_event(
        &fixture,
        6,
        RuntimeEventKind::ToolFinished {
            call_id: call.id,
            name: call.name,
            output: "ok".into(),
            is_error: false,
            truncated: false,
            execution_evidence: Some(evidence.clone()),
        },
    );

    let inspection = fixture
        .store
        .inspect_run("run-1")
        .expect("inspect stored run");

    assert!(matches!(
        &inspection.events[5].kind,
        RuntimeEventKind::ToolFinished {
            execution_evidence: Some(persisted),
            ..
        } if persisted == &evidence
    ));
    assert!(matches!(
        inspection.resume_point(),
        Ok(RunResumePoint::CommitToolMessage { .. })
    ));
}

fn local_execution_evidence(session_id: &str) -> ExecutionEvidence {
    ExecutionEvidence {
        v: EXECUTION_FABRIC_ABI_VERSION,
        attempt_ref: forge_runtime_domain::execution::fabric::AttemptRef {
            session_id: session_id.into(),
            run_id: "run-1".into(),
            tool_started_sequence: 5,
        },
        target_ref: ExecutionTarget::local().target_ref,
        source: ExecutionEvidenceSource::LocalProcessObservation,
        observation: LocalProcessObservation {
            exit_code: Some(0),
            rendered_output_bytes: 2,
            output_truncated: false,
        },
    }
}

#[test]
fn completed_run_atomically_authorizes_one_assistant_prompt() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    append_event(&fixture, 1, run_started());

    assert!(
        fixture
            .store
            .reconcile_completed_assistant("run-1")
            .is_err()
    );
    commit_completed(&fixture);
    let first = fixture
        .store
        .reconcile_completed_assistant("run-1")
        .expect("first reconciliation");
    assert_assistant_prompt_journal(&fixture, &first.id);
    let replay = fixture
        .store
        .reconcile_completed_assistant("run-1")
        .expect("idempotent reconciliation");

    assert_eq!(first, replay);
    assert_eq!(first.conversation_id, fixture.conversation_id);
    assert_eq!(first.role, "assistant");
    assert_eq!(first.content, "done");
    assert_eq!(
        fixture
            .store
            .snapshot_at_cursor()
            .expect("snapshot after writeback replay")
            .cursor,
        3
    );
    assert_eq!(
        fixture
            .store
            .list_prompts(Some(&fixture.conversation_id), 10)
            .expect("prompt list")
            .into_iter()
            .filter(|prompt| prompt.role == "assistant")
            .count(),
        1
    );
}

fn assert_assistant_prompt_journal(fixture: &Fixture, prompt_id: &str) {
    let changes = fixture
        .store
        .conversation_changes_after(0, 10)
        .expect("read change journal after assistant writeback");
    assert_eq!(changes.head_cursor, 3);
    assert_eq!(changes.changes.len(), 3);
    assert_eq!(changes.changes[2].entity_id, prompt_id);
}
