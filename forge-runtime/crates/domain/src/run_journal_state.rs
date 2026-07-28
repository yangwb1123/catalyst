use std::collections::VecDeque;

use crate::{Message, RUN_STORE_VERSION, RunOutcome, RuntimeEvent, RuntimeEventKind, ToolCall};

use super::{RunJournalError, RunRecovery, RunRecoveryState, journal_error};

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
pub(super) struct JournalState {
    phase: Phase,
    last_turn: u32,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
enum Phase {
    #[default]
    NeedStart,
    NeedUser(String),
    ReadyForTurn,
    InTurn,
    Calls(CallSequence),
    Started(StartedCall),
    ToolMessage(ToolMessageExpectation),
    Answer(String),
    Failed(String, String),
    ExpectedTerminal(RunOutcome),
    Terminal(RunOutcome),
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct CallSequence {
    remaining: VecDeque<ToolCall>,
    mode: CallMode,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct StartedCall {
    call: ToolCall,
    sequence: CallSequence,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
struct ToolMessageExpectation {
    message: ExpectedToolMessage,
    sequence: CallSequence,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
enum ExpectedToolMessage {
    Finished(ToolCall, String, bool, bool),
    Rejected(ToolCall, String),
}

#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize, serde::Serialize)]
enum CallMode {
    #[default]
    Undecided,
    Executing,
    Rejecting(String, String),
}

impl JournalState {
    pub(super) fn apply(
        &mut self,
        event: &RuntimeEvent,
        is_first: bool,
    ) -> Result<(), RunJournalError> {
        if is_first && !matches!(event.kind, RuntimeEventKind::RunStarted { .. }) {
            return Err(journal_error("first event must be run_started"));
        }
        let phase = self.phase.clone();
        self.phase = self.transition(phase, &event.kind)?;
        Ok(())
    }

    pub(super) fn recovery(&self) -> RunRecovery {
        let state = match &self.phase {
            Phase::Terminal(outcome) => RunRecoveryState::Terminal {
                outcome: outcome.clone(),
            },
            Phase::Started(started) => RunRecoveryState::PendingTool {
                calls: vec![started.call.clone()],
            },
            _ => RunRecoveryState::Incomplete,
        };
        RunRecovery {
            v: RUN_STORE_VERSION,
            state,
        }
    }

    fn transition(
        &mut self,
        phase: Phase,
        event: &RuntimeEventKind,
    ) -> Result<Phase, RunJournalError> {
        if matches!(phase, Phase::Terminal(_)) {
            return Err(journal_error("event appears after run_finished"));
        }
        if matches!(event, RuntimeEventKind::RunStarted { .. })
            && !matches!(phase, Phase::NeedStart)
        {
            return Err(journal_error("duplicate run_started"));
        }
        match phase {
            Phase::NeedStart => start_run(event),
            Phase::NeedUser(prompt) => commit_current_user(&prompt, event),
            Phase::ReadyForTurn => self.ready_event(event),
            Phase::InTurn => commit_turn_event(event),
            Phase::Calls(sequence) => resolve_call(sequence, event),
            Phase::Started(started) => finish_started_call(started, event),
            Phase::ToolMessage(expected) => commit_tool_message(expected, event),
            Phase::Answer(answer) => finish_answer(&answer, event),
            Phase::Failed(code, message) => finish_failure(&code, &message, event),
            Phase::ExpectedTerminal(outcome) => finish_expected(&outcome, event),
            Phase::Terminal(_) => unreachable!("terminal handled above"),
        }
    }

    fn ready_event(&mut self, event: &RuntimeEventKind) -> Result<Phase, RunJournalError> {
        match event {
            RuntimeEventKind::TurnStarted { turn } => {
                let expected = self.last_turn.checked_add(1).ok_or_else(|| {
                    journal_error("turn_started appears after the maximum turn number")
                })?;
                if *turn != expected {
                    return Err(journal_error("turn_started number is not sequential"));
                }
                self.last_turn = *turn;
                Ok(Phase::InTurn)
            }
            RuntimeEventKind::RuntimeError { code, message } => {
                Ok(failed(code.clone(), message.clone()))
            }
            RuntimeEventKind::RunFinished { outcome }
                if matches!(
                    outcome,
                    RunOutcome::Cancelled
                        | RunOutcome::LimitExceeded {
                            kind: crate::LimitKind::Turns
                        }
                ) =>
            {
                Ok(Phase::Terminal(outcome.clone()))
            }
            _ => Err(unexpected("expected a turn or pre-turn terminal outcome")),
        }
    }
}

fn start_run(event: &RuntimeEventKind) -> Result<Phase, RunJournalError> {
    match event {
        RuntimeEventKind::RunStarted { prompt } => Ok(Phase::NeedUser(prompt.clone())),
        _ => Err(journal_error("first event must be run_started")),
    }
}

fn commit_current_user(prompt: &str, event: &RuntimeEventKind) -> Result<Phase, RunJournalError> {
    match event {
        RuntimeEventKind::MessageCommitted {
            message: Message::User { text },
        } if text == prompt => Ok(Phase::ReadyForTurn),
        RuntimeEventKind::MessageCommitted {
            message: Message::User { .. },
        } => Err(journal_error(
            "current user message does not match run_started prompt",
        )),
        _ => Err(unexpected(
            "run_started must be followed by the matching current user message",
        )),
    }
}

fn commit_turn_event(event: &RuntimeEventKind) -> Result<Phase, RunJournalError> {
    match event {
        RuntimeEventKind::AssistantDelta { .. }
        | RuntimeEventKind::MessageCommitted {
            message: Message::ProviderContext { .. },
        } => Ok(Phase::InTurn),
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant { text, tool_calls },
        } => commit_assistant(text, tool_calls),
        RuntimeEventKind::RuntimeError { code, message } => {
            Ok(failed(code.clone(), message.clone()))
        }
        RuntimeEventKind::RunFinished { outcome }
            if matches!(
                outcome,
                RunOutcome::Cancelled
                    | RunOutcome::LimitExceeded {
                        kind: crate::LimitKind::ModelOutput
                    }
            ) =>
        {
            Ok(Phase::Terminal(outcome.clone()))
        }
        _ => Err(unexpected(
            "turn accepts only assistant output or an in-turn terminal outcome",
        )),
    }
}

fn commit_assistant(text: &str, calls: &[ToolCall]) -> Result<Phase, RunJournalError> {
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

fn resolve_call(
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

fn finish_started_call(
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

fn commit_tool_message(
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

fn finish_answer(answer: &str, event: &RuntimeEventKind) -> Result<Phase, RunJournalError> {
    match event {
        RuntimeEventKind::RunFinished {
            outcome: outcome @ RunOutcome::Completed { answer: committed },
        } if !answer.trim().is_empty() && committed == answer => {
            Ok(Phase::Terminal(outcome.clone()))
        }
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed { .. },
        } => Err(journal_error(
            "completed answer must match a non-empty assistant message without tool calls",
        )),
        RuntimeEventKind::RuntimeError { code, message } => {
            Ok(failed(code.clone(), message.clone()))
        }
        _ => Err(unexpected(
            "assistant answer must be followed by completed or failed terminal evidence",
        )),
    }
}

fn failed(code: String, message: String) -> Phase {
    Phase::Failed(code, message)
}

fn finish_failure(
    expected_code: &str,
    expected_message: &str,
    event: &RuntimeEventKind,
) -> Result<Phase, RunJournalError> {
    match event {
        RuntimeEventKind::RunFinished {
            outcome: outcome @ RunOutcome::Failed { code, message },
        } if code == expected_code && message == expected_message => {
            Ok(Phase::Terminal(outcome.clone()))
        }
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Failed { .. },
        } => Err(journal_error(
            "failed terminal must match the preceding runtime_error",
        )),
        _ => Err(unexpected(
            "runtime_error must be followed by matching failed terminal evidence",
        )),
    }
}

fn finish_expected(
    expected: &RunOutcome,
    event: &RuntimeEventKind,
) -> Result<Phase, RunJournalError> {
    match event {
        RuntimeEventKind::RunFinished { outcome } if outcome == expected => {
            Ok(Phase::Terminal(outcome.clone()))
        }
        RuntimeEventKind::RunFinished { .. } => Err(journal_error(
            "terminal outcome contradicts tool resolution",
        )),
        _ => Err(unexpected(
            "resolved tool batch requires its terminal outcome",
        )),
    }
}

fn unresolved_error() -> RunJournalError {
    journal_error("unresolved tool calls must resolve before the next turn or terminal")
}

fn unexpected(expected: &str) -> RunJournalError {
    journal_error(&format!("invalid Run transcript: {expected}"))
}
