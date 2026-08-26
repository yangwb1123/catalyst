use crate::{LimitKind, Message, RunOutcome, RunToolContinuation, RuntimeEventKind, ToolCall};

use super::{
    RunInspection, RunResumePoint,
    test_support::{assistant_event, event, record, tool_call, user_event},
};

#[test]
fn resume_point_preserves_a_partially_rejected_batch() {
    let first = tool_call();
    let second = ToolCall {
        id: "call-2".into(),
        ..tool_call()
    };
    let inspection = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, vec![first.clone(), second.clone()]),
            event(
                5,
                RuntimeEventKind::ToolRejected {
                    call: first.clone(),
                    code: "cancelled".into(),
                    message: "batch cancelled".into(),
                },
            ),
            event(
                6,
                RuntimeEventKind::MessageCommitted {
                    message: Message::Tool {
                        call_id: first.id,
                        name: first.name,
                        output: "cancelled: batch cancelled".into(),
                        is_error: true,
                        truncated: false,
                    },
                },
            ),
        ],
    )
    .expect("valid partial rejection prefix");

    assert_eq!(
        inspection.resume_point().expect("resume point"),
        RunResumePoint::RejectTools {
            calls: vec![second],
            code: "cancelled".into(),
            message: "batch cancelled".into(),
        }
    );
}

#[test]
fn resume_point_finishes_after_repairing_a_limit_rejection_message() {
    let call = tool_call();
    let inspection = RunInspection::validate(
        record(),
        vec![
            event(1, RuntimeEventKind::RunStarted { prompt: "p".into() }),
            user_event(2),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            assistant_event(4, vec![call.clone()]),
            event(
                5,
                RuntimeEventKind::ToolRejected {
                    call: call.clone(),
                    code: "tool_call_limit".into(),
                    message: "tool call was not executed".into(),
                },
            ),
        ],
    )
    .expect("valid limit rejection prefix");

    assert_eq!(
        inspection.resume_point().expect("resume point"),
        RunResumePoint::CommitToolMessage {
            message: Message::Tool {
                call_id: call.id,
                name: call.name,
                output: "tool_call_limit: tool call was not executed".into(),
                is_error: true,
                truncated: false,
            },
            continuation: RunToolContinuation::Finish {
                outcome: RunOutcome::LimitExceeded {
                    kind: LimitKind::ToolCalls,
                },
            },
        }
    );
}
