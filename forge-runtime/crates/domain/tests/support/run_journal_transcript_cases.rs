use super::*;

#[test]
fn fabricated_completed_tool_call_is_rejected() {
    let mut transcript = through_assistant(vec![tool_call()]);
    transcript.push(RuntimeEventKind::RunFinished {
        outcome: RunOutcome::Completed {
            answer: "fabricated".into(),
        },
    });

    let error = validate(transcript).expect_err("unresolved tool call cannot complete");

    assert!(error.message.contains("unresolved tool calls"));
}

#[test]
fn execution_evidence_must_bind_to_the_adjacent_local_tool_start() {
    let call = ToolCall {
        id: "exec-1".into(),
        name: "exec_command".into(),
        arguments: json!({}),
    };
    let mut transcript = through_assistant(vec![call.clone()]);
    transcript.push(RuntimeEventKind::ToolStarted { call: call.clone() });
    transcript.push(RuntimeEventKind::ToolFinished {
        call_id: call.id,
        name: call.name,
        output: "ok".into(),
        is_error: false,
        truncated: false,
        execution_evidence: Some(local_evidence(4, 2)),
    });

    let error = validate(transcript).expect_err("unbound evidence is rejected");

    assert!(error.message.contains("execution evidence"));
}

#[test]
fn execution_evidence_observation_must_match_the_tool_finished_output() {
    let call = ToolCall {
        id: "exec-1".into(),
        name: "exec_command".into(),
        arguments: json!({}),
    };
    let mut transcript = through_assistant(vec![call.clone()]);
    transcript.push(RuntimeEventKind::ToolStarted { call: call.clone() });
    transcript.push(RuntimeEventKind::ToolFinished {
        call_id: call.id,
        name: call.name,
        output: "ok".into(),
        is_error: false,
        truncated: false,
        execution_evidence: Some(local_evidence(5, 1)),
    });

    let error = validate(transcript).expect_err("mismatched observation is rejected");

    assert!(error.message.contains("execution evidence"));
}

fn local_evidence(start_sequence: u64, output_bytes: u64) -> ExecutionEvidence {
    ExecutionEvidence {
        v: EXECUTION_FABRIC_ABI_VERSION,
        attempt_ref: forge_runtime_domain::execution::fabric::AttemptRef {
            session_id: "conversation-1".into(),
            run_id: "run-1".into(),
            tool_started_sequence: start_sequence,
        },
        target_ref: ExecutionTarget::local().target_ref,
        source: ExecutionEvidenceSource::LocalProcessObservation,
        observation: LocalProcessObservation {
            exit_code: Some(0),
            rendered_output_bytes: output_bytes,
            output_truncated: false,
        },
    }
}

#[test]
fn contradictory_finished_tool_output_is_rejected() {
    let call = tool_call();
    let mut transcript = through_assistant(vec![call.clone()]);
    transcript.extend([
        RuntimeEventKind::ToolStarted { call: call.clone() },
        finished(&call, "durable output"),
        tool_message(&call, "different output", false, false),
    ]);

    let error = validate(transcript).expect_err("tool message must repeat durable output");

    assert!(error.message.contains("contradicts"));
}

#[test]
fn completed_multi_turn_tool_transcript_is_accepted() {
    let call = tool_call();
    let mut transcript = through_assistant(vec![call.clone()]);
    transcript.extend([
        RuntimeEventKind::ToolStarted { call: call.clone() },
        finished(&call, "contents"),
        tool_message(&call, "contents", false, false),
        RuntimeEventKind::TurnStarted { turn: 2 },
        assistant("final answer", Vec::new()),
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: "final answer".into(),
            },
        },
    ]);

    let inspection = validate(transcript).expect("real tool transcript");

    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { ref answer }
        } if answer == "final answer"
    ));
}

#[test]
fn pending_recovery_exposes_only_started_without_finished_effect() {
    let call = tool_call();
    let unresolved = through_assistant(vec![call.clone()]);
    assert!(matches!(
        validate(unresolved.clone())
            .expect("unstarted prefix")
            .recovery
            .state,
        RunRecoveryState::Incomplete
    ));

    let mut started = unresolved;
    started.push(RuntimeEventKind::ToolStarted { call: call.clone() });
    assert!(matches!(
        validate(started.clone()).expect("started prefix").recovery.state,
        RunRecoveryState::PendingTool { ref calls } if calls == std::slice::from_ref(&call)
    ));

    started.push(finished(&call, "contents"));
    assert!(matches!(
        validate(started).expect("finished prefix").recovery.state,
        RunRecoveryState::Incomplete
    ));
}

#[test]
fn rejection_message_must_encode_the_rejection() {
    let call = tool_call();
    let mut transcript = through_assistant(vec![call.clone()]);
    transcript.extend([
        rejected(&call, "truncated_tool_call"),
        tool_message(&call, "invented rejection", true, false),
    ]);

    let error = validate(transcript).expect_err("rejection output must be derived");

    assert!(error.message.contains("contradicts"));
}

#[test]
fn tool_call_limit_rejection_allows_its_real_terminal_path() {
    let call = tool_call();
    let output = format!("tool_call_limit: {REJECTION_MESSAGE}");
    let mut transcript = through_assistant(vec![call.clone()]);
    transcript.extend([
        rejected(&call, "tool_call_limit"),
        tool_message(&call, &output, true, false),
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::LimitExceeded {
                kind: LimitKind::ToolCalls,
            },
        },
    ]);

    let inspection = validate(transcript).expect("tool limit transcript");

    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::LimitExceeded {
                kind: LimitKind::ToolCalls
            }
        }
    ));
}

