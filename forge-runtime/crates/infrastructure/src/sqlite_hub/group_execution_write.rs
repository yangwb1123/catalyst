use std::time::Duration;

use crate::runtime_domain::{
    BeginGroupExecution, BeginGroupExecutionDisposition, BeginGroupExecutionResult,
    GROUP_EXECUTION_PROTOCOL_VERSION, GROUP_EXECUTION_VERSION, GroupExecutionEvent,
    GroupExecutionJournalCursor, GroupExecutionRecord, GroupExecutionRecovery,
    GroupExecutionStatus, HubEntity, HubStoreError, MAX_GROUP_EXECUTION_EVENTS,
    MAX_GROUP_EXECUTION_JOURNAL_BYTES,
};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use super::{
    group_execution_codec::{
        encode_cursor, encode_event, validate_event_source, validate_source_binding,
    },
    group_execution_read::{
        ValidatedExecution, find_by_id, find_by_key, load_source_for_begin, validate_stored,
    },
    group_run_codec::valid_text,
    read_error, write_error,
};

const EXECUTION_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ID_BYTES: usize = 128;
const MAX_KEY_BYTES: usize = 256;

pub(super) fn begin(
    connection: &mut Connection,
    request: &BeginGroupExecution,
) -> Result<BeginGroupExecutionResult, HubStoreError> {
    validate_replay_input(request)?;
    let transaction = immediate(connection)?;
    let result = begin_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

pub(super) fn append(
    connection: &mut Connection,
    event: &GroupExecutionEvent,
) -> Result<(), HubStoreError> {
    validate_event_identifier(event)?;
    let transaction = immediate(connection)?;
    append_locked(&transaction, event)?;
    transaction.commit().map_err(read_error)
}

fn immediate(connection: &mut Connection) -> Result<Transaction<'_>, HubStoreError> {
    connection
        .busy_timeout(EXECUTION_BUSY_TIMEOUT)
        .map_err(read_error)?;
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)
}

fn begin_locked(
    transaction: &Transaction<'_>,
    request: &BeginGroupExecution,
) -> Result<BeginGroupExecutionResult, HubStoreError> {
    if let Some(stored) = find_by_key(transaction, &request.idempotency_key)? {
        let validated = validate_stored(transaction, stored)?;
        ensure_idempotent_replay(&validated, request)?;
        return Ok(begin_result(
            BeginGroupExecutionDisposition::Replayed,
            validated.stored.record,
            validated.source,
        ));
    }
    validate_new_candidate(request)?;
    if let Some(stored) = find_by_id(transaction, &request.execution_id)? {
        validate_stored(transaction, stored)?;
        return Err(conflict(
            "Group Execution ID already belongs to another idempotency key",
        ));
    }
    create_locked(transaction, request)
}

fn create_locked(
    transaction: &Transaction<'_>,
    request: &BeginGroupExecution,
) -> Result<BeginGroupExecutionResult, HubStoreError> {
    let source = load_source_for_begin(transaction, &request.group_run_id)?;
    let record = record_from(request, &source.run.snapshot_sha256);
    validate_source_binding(&record, &source)?;
    let cursor =
        GroupExecutionJournalCursor::new(&record).map_err(|error| conflict(&error.message))?;
    let cursor_json = encode_cursor(&cursor)?;
    insert_execution(transaction, &record, &cursor_json, &request.idempotency_key)?;
    Ok(begin_result(
        BeginGroupExecutionDisposition::Created,
        record,
        source,
    ))
}

fn append_locked(
    transaction: &Transaction<'_>,
    event: &GroupExecutionEvent,
) -> Result<(), HubStoreError> {
    let stored = find_by_id(transaction, &event.execution_id)?
        .ok_or_else(|| not_found(HubEntity::GroupExecution, &event.execution_id))?;
    let mut validated = validate_stored(transaction, stored)?;
    let expected = validated.cursor.next_sequence();
    if event.seq < expected {
        return ensure_exact_replay(&validated, event);
    }
    if event.seq != expected {
        return Err(conflict("Group Execution event sequence is not contiguous"));
    }
    append_new_event(transaction, &mut validated, event)
}

