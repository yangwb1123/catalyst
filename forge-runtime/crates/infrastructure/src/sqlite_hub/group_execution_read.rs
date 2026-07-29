use crate::runtime_domain::{
    GroupExecutionInspection, GroupExecutionJournalCursor, GroupExecutionMode,
    GroupExecutionRecord, GroupExecutionStatus, GroupRunSnapshot, HubEntity, HubStoreError,
    MAX_GROUP_EXECUTION_EVENTS, MAX_GROUP_EXECUTION_JOURNAL_BYTES, MAX_GROUP_EXECUTION_LIST_LIMIT,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use super::{
    group_execution_codec::{
        decode_cursor, decode_event, validate_event_source, validate_receipt_binding,
        validate_record_metadata, validate_source_binding,
    },
    group_run_codec::{encode_hex_digest, valid_text},
    group_run_read::{decode_stored as decode_group_run, find_by_id as find_group_run},
    read_error,
};

const RECORD_COLUMNS: &str = "id,group_run_id,execution_version,mode,status,\
 source_snapshot_sha256,protocol_version,created_at_ms";
const STORED_COLUMNS: &str = "id,group_run_id,execution_version,mode,status,\
 source_snapshot_sha256,protocol_version,created_at_ms,cursor_json,journal_bytes,\
 idempotency_key";
const MAX_ID_BYTES: usize = 128;
const MAX_KEY_BYTES: usize = 256;

pub(super) struct StoredExecution {
    pub record: GroupExecutionRecord,
    pub cursor_json: String,
    pub journal_bytes: usize,
    pub idempotency_key: String,
}

pub(super) struct ValidatedExecution {
    pub stored: StoredExecution,
    pub cursor: GroupExecutionJournalCursor,
    pub inspection: GroupExecutionInspection,
    pub source: GroupRunSnapshot,
}

pub(super) fn inspect(
    connection: &mut Connection,
    execution_id: &str,
) -> Result<GroupExecutionInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let stored = find_by_id(&transaction, execution_id)?
        .ok_or_else(|| not_found(HubEntity::GroupExecution, execution_id))?;
    let validated = validate_stored(&transaction, stored)?;
    transaction.commit().map_err(read_error)?;
    Ok(validated.inspection)
}

