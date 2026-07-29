use std::sync::Arc;

use crate::{
    HubError, HubField, MAX_ENTITY_ID_BYTES, MAX_IDEMPOTENCY_KEY_BYTES,
    runtime_domain::{
        BeginGroupExecution, BeginGroupExecutionDisposition, BeginGroupExecutionResult,
        GROUP_EXECUTION_PROTOCOL_VERSION, GROUP_EXECUTION_VERSION, GROUP_RUN_VERSION,
        GroupExecutionEvent, GroupExecutionEventKind, GroupExecutionInspection,
        GroupExecutionJournalCursor, GroupExecutionOutcome, GroupExecutionReceipt,
        GroupExecutionRecord, GroupExecutionRecovery, GroupExecutionStatus, GroupExecutionStore,
        GroupRunStatus, HubStoreError, MAX_GROUP_EXECUTION_LIST_LIMIT,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartGroupExecutionResult {
    pub v: u16,
    pub disposition: BeginGroupExecutionDisposition,
    pub inspection: GroupExecutionInspection,
}

pub struct GroupExecutionService {
    store: Arc<dyn GroupExecutionStore>,
}

impl GroupExecutionService {
    #[must_use]
    pub fn new(store: Arc<dyn GroupExecutionStore>) -> Self {
        Self { store }
    }

    /// Starts or safely completes one offline snapshot-validation execution.
    ///
    /// This path only verifies frozen local bytes and appends deterministic
    /// evidence. It never constructs a provider, tool, or workspace.
    ///
    /// # Errors
    ///
    /// Returns validation, corruption, conflict, or storage errors.
    pub fn start(
        &self,
        request: &BeginGroupExecution,
    ) -> Result<StartGroupExecutionResult, HubError> {
        validate_request(request)?;
        let begun = self.store.begin_group_execution(request)?;
        validate_begin_binding(request, &begun)?;
        let expected = expected_events(&begun)?;
        let current = self.checked_inspect(&begun.execution.execution_id)?;
        validate_execution_binding(&begun.execution, &current.execution)?;
        validate_prefix(&current.events, &expected)?;
        if matches!(current.recovery, GroupExecutionRecovery::Terminal { .. }) {
            return Ok(start_result(begun.disposition, current));
        }
        for event in expected.iter().skip(current.events.len()) {
            self.store.append_group_execution_event(event)?;
        }
        let completed = self.checked_inspect(&begun.execution.execution_id)?;
        validate_completed(&completed, &expected)?;
        Ok(start_result(begun.disposition, completed))
    }

    /// Loads and independently validates one durable execution prefix.
    ///
    /// # Errors
    ///
    /// Returns validation, corruption, or storage errors.
    pub fn inspect(&self, execution_id: &str) -> Result<GroupExecutionInspection, HubError> {
        required_identifier(execution_id, HubField::GroupExecutionId)?;
        self.checked_inspect(execution_id)
    }

    /// Lists bounded Group Execution metadata.
    ///
    /// # Errors
    ///
    /// Returns validation or storage errors.
    pub fn list(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupExecutionRecord>, HubError> {
        if let Some(id) = group_run_id {
            required_identifier(id, HubField::GroupRunId)?;
        }
        validate_list_limit(limit)?;
        Ok(self.store.list_group_executions(group_run_id, limit)?)
    }

    fn checked_inspect(&self, execution_id: &str) -> Result<GroupExecutionInspection, HubError> {
        let stored = self.store.inspect_group_execution(execution_id)?;
        let rebuilt =
            GroupExecutionInspection::validate(stored.execution.clone(), stored.events.clone())
                .map_err(|error| corrupt(&error.message))?;
        if rebuilt == stored {
            Ok(stored)
        } else {
            Err(corrupt(
                "stored Group Execution inspection disagrees with its journal",
            ))
        }
    }
}

fn validate_request(request: &BeginGroupExecution) -> Result<(), HubError> {
    if request.v != GROUP_EXECUTION_VERSION {
        return Err(HubError::UnsupportedGroupExecutionVersion {
            actual: request.v,
            expected: GROUP_EXECUTION_VERSION,
        });
    }
    required_identifier(&request.execution_id, HubField::GroupExecutionId)?;
    required_identifier(&request.group_run_id, HubField::GroupRunId)?;
    required_text(
        &request.idempotency_key,
        HubField::IdempotencyKey,
        MAX_IDEMPOTENCY_KEY_BYTES,
    )?;
    if i64::try_from(request.created_at_ms).is_err() {
        return Err(HubError::GroupExecutionCreationTimeOutOfRange);
    }
    Ok(())
}

fn validate_begin_binding(
    request: &BeginGroupExecution,
    result: &BeginGroupExecutionResult,
) -> Result<(), HubError> {
    let execution = &result.execution;
    let snapshot = &result.snapshot;
    let valid = result.v == GROUP_EXECUTION_VERSION
        && execution.v == GROUP_EXECUTION_VERSION
        && execution.group_run_id == request.group_run_id
        && execution.mode == request.mode
        && execution.protocol_version == GROUP_EXECUTION_PROTOCOL_VERSION
        && snapshot.v == GROUP_RUN_VERSION
        && snapshot.run.v == GROUP_RUN_VERSION
        && snapshot.run.status == GroupRunStatus::Prepared
        && snapshot.run.run_id == execution.group_run_id
        && snapshot.run.snapshot_sha256 == execution.source_snapshot_sha256;
    let disposition_valid = match result.disposition {
        BeginGroupExecutionDisposition::Created => {
            execution.execution_id == request.execution_id
                && execution.created_at_ms == request.created_at_ms
                && execution.status == GroupExecutionStatus::Incomplete
        }
        BeginGroupExecutionDisposition::Replayed => true,
    };
    (valid && disposition_valid)
        .then_some(())
        .ok_or_else(|| corrupt("Group Execution begin result has inconsistent source bindings"))
}

fn validate_execution_binding(
    expected: &GroupExecutionRecord,
    actual: &GroupExecutionRecord,
) -> Result<(), HubError> {
    let valid = expected.v == actual.v
        && expected.execution_id == actual.execution_id
        && expected.group_run_id == actual.group_run_id
        && expected.mode == actual.mode
        && expected.source_snapshot_sha256 == actual.source_snapshot_sha256
        && expected.protocol_version == actual.protocol_version
        && expected.created_at_ms == actual.created_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| corrupt("Group Execution inspection belongs to another execution"))
}

fn expected_events(
    begun: &BeginGroupExecutionResult,
) -> Result<Vec<GroupExecutionEvent>, HubError> {
    let execution = &begun.execution;
    let snapshot = &begun.snapshot;
    let receipt = GroupExecutionReceipt {
        v: GROUP_EXECUTION_VERSION,
        execution_id: execution.execution_id.clone(),
        group_run_id: execution.group_run_id.clone(),
        group_id: snapshot.run.group_id.clone(),
        context_version: snapshot.run.context_version,
        context_slice_sha256: snapshot.run.context_slice_sha256.clone(),
        snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
        snapshot_bytes: snapshot.run.snapshot_bytes,
        stats: snapshot.context.payload.stats.clone(),
    };
    let events = deterministic_events(execution, receipt);
    let mut cursor =
        GroupExecutionJournalCursor::new(execution).map_err(|error| corrupt(&error.message))?;
    for event in &events {
        cursor
            .append(event)
            .map_err(|error| corrupt(&error.message))?;
    }
    Ok(events)
}

fn deterministic_events(
    execution: &GroupExecutionRecord,
    receipt: GroupExecutionReceipt,
) -> Vec<GroupExecutionEvent> {
    vec![
        event(
            execution,
            1,
            GroupExecutionEventKind::ExecutionStarted {
                group_run_id: execution.group_run_id.clone(),
                snapshot_sha256: execution.source_snapshot_sha256.clone(),
            },
        ),
        event(
            execution,
            2,
            GroupExecutionEventKind::SnapshotVerified { receipt },
        ),
        event(
            execution,
            3,
            GroupExecutionEventKind::ExecutionFinished {
                outcome: GroupExecutionOutcome::SnapshotValidated,
            },
        ),
    ]
}

fn event(
    execution: &GroupExecutionRecord,
    seq: u64,
    kind: GroupExecutionEventKind,
) -> GroupExecutionEvent {
    GroupExecutionEvent {
        v: GROUP_EXECUTION_PROTOCOL_VERSION,
        execution_id: execution.execution_id.clone(),
        seq,
        kind,
    }
}

fn validate_prefix(
    actual: &[GroupExecutionEvent],
    expected: &[GroupExecutionEvent],
) -> Result<(), HubError> {
    if actual.len() <= expected.len() && actual == &expected[..actual.len()] {
        Ok(())
    } else {
        Err(corrupt(
            "Group Execution journal is not the deterministic snapshot-validation prefix",
        ))
    }
}

fn validate_completed(
    inspection: &GroupExecutionInspection,
    expected: &[GroupExecutionEvent],
) -> Result<(), HubError> {
    validate_prefix(&inspection.events, expected)?;
    let complete = inspection.events.len() == expected.len()
        && matches!(
            inspection.recovery,
            GroupExecutionRecovery::Terminal {
                outcome: GroupExecutionOutcome::SnapshotValidated
            }
        );
    complete
        .then_some(())
        .ok_or_else(|| corrupt("Group Execution did not reach its validated terminal state"))
}

fn start_result(
    disposition: BeginGroupExecutionDisposition,
    inspection: GroupExecutionInspection,
) -> StartGroupExecutionResult {
    StartGroupExecutionResult {
        v: GROUP_EXECUTION_VERSION,
        disposition,
        inspection,
    }
}

fn required_identifier(value: &str, field: HubField) -> Result<(), HubError> {
    required_text(value, field, MAX_ENTITY_ID_BYTES)
}

fn required_text(value: &str, field: HubField, max_bytes: usize) -> Result<(), HubError> {
    if value.trim().is_empty() {
        return Err(HubError::Empty { field });
    }
    if value.len() > max_bytes {
        return Err(HubError::TooLong { field, max_bytes });
    }
    if value.chars().any(unsupported_identifier_character) {
        return Err(HubError::InvalidCharacters { field });
    }
    Ok(())
}

fn unsupported_identifier_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn validate_list_limit(limit: usize) -> Result<(), HubError> {
    if (1..=MAX_GROUP_EXECUTION_LIST_LIMIT).contains(&limit) {
        return Ok(());
    }
    Err(HubError::OutOfRange {
        field: HubField::GroupExecutionLimit,
        min: 1,
        max: MAX_GROUP_EXECUTION_LIST_LIMIT,
    })
}

fn corrupt(message: &str) -> HubError {
    HubError::Store(HubStoreError::Corrupt {
        message: message.into(),
    })
}
