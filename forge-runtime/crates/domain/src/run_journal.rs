use crate::{
    Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunOutcome, RunRecord, RuntimeEvent, ToolCall,
};

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

/// The only safe continuation points for an explicit Run resume.
///
/// This is derived from the already validated journal, never from a caller
/// supplied cursor.  In particular, a `PendingTool` point is intentionally
/// exposed as a refusal point: a durable `tool_started` event means the
/// external effect may already have happened, so an automatic replay must not
/// execute that tool again.
#[derive(Clone, Debug, PartialEq)]
pub enum RunResumePoint {
    Start,
    CommitUser {
        prompt: String,
    },
    StartTurn {
        turn: u32,
    },
    ContinueTurn {
        turn: u32,
    },
    ExecuteTools {
        calls: Vec<ToolCall>,
    },
    RejectTools {
        calls: Vec<ToolCall>,
        code: String,
        message: String,
    },
    CommitToolMessage {
        message: Message,
        continuation: RunToolContinuation,
    },
    Finish {
        outcome: RunOutcome,
    },
    PendingTool {
        call: ToolCall,
    },
}

/// The validated action that follows a repaired Tool message.
#[derive(Clone, Debug, PartialEq)]
pub enum RunToolContinuation {
    ExecuteTools {
        calls: Vec<ToolCall>,
    },
    RejectTools {
        calls: Vec<ToolCall>,
        code: String,
        message: String,
    },
    StartTurn {
        turn: u32,
    },
    Finish {
        outcome: RunOutcome,
    },
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

    /// Returns the validated point at which an explicit resume may continue.
    #[must_use]
    pub fn resume_point(&self) -> RunResumePoint {
        self.state.resume_point()
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

    /// Derives the explicit-resume continuation from this validated journal.
    ///
    /// # Errors
    ///
    /// Returns an error only if the inspection was constructed outside
    /// [`RunInspection::validate`] with an invalid event prefix.
    pub fn resume_point(&self) -> Result<RunResumePoint, RunJournalError> {
        let mut cursor = RunJournalCursor::new(&self.run)?;
        for event in &self.events {
            cursor.append(event)?;
        }
        Ok(cursor.resume_point())
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
#[path = "run_journal_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "run_journal_resume_rejection_tests.rs"]
mod resume_rejection_tests;

#[cfg(test)]
mod tests {
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
}