#[test]
fn cancellation_and_model_output_limit_paths_remain_valid() {
    let cancelled = validate(started(vec![RuntimeEventKind::RunFinished {
        outcome: RunOutcome::Cancelled,
    }]))
    .expect("pre-turn cancellation");
    assert!(matches!(
        cancelled.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Cancelled
        }
    ));

    let limited = validate(started(vec![
        RuntimeEventKind::TurnStarted { turn: 1 },
        RuntimeEventKind::AssistantDelta {
            delta: "partial".into(),
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::LimitExceeded {
                kind: LimitKind::ModelOutput,
            },
        },
    ]))
    .expect("model output limit");
    assert!(matches!(
        limited.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::LimitExceeded {
                kind: LimitKind::ModelOutput
            }
        }
    ));
}

#[test]
fn cancellation_between_tools_resolves_the_unstarted_call_before_terminal() {
    let first = tool_call();
    let mut second = tool_call();
    second.id = "call-2".into();
    let mut transcript = through_assistant(vec![first.clone(), second.clone()]);
    transcript.extend([
        RuntimeEventKind::ToolStarted {
            call: first.clone(),
        },
        finished(&first, "contents"),
        tool_message(&first, "contents", false, false),
        rejected(&second, "cancelled"),
        tool_message(
            &second,
            &format!("cancelled: {REJECTION_MESSAGE}"),
            true,
            false,
        ),
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Cancelled,
        },
    ]);

    let inspection = validate(transcript).expect("between-tool cancellation");

    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Cancelled
        }
    ));
}

#[test]
fn failed_terminal_must_match_runtime_error() {
    let valid = started(vec![
        RuntimeEventKind::RuntimeError {
            code: "workspace_unavailable".into(),
            message: "cannot open workspace".into(),
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Failed {
                code: "workspace_unavailable".into(),
                message: "cannot open workspace".into(),
            },
        },
    ]);
    validate(valid).expect("matching failure transcript");

    let invalid = started(vec![
        RuntimeEventKind::RuntimeError {
            code: "provider_error".into(),
            message: "private detail".into(),
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Failed {
                code: "different".into(),
                message: "private detail".into(),
            },
        },
    ]);
    let error = validate(invalid).expect_err("divergent failure evidence");
    assert!(error.message.contains("preceding runtime_error"));
}

#[test]
fn current_user_and_turn_sequence_are_enforced() {
    let wrong_user = vec![
        RuntimeEventKind::RunStarted {
            prompt: PROMPT.into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "different".into(),
            },
        },
    ];
    assert!(
        validate(wrong_user)
            .expect_err("bound user content")
            .message
            .contains("does not match")
    );

    let nonsequential = started(vec![RuntimeEventKind::TurnStarted { turn: 2 }]);
    assert!(
        validate(nonsequential)
            .expect_err("turn numbering")
            .message
            .contains("not sequential")
    );
}

#[test]
fn cursor_round_trip_continues_the_same_provider_context_transcript() {
    let call = tool_call();
    let kinds = started(vec![
        RuntimeEventKind::TurnStarted { turn: 1 },
        provider_context(),
        assistant("working", vec![call.clone()]),
        RuntimeEventKind::ToolStarted { call: call.clone() },
        finished(&call, "contents"),
        tool_message(&call, "contents", false, false),
        RuntimeEventKind::TurnStarted { turn: 2 },
        assistant("final answer", Vec::new()),
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: "final answer".into(),
            },
        },
    ]);
    let events = events(kinds);
    let mut cursor = RunJournalCursor::new(&record()).expect("cursor");
    for event in &events[..6] {
        cursor.append(event).expect("prefix append");
    }
    let encoded = serde_json::to_string(&cursor).expect("serialize cursor");
    let mut restored: RunJournalCursor = serde_json::from_str(&encoded).expect("restore cursor");
    assert_eq!(restored, cursor);
    restored.validate_run(&record()).expect("cursor binding");
    for event in &events[6..] {
        restored.append(event).expect("continued append");
    }

    assert_eq!(
        restored.recovery(),
        RunInspection::validate(record(), events)
            .expect("full validation")
            .recovery
    );
}

#[test]
fn provider_context_is_rejected_outside_an_open_turn() {
    let error =
        validate(started(vec![provider_context()])).expect_err("provider context needs a turn");

    assert!(error.message.contains("expected a turn"));
}

#[test]
fn rejected_cursor_append_does_not_mutate_the_cursor() {
    let mut cursor = RunJournalCursor::new(&record()).expect("cursor");
    let first = runtime_event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: PROMPT.into(),
        },
    );
    let mut wrong_sequence = first.clone();
    wrong_sequence.seq = 2;
    cursor
        .append(&wrong_sequence)
        .expect_err("sequence mismatch");
    cursor.append(&first).expect("valid retry");
    cursor
        .append(&runtime_event(
            2,
            RuntimeEventKind::MessageCommitted {
                message: Message::User {
                    text: PROMPT.into(),
                },
            },
        ))
        .expect("current user");

    let before_invalid_turn = cursor.clone();
    cursor
        .append(&runtime_event(3, RuntimeEventKind::TurnStarted { turn: 2 }))
        .expect_err("semantic mismatch");
    assert_eq!(cursor, before_invalid_turn);
    cursor
        .append(&runtime_event(3, RuntimeEventKind::TurnStarted { turn: 1 }))
        .expect("valid turn retry");

    assert_eq!(cursor.next_sequence(), 4);
}
