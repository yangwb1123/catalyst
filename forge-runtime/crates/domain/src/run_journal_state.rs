use crate::{
    Message, RUN_STORE_VERSION, RunOutcome, RunResumePoint, RunToolContinuation, RuntimeEvent,
    RuntimeEventKind,
};

use super::{RunJournalError, RunRecovery, RunRecoveryState, journal_error};

#[path = "run_journal_tools.rs"]
mod tools;

use tools::{
    CallSequence, StartedCall, ToolMessageExpectation, commit_assistant, commit_tool_message,
    finish_started_call, resolve_call, tool_continuation,
};

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

    pub(super) fn resume_point(&self) -> RunResumePoint {
        match &self.phase {
            Phase::NeedStart => RunResumePoint::Start,
            Phase::NeedUser(prompt) => RunResumePoint::CommitUser {
                prompt: prompt.clone(),
            },
            Phase::ReadyForTurn => RunResumePoint::StartTurn {
                turn: self.last_turn.saturating_add(1),
            },
            Phase::InTurn => RunResumePoint::ContinueTurn {
                turn: self.last_turn,
            },
            Phase::Calls(sequence) => resume_tool_continuation(tool_continuation(
                sequence,
                self.last_turn.saturating_add(1),
            )),
            Phase::Started(started) => RunResumePoint::PendingTool {
                call: started.call.clone(),
            },
            Phase::ToolMessage(expected) => RunResumePoint::CommitToolMessage {
                message: expected.message.as_message(),
                continuation: tool_continuation(
                    &expected.sequence,
                    self.last_turn.saturating_add(1),
                ),
            },
            Phase::Answer(answer) => RunResumePoint::Finish {
                outcome: RunOutcome::Completed {
                    answer: answer.clone(),
                },
            },
            Phase::Failed(code, message) => RunResumePoint::Finish {
                outcome: RunOutcome::Failed {
                    code: code.clone(),
                    message: message.clone(),
                },
            },
            Phase::ExpectedTerminal(outcome) | Phase::Terminal(outcome) => RunResumePoint::Finish {
                outcome: outcome.clone(),
            },
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

fn resume_tool_continuation(continuation: RunToolContinuation) -> RunResumePoint {
    match continuation {
        RunToolContinuation::ExecuteTools { calls } => RunResumePoint::ExecuteTools { calls },
        RunToolContinuation::RejectTools {
            calls,
            code,
            message,
        } => RunResumePoint::RejectTools {
            calls,
            code,
            message,
        },
        RunToolContinuation::StartTurn { turn } => RunResumePoint::StartTurn { turn },
        RunToolContinuation::Finish { outcome } => RunResumePoint::Finish { outcome },
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

fn unexpected(expected: &str) -> RunJournalError {
    journal_error(&format!("invalid Run transcript: {expected}"))
}