pub(super) fn list(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupExecutionRecord>, HubStoreError> {
    validate_list_limit(limit)?;
    if let Some(id) = group_run_id {
        validate_filter(id)?;
        ensure_group_run_exists(connection, id)?;
    }
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    match group_run_id {
        Some(id) => query_records(
            connection,
            "WHERE group_run_id = ?1 ORDER BY created_at_ms DESC,id DESC LIMIT ?2",
            params![id, limit],
        ),
        None => query_records(
            connection,
            "ORDER BY created_at_ms DESC,id DESC LIMIT ?1",
            [limit],
        ),
    }
}

pub(super) fn find_by_id(
    connection: &Connection,
    execution_id: &str,
) -> Result<Option<StoredExecution>, HubStoreError> {
    query_stored(connection, "id", execution_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<StoredExecution>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn load_source_for_begin(
    connection: &Connection,
    group_run_id: &str,
) -> Result<GroupRunSnapshot, HubStoreError> {
    let stored = find_group_run(connection, group_run_id)?
        .ok_or_else(|| not_found(HubEntity::GroupRun, group_run_id))?;
    decode_group_run(stored)
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: StoredExecution,
) -> Result<ValidatedExecution, HubStoreError> {
    validate_stored_header(&stored)?;
    let source = load_existing_source(connection, &stored.record.group_run_id)?;
    validate_source_binding(&stored.record, &source)?;
    let (events, actual_bytes) = load_events(connection, &stored.record.execution_id)?;
    validate_stored_events(&stored.record, &source, &events)?;
    let inspection = GroupExecutionInspection::validate(stored.record.clone(), events)
        .map_err(|error| corrupt(&error.message))?;
    validate_journal(&stored, &inspection, actual_bytes)?;
    if let Some(receipt) = &inspection.receipt {
        validate_receipt_binding(&stored.record, &source, receipt)?;
    }
    let cursor = decode_cursor(&stored.cursor_json, &stored.record)?;
    Ok(ValidatedExecution {
        stored,
        cursor,
        inspection,
        source,
    })
}

fn validate_stored_header(stored: &StoredExecution) -> Result<(), HubStoreError> {
    validate_record_metadata(&stored.record)?;
    if !valid_text(&stored.idempotency_key, MAX_KEY_BYTES)
        || stored.journal_bytes > MAX_GROUP_EXECUTION_JOURNAL_BYTES
    {
        return Err(corrupt(
            "stored Group Execution key or journal byte count violates its bounds",
        ));
    }
    Ok(())
}

fn load_existing_source(
    connection: &Connection,
    group_run_id: &str,
) -> Result<GroupRunSnapshot, HubStoreError> {
    let stored = find_group_run(connection, group_run_id)?
        .ok_or_else(|| corrupt("stored Group Execution references a missing frozen Group Run"))?;
    decode_group_run(stored)
}

fn load_events(
    connection: &Connection,
    execution_id: &str,
) -> Result<(Vec<crate::runtime_domain::GroupExecutionEvent>, usize), HubStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT seq,event_json,event_sha256 FROM group_execution_events
             WHERE execution_id = ?1 ORDER BY seq LIMIT 4",
        )
        .map_err(read_error)?;
    let rows = statement
        .query_map([execution_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(read_error)?;
    collect_events(rows)
}

fn collect_events(
    rows: rusqlite::MappedRows<
        '_,
        impl FnMut(&Row<'_>) -> rusqlite::Result<(i64, String, Vec<u8>)>,
    >,
) -> Result<(Vec<crate::runtime_domain::GroupExecutionEvent>, usize), HubStoreError> {
    let mut events = Vec::new();
    let mut total_bytes = 0_usize;
    for row in rows {
        let (sequence, json, digest) = row.map_err(read_error)?;
        if events.len() >= MAX_GROUP_EXECUTION_EVENTS {
            return Err(corrupt("stored Group Execution has too many events"));
        }
        total_bytes = total_bytes
            .checked_add(json.len())
            .ok_or_else(|| corrupt("stored Group Execution journal byte count overflowed"))?;
        if total_bytes > MAX_GROUP_EXECUTION_JOURNAL_BYTES {
            return Err(corrupt(
                "stored Group Execution journal exceeds its durable byte limit",
            ));
        }
        events.push(decode_event(sequence, &json, &digest)?);
    }
    Ok((events, total_bytes))
}

fn validate_stored_events(
    record: &GroupExecutionRecord,
    source: &GroupRunSnapshot,
    events: &[crate::runtime_domain::GroupExecutionEvent],
) -> Result<(), HubStoreError> {
    for event in events {
        validate_event_source(record, source, event).map_err(|error| {
            corrupt(&format!(
                "stored Group Execution event violates its frozen source: {error}"
            ))
        })?;
    }
    Ok(())
}

fn validate_journal(
    stored: &StoredExecution,
    inspection: &GroupExecutionInspection,
    actual_bytes: usize,
) -> Result<(), HubStoreError> {
    let cursor = decode_cursor(&stored.cursor_json, &stored.record)?;
    let mut rebuilt = GroupExecutionJournalCursor::new(&stored.record)
        .map_err(|error| corrupt(&error.message))?;
    for event in &inspection.events {
        rebuilt
            .append(event)
            .map_err(|error| corrupt(&error.message))?;
    }
    if cursor == rebuilt && stored.journal_bytes == actual_bytes {
        Ok(())
    } else {
        Err(corrupt(
            "stored Group Execution cursor or byte count disagrees with its journal",
        ))
    }
}

fn query_stored(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<StoredExecution>, HubStoreError> {
    let sql = match column {
        "id" => format!("SELECT {STORED_COLUMNS} FROM group_executions WHERE id = ?1"),
        "idempotency_key" => {
            format!("SELECT {STORED_COLUMNS} FROM group_executions WHERE idempotency_key = ?1")
        }
        _ => return Err(conflict("unsupported Group Execution lookup")),
    };
    connection
        .query_row(&sql, [value], stored_row)
        .optional()
        .map_err(read_error)
}

fn query_records<P>(
    connection: &Connection,
    suffix: &str,
    parameters: P,
) -> Result<Vec<GroupExecutionRecord>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!("SELECT {RECORD_COLUMNS} FROM group_executions {suffix}");
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, record_row)
        .map_err(read_error)?
        .map(|row| {
            let record = row.map_err(read_error)?;
            validate_record_metadata(&record)?;
            Ok(record)
        })
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<StoredExecution> {
    let journal_bytes = convert(row, 9, "Group Execution journal byte count")?;
    Ok(StoredExecution {
        record: record_row(row)?,
        cursor_json: row.get(8)?,
        journal_bytes,
        idempotency_key: row.get(10)?,
    })
}

fn record_row(row: &Row<'_>) -> rusqlite::Result<GroupExecutionRecord> {
    let mode = parse_mode(&row.get::<_, String>(3)?)?;
    let status = parse_status(&row.get::<_, String>(4)?)?;
    let source_digest = parse_digest(row, 5)?;
    Ok(GroupExecutionRecord {
        v: convert(row, 2, "Group Execution version")?,
        execution_id: row.get(0)?,
        group_run_id: row.get(1)?,
        mode,
        status,
        source_snapshot_sha256: source_digest,
        protocol_version: convert(row, 6, "Group Execution protocol version")?,
        created_at_ms: convert(row, 7, "Group Execution creation time")?,
    })
}

fn parse_mode(value: &str) -> rusqlite::Result<GroupExecutionMode> {
    match value {
        "offline_snapshot_validation" => Ok(GroupExecutionMode::OfflineSnapshotValidation),
        _ => Err(conversion_error(3, "unsupported Group Execution mode")),
    }
}

fn parse_status(value: &str) -> rusqlite::Result<GroupExecutionStatus> {
    match value {
        "incomplete" => Ok(GroupExecutionStatus::Incomplete),
        "completed" => Ok(GroupExecutionStatus::Completed),
        _ => Err(conversion_error(4, "unsupported Group Execution status")),
    }
}

fn parse_digest(row: &Row<'_>, index: usize) -> rusqlite::Result<String> {
    let value: Vec<u8> = row.get(index)?;
    let digest: [u8; 32] = value
        .try_into()
        .map_err(|_| conversion_error(index, "stored digest must contain 32 bytes"))?;
    Ok(encode_hex_digest(&digest))
}

fn convert<T>(row: &Row<'_>, index: usize, subject: &str) -> rusqlite::Result<T>
where
    T: TryFrom<i64>,
    T::Error: std::error::Error + Send + Sync + 'static,
{
    T::try_from(row.get::<_, i64>(index)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid {subject}: {error}"),
            )),
        )
    })
}

fn ensure_group_run_exists(connection: &Connection, id: &str) -> Result<(), HubStoreError> {
    connection
        .query_row("SELECT 1 FROM group_runs WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::GroupRun, id))
}

fn validate_filter(id: &str) -> Result<(), HubStoreError> {
    if valid_text(id, MAX_ID_BYTES) {
        Ok(())
    } else {
        Err(conflict("Group Run filter is outside its byte bounds"))
    }
}

fn validate_list_limit(limit: usize) -> Result<(), HubStoreError> {
    if (1..=MAX_GROUP_EXECUTION_LIST_LIMIT).contains(&limit) {
        Ok(())
    } else {
        Err(conflict("Group Execution list limit is outside its bounds"))
    }
}

fn conversion_error(index: usize, message: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.to_owned(),
        )),
    )
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupExecution,
        message: message.into(),
    }
}
