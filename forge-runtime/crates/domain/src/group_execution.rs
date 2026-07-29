use serde::{Deserialize, Serialize};

use crate::{GroupContextStats, GroupRunSnapshot, HubStoreError};

#[path = "group_execution_validation.rs"]
mod validation;

use validation::{
    is_lower_hex_digest, journal_error, valid_identifier, validate_receipt, validate_record,
    validate_status,
};

pub const GROUP_EXECUTION_VERSION: u16 = 1;
pub const GROUP_EXECUTION_PROTOCOL_VERSION: u16 = 1;
pub const MAX_GROUP_EXECUTION_LIST_LIMIT: usize = 100;
pub const MAX_GROUP_EXECUTION_EVENT_JSON_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_EXECUTION_CURSOR_JSON_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_EXECUTION_EVENTS: usize = 3;
pub const MAX_GROUP_EXECUTION_JOURNAL_BYTES: usize = 192 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupExecutionMode {
    OfflineSnapshotValidation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginGroupExecution {
    pub v: u16,
    pub execution_id: String,
    pub group_run_id: String,
    pub mode: GroupExecutionMode,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupExecutionStatus {
    Incomplete,
    Completed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupExecutionRecord {
    pub v: u16,
    pub execution_id: String,
    pub group_run_id: String,
    pub mode: GroupExecutionMode,
    pub status: GroupExecutionStatus,
    pub source_snapshot_sha256: String,
    pub protocol_version: u16,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginGroupExecutionDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginGroupExecutionResult {
    pub v: u16,
    pub disposition: BeginGroupExecutionDisposition,
    pub execution: GroupExecutionRecord,
    pub snapshot: GroupRunSnapshot,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupExecutionReceipt {
    pub v: u16,
    pub execution_id: String,
    pub group_run_id: String,
    pub group_id: String,
    pub context_version: u16,
    pub context_slice_sha256: String,
    pub snapshot_sha256: String,
    pub snapshot_bytes: usize,
    pub stats: GroupContextStats,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupExecutionOutcome {
    SnapshotValidated,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupExecutionEvent {
    pub v: u16,
    pub execution_id: String,
    pub seq: u64,
    #[serde(flatten)]
    pub kind: GroupExecutionEventKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupExecutionEventKind {
    ExecutionStarted {
        group_run_id: String,
        snapshot_sha256: String,
    },
    SnapshotVerified {
        receipt: GroupExecutionReceipt,
    },
    ExecutionFinished {
        outcome: GroupExecutionOutcome,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum GroupExecutionRecovery {
    Incomplete,
    Terminal { outcome: GroupExecutionOutcome },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupExecutionInspection {
    pub v: u16,
    pub execution: GroupExecutionRecord,
    pub events: Vec<GroupExecutionEvent>,
    pub recovery: GroupExecutionRecovery,
    pub receipt: Option<GroupExecutionReceipt>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupExecutionJournalError {
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GroupExecutionJournalCursor {
    v: u16,
    execution_id: String,
    group_run_id: String,
    mode: GroupExecutionMode,
    source_snapshot_sha256: String,
    protocol_version: u16,
    next_sequence: u64,
    state: JournalState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum JournalState {
    NeedStart,
    NeedVerification,
    NeedFinish(GroupExecutionReceipt),
    Terminal(GroupExecutionReceipt),
}

impl GroupExecutionJournalCursor {
    /// Creates an empty cursor bound to one Group Execution.
    ///
    /// # Errors
    ///
    /// Returns an error when record metadata violates the protocol contract.
    pub fn new(record: &GroupExecutionRecord) -> Result<Self, GroupExecutionJournalError> {
        validate_record(record)?;
        Ok(Self {
            v: GROUP_EXECUTION_VERSION,
            execution_id: record.execution_id.clone(),
            group_run_id: record.group_run_id.clone(),
            mode: record.mode,
            source_snapshot_sha256: record.source_snapshot_sha256.clone(),
            protocol_version: record.protocol_version,
            next_sequence: 1,
            state: JournalState::NeedStart,
        })
    }

    /// Applies the next semantically valid event.
    ///
    /// # Errors
    ///
    /// Returns an error without changing the cursor for a gap or bad binding.
    pub fn append(
        &mut self,
        event: &GroupExecutionEvent,
    ) -> Result<(), GroupExecutionJournalError> {
        self.validate_internal()?;
        self.validate_envelope(event)?;
        let next_state = self.next_state(event)?;
        let following = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| journal_error("Group Execution event sequence exhausted"))?;
        self.state = next_state;
        self.next_sequence = following;
        Ok(())
    }

    /// Checks that a restored cursor belongs to the supplied record.
    ///
    /// # Errors
    ///
    /// Returns an error for any identity, source, mode, or version divergence.
    pub fn validate_record(
        &self,
        record: &GroupExecutionRecord,
    ) -> Result<(), GroupExecutionJournalError> {
        validate_record(record)?;
        self.validate_internal()?;
        let matches = self.execution_id == record.execution_id
            && self.group_run_id == record.group_run_id
            && self.mode == record.mode
            && self.source_snapshot_sha256 == record.source_snapshot_sha256
            && self.protocol_version == record.protocol_version;
        if !matches {
            return Err(journal_error(
                "Group Execution cursor does not match its record",
            ));
        }
        validate_status(record.status, &self.recovery())
    }

    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    #[must_use]
    pub fn recovery(&self) -> GroupExecutionRecovery {
        match self.state {
            JournalState::Terminal(_) => GroupExecutionRecovery::Terminal {
                outcome: GroupExecutionOutcome::SnapshotValidated,
            },
            JournalState::NeedStart
            | JournalState::NeedVerification
            | JournalState::NeedFinish(_) => GroupExecutionRecovery::Incomplete,
        }
    }

    #[must_use]
    pub fn receipt(&self) -> Option<&GroupExecutionReceipt> {
        match &self.state {
            JournalState::NeedFinish(receipt) | JournalState::Terminal(receipt) => Some(receipt),
            JournalState::NeedStart | JournalState::NeedVerification => None,
        }
    }

    fn validate_internal(&self) -> Result<(), GroupExecutionJournalError> {
        if self.v != GROUP_EXECUTION_VERSION
            || self.protocol_version != GROUP_EXECUTION_PROTOCOL_VERSION
            || !valid_identifier(&self.execution_id)
            || !valid_identifier(&self.group_run_id)
            || !is_lower_hex_digest(&self.source_snapshot_sha256)
        {
            return Err(journal_error("invalid Group Execution cursor binding"));
        }
        match &self.state {
            JournalState::NeedStart if self.next_sequence == 1 => Ok(()),
            JournalState::NeedVerification if self.next_sequence == 2 => Ok(()),
            JournalState::NeedFinish(receipt) if self.next_sequence == 3 => {
                self.validate_bound_receipt(receipt)
            }
            JournalState::Terminal(receipt) if self.next_sequence == 4 => {
                self.validate_bound_receipt(receipt)
            }
            _ => Err(journal_error(
                "Group Execution cursor state disagrees with its next sequence",
            )),
        }
    }

    fn validate_envelope(
        &self,
        event: &GroupExecutionEvent,
    ) -> Result<(), GroupExecutionJournalError> {
        if event.v != self.protocol_version || event.execution_id != self.execution_id {
            return Err(journal_error(
                "Group Execution event envelope does not match its record",
            ));
        }
        if event.seq != self.next_sequence {
            return Err(journal_error(
                "Group Execution event sequence is not contiguous",
            ));
        }
        Ok(())
    }

    fn next_state(
        &self,
        event: &GroupExecutionEvent,
    ) -> Result<JournalState, GroupExecutionJournalError> {
        match (&self.state, &event.kind) {
            (
                JournalState::NeedStart,
                GroupExecutionEventKind::ExecutionStarted {
                    group_run_id,
                    snapshot_sha256,
                },
            ) => self.start(group_run_id, snapshot_sha256),
            (
                JournalState::NeedVerification,
                GroupExecutionEventKind::SnapshotVerified { receipt },
            ) => self.verify(receipt),
            (
                JournalState::NeedFinish(receipt),
                GroupExecutionEventKind::ExecutionFinished {
                    outcome: GroupExecutionOutcome::SnapshotValidated,
                },
            ) => Ok(JournalState::Terminal(receipt.clone())),
            (JournalState::Terminal(_), _) => {
                Err(journal_error("Group Execution journal is already terminal"))
            }
            _ => Err(journal_error("invalid Group Execution event transition")),
        }
    }

    fn start(
        &self,
        group_run_id: &str,
        snapshot_sha256: &str,
    ) -> Result<JournalState, GroupExecutionJournalError> {
        if group_run_id == self.group_run_id && snapshot_sha256 == self.source_snapshot_sha256 {
            Ok(JournalState::NeedVerification)
        } else {
            Err(journal_error(
                "execution_started does not match the frozen source",
            ))
        }
    }

    fn verify(
        &self,
        receipt: &GroupExecutionReceipt,
    ) -> Result<JournalState, GroupExecutionJournalError> {
        self.validate_bound_receipt(receipt)?;
        Ok(JournalState::NeedFinish(receipt.clone()))
    }

    fn validate_bound_receipt(
        &self,
        receipt: &GroupExecutionReceipt,
    ) -> Result<(), GroupExecutionJournalError> {
        validate_receipt(receipt)?;
        let matches = receipt.execution_id == self.execution_id
            && receipt.group_run_id == self.group_run_id
            && receipt.snapshot_sha256 == self.source_snapshot_sha256;
        if matches {
            Ok(())
        } else {
            Err(journal_error(
                "snapshot_verified receipt does not match the frozen source",
            ))
        }
    }
}

impl GroupExecutionInspection {
    /// Rebuilds and validates a complete durable Group Execution prefix.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt metadata, events, receipt, or status.
    pub fn validate(
        execution: GroupExecutionRecord,
        events: Vec<GroupExecutionEvent>,
    ) -> Result<Self, GroupExecutionJournalError> {
        let mut cursor = GroupExecutionJournalCursor::new(&execution)?;
        for event in &events {
            cursor.append(event)?;
        }
        validate_status(execution.status, &cursor.recovery())?;
        Ok(Self {
            v: GROUP_EXECUTION_VERSION,
            execution,
            events,
            recovery: cursor.recovery(),
            receipt: cursor.receipt().cloned(),
        })
    }
}

pub trait GroupExecutionStore: Send + Sync {
    /// Atomically binds one execution to a fully verified frozen snapshot.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, not-found, or storage error.
    fn begin_group_execution(
        &self,
        request: &BeginGroupExecution,
    ) -> Result<BeginGroupExecutionResult, HubStoreError>;

    /// Appends one contiguous event, accepting only an exact committed replay.
    ///
    /// # Errors
    ///
    /// Returns an error when the event violates the journal or cannot commit.
    fn append_group_execution_event(
        &self,
        event: &GroupExecutionEvent,
    ) -> Result<(), HubStoreError>;

    /// Loads and validates one complete durable execution prefix.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing, corrupt, or unavailable execution.
    fn inspect_group_execution(
        &self,
        execution_id: &str,
    ) -> Result<GroupExecutionInspection, HubStoreError>;

    /// Lists bounded execution metadata, optionally for one frozen Group Run.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid filter/limit, corruption, or storage.
    fn list_group_executions(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupExecutionRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupExecutionJournalError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupExecutionJournalError {}
