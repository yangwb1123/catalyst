use crate::{Message, RunOutcome, RunRecoveryState, RunToolContinuation, RuntimeEventKind};

use super::{
    RunInspection, RunResumePoint,
    test_support::{assistant_event, event, record, tool_call, user_event},
};

#[test]
fn resume_point_executes_only_unstarted_calls() {
    let inspection = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, vec![tool_call()]),
        ],
    )
    .expect("valid tool-call prefix");

    assert_eq!(
        inspection.resume_point().expect("resume point"),
        RunResumePoint::ExecuteTools {
            calls: vec![tool_call()]
        }
    );
}

#[test]
fn unmatched_tool_start_is_recovery_blocked() {
    let inspection = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, vec![tool_call()]),
            event(5, RuntimeEventKind::ToolStarted { call: tool_call() }),
        ],
    )
    .expect("valid incomplete prefix");

    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::PendingTool { ref calls } if calls == &[tool_call()]
    ));
    assert_eq!(
        inspection.resume_point().expect("resume point"),
        RunResumePoint::PendingTool { call: tool_call() }
    );
}

#[test]
fn resume_point_commits_a_missing_tool_message_without_replaying_the_effect() {
    let inspection = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, vec![tool_call()]),
            event(5, RuntimeEventKind::ToolStarted { call: tool_call() }),
            event(
                6,
                RuntimeEventKind::ToolFinished {
                    call_id: "call-1".into(),
                    name: "read_file".into(),
                    output: "result".into(),
                    is_error: false,
                    truncated: false,
                    execution_evidence: None,
                },
            ),
        ],
    )
    .expect("valid completed-effect prefix");

    assert_eq!(
        inspection.resume_point().expect("resume point"),
        RunResumePoint::CommitToolMessage {
            message: Message::Tool {
                call_id: "call-1".into(),
                name: "read_file".into(),
                output: "result".into(),
                is_error: false,
                truncated: false,
            },
            continuation: RunToolContinuation::StartTurn { turn: 2 },
        }
    );
}

#[test]
fn resume_point_finishes_a_committed_answer_without_calling_the_provider() {
    let inspection = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, Vec::new()),
        ],
    )
    .expect("valid answer prefix");

    assert_eq!(
        inspection.resume_point().expect("resume point"),
        RunResumePoint::Finish {
            outcome: RunOutcome::Completed {
                answer: "done".into()
            }
        }
    );
}

#[test]
fn terminal_with_pending_tool_is_rejected() {
    let error = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, vec![tool_call()]),
            event(5, RuntimeEventKind::ToolStarted { call: tool_call() }),
            event(
                6,
                RuntimeEventKind::RunFinished {
                    outcome: RunOutcome::Cancelled,
                },
            ),
        ],
    )
    .expect_err("pending tool blocks terminal");

    assert!(error.message.contains("started tool effect"));
}

#[test]
fn first_tool_start_is_rejected() {
    let error = RunInspection::validate(
        record(),
        vec![event(
            1,
            RuntimeEventKind::ToolStarted { call: tool_call() },
        )],
    )
    .expect_err("a tool start cannot be the first event");

    assert!(error.message.contains("first event"));
}

#[test]
fn first_run_finished_is_rejected() {
    let error = RunInspection::validate(
        record(),
        vec![event(
            1,
            RuntimeEventKind::RunFinished {
                outcome: RunOutcome::Cancelled,
            },
        )],
    )
    .expect_err("a terminal event cannot be the first event");

    assert!(error.message.contains("first event"));
}

#[test]
fn completed_answer_requires_a_matching_assistant_message() {
    let error = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, Vec::new()),
            event(
                5,
                RuntimeEventKind::RunFinished {
                    outcome: RunOutcome::Completed {
                        answer: "uncommitted".into(),
                    },
                },
            ),
        ],
    )
    .expect_err("an uncommitted answer is not durable evidence");

    assert!(error.message.contains("completed answer"));
}

#[test]
fn completed_answer_accepts_the_matching_assistant_message() {
    let inspection = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, Vec::new()),
            event(
                5,
                RuntimeEventKind::RunFinished {
                    outcome: RunOutcome::Completed {
                        answer: "done".into(),
                    },
                },
            ),
        ],
    )
    .expect("matching durable assistant answer");

    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { ref answer }
        } if answer == "done"
    ));
}
