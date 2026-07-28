use crate::{PROTOCOL_VERSION, RUN_STORE_VERSION, RunOutcome, RunRecord, RuntimeEvent, ToolCall};

#[path = "run_journal_state.rs"]
mod state;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RunInspection {
    pub v: u16,
    pub run: RunRecord,
    pub events: Vec<RuntimeEvent>,
    pub recovery: RunRecovery,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RunRecovery {
    pub v: u16,
    #[serde(flatten)]
    pub state: RunRecoveryState,
}

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RunRecoveryState {
    Terminal { outcome: RunOutcome },
    Incomplete,
    PendingTool { calls: Vec<ToolCall> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunJournalError {
    pub message: String,
}

/// Serializable incremental validator for one durable Run journal.
#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct RunJournalCursor {
    v: u16,
    run_id: String,
    conversation_id: String,
    protocol_version: u16,
    next_sequence: u64,
    state: state::JournalState,
}

impl RunJournalCursor {
    /// Creates an empty cursor bound to one Run record.
    ///
    /// # Errors
    ///
    /// Returns an error when the Run record version is unsupported.
    pub fn new(run: &RunRecord) -> Result<Self, RunJournalError> {
        validate_record(run)?;
        Ok(Self {
            v: RUN_STORE_VERSION,
            run_id: run.run_id.clone(),
            conversation_id: run.conversation_id.clone(),
            protocol_version: run.protocol_version,
            next_sequence: 1,
            state: state::JournalState::default(),
        })
    }

    /// Validates and applies the next event without rescanning prior events.
    ///
    /// The cursor is unchanged when validation fails.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid cursor, envelope, sequence, or transcript
    /// transition.
    pub fn append(&mut self, event: &RuntimeEvent) -> Result<(), RunJournalError> {
        self.validate_internal()?;
        self.validate_envelope(event)?;
        let following = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| journal_error("runtime event sequence exhausted"))?;
        self.state.apply(event, self.next_sequence == 1)?;
        self.next_sequence = following;
        Ok(())
    }

    /// Verifies that a restored cursor belongs to the supplied Run record.
    ///
    /// # Errors
    ///
    /// Returns an error when identity or protocol fields diverge.
    pub fn validate_run(&self, run: &RunRecord) -> Result<(), RunJournalError> {
        validate_record(run)?;
        self.validate_internal()?;
        if self.run_id != run.run_id
            || self.conversation_id != run.conversation_id
            || self.protocol_version != run.protocol_version
        {
            return Err(journal_error("Run journal cursor does not match Run"));
        }
        Ok(())
    }

    /// Returns the sequence number required by the next append.
    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    /// Derives terminal or interrupted recovery state from the current prefix.
    #[must_use]
    pub fn recovery(&self) -> RunRecovery {
        self.state.recovery()
    }

    fn validate_internal(&self) -> Result<(), RunJournalError> {
        if self.v != RUN_STORE_VERSION
            || self.protocol_version != PROTOCOL_VERSION
            || self.next_sequence == 0
        {
            return Err(journal_error("invalid Run journal cursor"));
        }
        Ok(())
    }

    fn validate_envelope(&self, event: &RuntimeEvent) -> Result<(), RunJournalError> {
        if event.v != self.protocol_version {
            return Err(journal_error("runtime event version does not match Run"));
        }
        if event.run_id != self.run_id || event.session_id != self.conversation_id {
            return Err(journal_error("runtime event envelope does not match Run"));
        }
        if event.seq != self.next_sequence {
            return Err(journal_error("runtime event sequence is not contiguous"));
        }
        Ok(())
    }
}

impl RunInspection {
    /// Validates a durable Run prefix and derives its recovery state.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported version, a divergent envelope,
    /// sequence gaps, invalid tool lifecycle, or events after termination.
    pub fn validate(run: RunRecord, events: Vec<RuntimeEvent>) -> Result<Self, RunJournalError> {
        let mut cursor = RunJournalCursor::new(&run)?;
        for event in &events {
            cursor.append(event)?;
        }
        Ok(Self {
            v: RUN_STORE_VERSION,
            run,
            events,
            recovery: cursor.recovery(),
        })
    }
}

impl std::fmt::Display for RunJournalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RunJournalError {}

fn validate_record(run: &RunRecord) -> Result<(), RunJournalError> {
    if run.v != RUN_STORE_VERSION {
        return Err(journal_error("unsupported run-store record version"));
    }
    if run.protocol_version != PROTOCOL_VERSION {
        return Err(journal_error("unsupported runtime event protocol version"));
    }
    Ok(())
}

fn journal_error(message: &str) -> RunJournalError {
    RunJournalError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits, RunOutcome,
        RunProvider, RunRecord, RunRecoveryState, RuntimeEvent, RuntimeEventKind, ToolCall,
    };
    use serde_json::json;

    use super::RunInspection;

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

    fn record() -> RunRecord {
        RunRecord {
            v: RUN_STORE_VERSION,
            run_id: "run-1".into(),
            conversation_id: "conversation-1".into(),
            prompt_id: "prompt-1".into(),
            project_id: "project-1".into(),
            execution: RunExecution {
                provider: RunProvider::DeterministicRead {
                    path: "README.md".into(),
                },
                system_prompt: "answer".into(),
                allowed_read_paths: vec!["README.md".into()],
                limits: RunLimits::default(),
            },
            protocol_version: PROTOCOL_VERSION,
            created_at_ms: 1,
        }
    }

    fn event(seq: u64, kind: RuntimeEventKind) -> RuntimeEvent {
        RuntimeEvent {
            v: PROTOCOL_VERSION,
            session_id: "conversation-1".into(),
            run_id: "run-1".into(),
            seq,
            emitted_at_ms: seq,
            kind,
        }
    }

    fn user_event(seq: u64) -> RuntimeEvent {
        event(
            seq,
            RuntimeEventKind::MessageCommitted {
                message: Message::User { text: "p".into() },
            },
        )
    }

    fn assistant_event(seq: u64, tool_calls: Vec<ToolCall>) -> RuntimeEvent {
        event(
            seq,
            RuntimeEventKind::MessageCommitted {
                message: Message::Assistant {
                    text: "done".into(),
                    tool_calls,
                },
            },
        )
    }

    fn tool_call() -> ToolCall {
        ToolCall {
            id: "call-1".into(),
            name: "read_file".into(),
            arguments: json!({"path": "README.md"}),
        }
    }
}