fn append_new_event(
    transaction: &Transaction<'_>,
    validated: &mut ValidatedExecution,
    event: &GroupExecutionEvent,
) -> Result<(), HubStoreError> {
    if validated.stored.record.status == GroupExecutionStatus::Completed {
        return Err(conflict("Group Execution journal is already terminal"));
    }
    validate_event_source(&validated.stored.record, &validated.source, event)
        .map_err(|_| conflict("Group Execution event does not match its frozen source"))?;
    let encoded = encode_event(event)?;
    validate_capacity(
        validated.inspection.events.len(),
        validated.stored.journal_bytes,
        encoded.json.len(),
    )?;
    validated
        .cursor
        .append(event)
        .map_err(|error| conflict(&error.message))?;
    let journal_bytes = validated
        .stored
        .journal_bytes
        .checked_add(encoded.json.len())
        .ok_or_else(|| conflict("Group Execution journal byte count overflowed"))?;
    let cursor_json = encode_cursor(&validated.cursor)?;
    insert_event(transaction, event, &encoded)?;
    update_journal(
        transaction,
        &event.execution_id,
        &cursor_json,
        journal_bytes,
        status_from_cursor(&validated.cursor),
    )
}

fn validate_capacity(
    event_count: usize,
    journal_bytes: usize,
    candidate_bytes: usize,
) -> Result<(), HubStoreError> {
    if event_count >= MAX_GROUP_EXECUTION_EVENTS {
        return Err(conflict(
            "Group Execution journal already has its maximum events",
        ));
    }
    let total = journal_bytes
        .checked_add(candidate_bytes)
        .ok_or_else(|| conflict("Group Execution journal byte count overflowed"))?;
    if total <= MAX_GROUP_EXECUTION_JOURNAL_BYTES {
        Ok(())
    } else {
        Err(conflict(
            "Group Execution journal exceeds its durable byte limit",
        ))
    }
}

fn ensure_exact_replay(
    validated: &ValidatedExecution,
    event: &GroupExecutionEvent,
) -> Result<(), HubStoreError> {
    let index = usize::try_from(event.seq.saturating_sub(1))
        .map_err(|error| conflict(&format!("invalid event sequence: {error}")))?;
    match validated.inspection.events.get(index) {
        Some(committed) if committed == event => Ok(()),
        Some(_) => Err(conflict(
            "Group Execution event sequence was replayed with different content",
        )),
        None => Err(HubStoreError::Corrupt {
            message: "Group Execution cursor references a missing durable event".into(),
        }),
    }
}

fn validate_replay_input(request: &BeginGroupExecution) -> Result<(), HubStoreError> {
    if request.v != GROUP_EXECUTION_VERSION {
        return Err(conflict("unsupported Group Execution request version"));
    }
    for (value, max, subject) in [
        (request.group_run_id.as_str(), MAX_ID_BYTES, "Group Run ID"),
        (
            request.idempotency_key.as_str(),
            MAX_KEY_BYTES,
            "idempotency key",
        ),
    ] {
        if !valid_text(value, max) {
            return Err(conflict(&format!("{subject} is outside its byte bounds")));
        }
    }
    Ok(())
}

fn validate_new_candidate(request: &BeginGroupExecution) -> Result<(), HubStoreError> {
    if !valid_text(&request.execution_id, MAX_ID_BYTES) {
        return Err(conflict("Group Execution ID is outside its byte bounds"));
    }
    i64::try_from(request.created_at_ms)
        .map(|_| ())
        .map_err(|_| conflict("Group Execution creation time exceeds SQLite bounds"))
}

fn validate_event_identifier(event: &GroupExecutionEvent) -> Result<(), HubStoreError> {
    if valid_text(&event.execution_id, MAX_ID_BYTES) && event.seq > 0 {
        Ok(())
    } else {
        Err(conflict(
            "Group Execution event has an invalid execution ID or sequence",
        ))
    }
}

fn ensure_idempotent_replay(
    validated: &ValidatedExecution,
    request: &BeginGroupExecution,
) -> Result<(), HubStoreError> {
    let record = &validated.stored.record;
    let matches = validated.stored.idempotency_key == request.idempotency_key
        && record.group_run_id == request.group_run_id
        && record.mode == request.mode;
    if matches {
        Ok(())
    } else {
        Err(conflict(
            "idempotency key was reused with a different source or mode",
        ))
    }
}

