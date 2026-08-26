use std::collections::VecDeque;

use crate::{Message, RunOutcome, RunToolContinuation, RuntimeEventKind, ToolCall};

use super::super::{RunJournalError, journal_error};
use super::Phase;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(super) struct CallSequence {
    pub(super) remaining: VecDeque<ToolCall>,
    mode: CallMode,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(super) struct StartedCall {
    pub(super) call: ToolCall,
    sequence: CallSequence,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(super) struct ToolMessageExpectation {
    pub(super) message: ExpectedToolMessage,
    pub(super) sequence: CallSequence,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(super) enum ExpectedToolMessage {
    Finished(ToolCall, String, bool, bool),
    Rejected(ToolCall, String),
}

impl ExpectedToolMessage {
    pub(super) fn as_message(&self) -> Message {
        match self {
            Self::Finished(call, output, is_error, truncated) => Message::Tool {
                call_id: call.id.clone(),
                name: call.name.clone(),
                output: output.clone(),
                is_error: *is_error,
                truncated: *truncated,
            },
            Self::Rejected(call, output) => Message::Tool {
                call_id: call.id.clone(),
                name: call.name.clone(),
                output: output.clone(),
                is_error: true,
                truncated: false,
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
enum CallMode {
    #[default]
    Undecided,
    Executing,
    Rejecting(String, String),
}

pub(super) fn commit_assistant(text: &str, calls: &[ToolCall]) -> Result<Phase, RunJournalError> {
    validate_tool_calls(calls)?;
    if calls.is_empty() {
        return Ok(Phase::Answer(text.into()));
    }
    Ok(Phase::Calls(CallSequence {
        remaining: calls.iter().cloned().collect(),
        mode: CallMode::Undecided,
    }))
}

fn validate_tool_calls(calls: &[ToolCall]) -> Result<(), RunJournalError> {
    let mut ids = std::collections::BTreeSet::new();
    for call in calls {
        if call.id.trim().is_empty() || call.name.trim().is_empty() {
            return Err(journal_error(
                "assistant committed a tool call with an empty id or name",
            ));
        }
        if !ids.insert(call.id.as_str()) {
            return Err(journal_error(
                "assistant committed duplicate tool call identifiers",
            ));
        }
    }
    Ok(())
}

pub(super) fn resolve_call(
    mut sequence: CallSequence,
    event: &RuntimeEventKind,
) -> Result<Phase, RunJournalError> {
    let expected = sequence
        .remaining
        .pop_front()
        .ok_or_else(|| journal_error("tool resolution has no unresolved call"))?;
    match event {
        RuntimeEventKind::ToolStarted { call } if call == &expected => {
            sequence.mode = started_mode(&sequence.mode)?;
            Ok(Phase::Started(StartedCall {
                call: expected,
                sequence,
            }))
        }
        RuntimeEventKind::ToolRejected {
            call,
            code,
            message,
        } if call == &expected => reject_call(sequence, expected, code, message),
        RuntimeEventKind::ToolStarted { .. } | RuntimeEventKind::ToolRejected { .. } => Err(
            journal_error("tool lifecycle event does not match the next unresolved call"),
        ),
        _ => Err(unresolved_error()),
    }
}

fn started_mode(mode: &CallMode) -> Result<CallMode, RunJournalError> {
    match mode {
        CallMode::Undecided | CallMode::Executing => Ok(CallMode::Executing),
        CallMode::Rejecting(..) => Err(journal_error(
            "tool start cannot follow batch tool rejection",
        )),
    }
}

fn reject_call(
    mut sequence: CallSequence,
    call: ToolCall,
    code: &str,
    message: &str,
) -> Result<Phase, RunJournalError> {
    sequence.mode = rejected_mode(sequence.mode, code, message)?;
    let output = format!("{code}: {message}");
    Ok(Phase::ToolMessage(ToolMessageExpectation {
        message: ExpectedToolMessage::Rejected(call, output),
        sequence,
    }))
}

fn rejected_mode(mode: CallMode, code: &str, message: &str) -> Result<CallMode, RunJournalError> {
    match mode {
        CallMode::Undecided => Ok(rejecting(code, message)),
        CallMode::Executing if code == "cancelled" => Ok(rejecting(code, message)),
        CallMode::Executing => Err(journal_error(
            "only cancellation may reject a partially executed tool batch",
        )),
        CallMode::Rejecting(expected_code, expected_message)
            if expected_code == code && expected_message == message =>
        {
            Ok(CallMode::Rejecting(expected_code, expected_message))
        }
        CallMode::Rejecting(..) => Err(journal_error(
            "one rejected tool batch must use one code and message",
        )),
    }
}

fn rejecting(code: &str, message: &str) -> CallMode {
    CallMode::Rejecting(code.into(), message.into())
}

pub(super) fn finish_started_call(
    started: StartedCall,
    event: &RuntimeEventKind,
) -> Result<Phase, RunJournalError> {
    let RuntimeEventKind::ToolFinished {
        call_id,
        name,
        output,
        is_error,
        truncated,
    } = event
    else {
        return Err(journal_error(
            "started tool effect must be followed by matching tool_finished",
        ));
    };
    if call_id != &started.call.id || name != &started.call.name {
        return Err(journal_error(
            "tool_finished does not match the started tool call",
        ));
    }
    Ok(Phase::ToolMessage(ToolMessageExpectation {
        message: ExpectedToolMessage::Finished(started.call, output.clone(), *is_error, *truncated),
        sequence: started.sequence,
    }))
}

pub(super) fn commit_tool_message(
    expected: ToolMessageExpectation,
    event: &RuntimeEventKind,
) -> Result<Phase, RunJournalError> {
    let RuntimeEventKind::MessageCommitted {
        message:
            Message::Tool {
                call_id,
                name,
                output,
                is_error,
                truncated,
            },
    } = event
    else {
        return Err(journal_error(
            "tool resolution must be followed by its committed tool message",
        ));
    };
    validate_tool_message(
        expected.message,
        call_id,
        name,
        output,
        *is_error,
        *truncated,
    )?;
    Ok(advance_calls(expected.sequence))
}

fn validate_tool_message(
    expected: ExpectedToolMessage,
    call_id: &str,
    name: &str,
    output: &str,
    is_error: bool,
    truncated: bool,
) -> Result<(), RunJournalError> {
    let valid = match expected {
        ExpectedToolMessage::Finished(call, expected, expected_error, expected_truncated) => {
            call.id == call_id
                && call.name == name
                && expected == output
                && expected_error == is_error
                && expected_truncated == truncated
        }
        ExpectedToolMessage::Rejected(call, expected) => {
            call.id == call_id
                && call.name == name
                && is_error
                && valid_rejected_output(&expected, output, truncated)
        }
    };
    valid
        .then_some(())
        .ok_or_else(|| journal_error("committed tool message contradicts its lifecycle event"))
}

fn valid_rejected_output(expected: &str, actual: &str, truncated: bool) -> bool {
    if truncated {
        actual.len() < expected.len() && expected.starts_with(actual)
    } else {
        actual == expected
    }
}

fn advance_calls(sequence: CallSequence) -> Phase {
    if !sequence.remaining.is_empty() {
        return Phase::Calls(sequence);
    }
    match sequence.mode {
        CallMode::Rejecting(code, _) if code == "tool_call_limit" => {
            Phase::ExpectedTerminal(RunOutcome::LimitExceeded {
                kind: crate::LimitKind::ToolCalls,
            })
        }
        CallMode::Rejecting(code, _) if code == "cancelled" => {
            Phase::ExpectedTerminal(RunOutcome::Cancelled)
        }
        CallMode::Undecided | CallMode::Executing | CallMode::Rejecting(..) => Phase::ReadyForTurn,
    }
}

pub(super) fn tool_continuation(sequence: &CallSequence, next_turn: u32) -> RunToolContinuation {
    if !sequence.remaining.is_empty() {
        let calls = sequence.remaining.iter().cloned().collect();
        return match &sequence.mode {
            CallMode::Rejecting(code, message) => RunToolContinuation::RejectTools {
                calls,
                code: code.clone(),
                message: message.clone(),
            },
            CallMode::Undecided | CallMode::Executing => {
                RunToolContinuation::ExecuteTools { calls }
            }
        };
    }
    match &sequence.mode {
        CallMode::Rejecting(code, _) if code == "tool_call_limit" => RunToolContinuation::Finish {
            outcome: RunOutcome::LimitExceeded {
                kind: crate::LimitKind::ToolCalls,
            },
        },
        CallMode::Rejecting(code, _) if code == "cancelled" => RunToolContinuation::Finish {
            outcome: RunOutcome::Cancelled,
        },
        CallMode::Undecided | CallMode::Executing | CallMode::Rejecting(..) => {
            RunToolContinuation::StartTurn { turn: next_turn }
        }
    }
}

fn unresolved_error() -> RunJournalError {
    journal_error("unresolved tool calls must resolve before the next turn or terminal")
}