fn record_from(request: &BeginGroupExecution, source_digest: &str) -> GroupExecutionRecord {
    GroupExecutionRecord {
        v: GROUP_EXECUTION_VERSION,
        execution_id: request.execution_id.clone(),
        group_run_id: request.group_run_id.clone(),
        mode: request.mode,
        status: GroupExecutionStatus::Incomplete,
        source_snapshot_sha256: source_digest.into(),
        protocol_version: GROUP_EXECUTION_PROTOCOL_VERSION,
        created_at_ms: request.created_at_ms,
    }
}

fn insert_execution(
    transaction: &Transaction<'_>,
    record: &GroupExecutionRecord,
    cursor_json: &str,
    key: &str,
) -> Result<(), HubStoreError> {
    let source_digest = source_digest_from(record)?;
    transaction
        .execute(
            "INSERT INTO group_executions(
               id,group_run_id,execution_version,mode,status,source_snapshot_sha256,
               cursor_json,journal_bytes,idempotency_key,protocol_version,created_at_ms
             ) VALUES(?1,?2,?3,'offline_snapshot_validation','incomplete',?4,?5,0,?6,?7,?8)",
            params![
                record.execution_id,
                record.group_run_id,
                i64::from(record.v),
                source_digest.as_slice(),
                cursor_json,
                key,
                i64::from(record.protocol_version),
                to_i64(record.created_at_ms)?
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupExecution, error))?;
    Ok(())
}

fn source_digest_from(record: &GroupExecutionRecord) -> Result<[u8; 32], HubStoreError> {
    crate::runtime_domain::GroupExecutionJournalCursor::new(record)
        .map_err(|error| conflict(&error.message))?;
    super::group_run_codec::decode_hex_digest(&record.source_snapshot_sha256)
        .ok_or_else(|| conflict("Group Execution source digest is invalid"))
}

fn insert_event(
    transaction: &Transaction<'_>,
    event: &GroupExecutionEvent,
    encoded: &super::group_execution_codec::EncodedEvent,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_execution_events(execution_id,seq,event_json,event_sha256)
             VALUES(?1,?2,?3,?4)",
            params![
                event.execution_id,
                to_i64(event.seq)?,
                encoded.json,
                encoded.digest.as_slice()
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupExecution, error))?;
    Ok(())
}

fn update_journal(
    transaction: &Transaction<'_>,
    execution_id: &str,
    cursor_json: &str,
    journal_bytes: usize,
    status: GroupExecutionStatus,
) -> Result<(), HubStoreError> {
    let changed = transaction
        .execute(
            "UPDATE group_executions
             SET cursor_json = ?1,journal_bytes = ?2,status = ?3 WHERE id = ?4",
            params![
                cursor_json,
                i64::try_from(journal_bytes).map_err(|error| conflict(&error.to_string()))?,
                status_text(status),
                execution_id
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupExecution, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(HubStoreError::Corrupt {
            message: "Group Execution disappeared while updating its journal".into(),
        })
    }
}

fn status_from_cursor(cursor: &GroupExecutionJournalCursor) -> GroupExecutionStatus {
    match cursor.recovery() {
        GroupExecutionRecovery::Terminal { .. } => GroupExecutionStatus::Completed,
        GroupExecutionRecovery::Incomplete => GroupExecutionStatus::Incomplete,
    }
}

fn status_text(status: GroupExecutionStatus) -> &'static str {
    match status {
        GroupExecutionStatus::Incomplete => "incomplete",
        GroupExecutionStatus::Completed => "completed",
    }
}

fn begin_result(
    disposition: BeginGroupExecutionDisposition,
    execution: GroupExecutionRecord,
    snapshot: crate::runtime_domain::GroupRunSnapshot,
) -> BeginGroupExecutionResult {
    BeginGroupExecutionResult {
        v: GROUP_EXECUTION_VERSION,
        disposition,
        execution,
        snapshot,
    }
}

fn to_i64(value: u64) -> Result<i64, HubStoreError> {
    i64::try_from(value).map_err(|error| conflict(&error.to_string()))
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupExecution,
        message: message.into(),
    }
}
