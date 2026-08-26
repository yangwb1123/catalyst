use crate::runtime_domain::{
    MAX_RUN_CURSOR_JSON_BYTES, MAX_RUN_EVENT_JSON_BYTES, MAX_RUN_EVENTS,
    MAX_RUN_EXECUTION_JSON_BYTES, MAX_RUN_JOURNAL_BYTES, MAX_RUN_LIST_LIMIT, RUN_STORE_VERSION,
    RunEntity, RunExecution, RunInspection, RunJournalCursor, RunRecord, RunStoreError,
    RuntimeEvent,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};

use super::run_integrity::validate_bound_prompt;

const RUN_COLUMNS: &str = "id, conversation_id, prompt_id, project_id, execution_json, \
                           idempotency_key, protocol_version, created_at_ms";

pub(super) struct StoredRun {
    pub record: RunRecord,
    pub idempotency_key: String,
}

pub(super) fn inspect(
    connection: &Connection,
    run_id: &str,
) -> Result<RunInspection, RunStoreError> {
    inspect_snapshot(connection, run_id, || Ok(()))
}

pub(super) fn inspect_transaction(
    connection: &mut Connection,
    run_id: &str,
) -> Result<RunInspection, RunStoreError> {
    inspect_transaction_with_hook(connection, run_id, || Ok(()))
}

fn inspect_transaction_with_hook<F>(
    connection: &mut Connection,
    run_id: &str,
    after_cursor: F,
) -> Result<RunInspection, RunStoreError>
where
    F: FnOnce() -> Result<(), RunStoreError>,
{
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|error| read_error(&error))?;
    let inspection = inspect_snapshot(&transaction, run_id, after_cursor)?;
    transaction.commit().map_err(|error| read_error(&error))?;
    Ok(inspection)
}

fn inspect_snapshot<F>(
    connection: &Connection,
    run_id: &str,
    after_cursor: F,
) -> Result<RunInspection, RunStoreError>
where
    F: FnOnce() -> Result<(), RunStoreError>,
{
    let inspection = inspect_base_snapshot(connection, run_id, after_cursor)?;
    super::run_lineage_read::validate_inspection(connection, &inspection)?;
    Ok(inspection)
}

pub(super) fn inspect_base(
    connection: &Connection,
    run_id: &str,
) -> Result<RunInspection, RunStoreError> {
    inspect_base_snapshot(connection, run_id, || Ok(()))
}

fn inspect_base_snapshot<F>(
    connection: &Connection,
    run_id: &str,
    after_cursor: F,
) -> Result<RunInspection, RunStoreError>
where
    F: FnOnce() -> Result<(), RunStoreError>,
{
    let stored = find_run(connection, run_id)?.ok_or_else(|| not_found(RunEntity::Run, run_id))?;
    let journal = load_cursor(connection, &stored.record)?;
    after_cursor()?;
    let loaded = load_events(connection, run_id)?;
    validate_bound_prompt(connection, &stored.record, &loaded.events)?;
    let inspection = RunInspection::validate(stored.record, loaded.events).map_err(|error| {
        RunStoreError::Corrupt {
            message: error.message,
        }
    })?;
    validate_cursor_snapshot(&inspection, &journal, loaded.total_bytes)?;
    Ok(inspection)
}

#[cfg(test)]
pub(super) fn inspect_after_cursor<F>(
    connection: &mut Connection,
    run_id: &str,
    after_cursor: F,
) -> Result<RunInspection, RunStoreError>
where
    F: FnOnce() -> Result<(), RunStoreError>,
{
    inspect_transaction_with_hook(connection, run_id, after_cursor)
}

pub(super) struct StoredJournal {
    pub cursor: RunJournalCursor,
    pub total_bytes: usize,
}

pub(super) fn load_cursor(
    connection: &Connection,
    record: &RunRecord,
) -> Result<StoredJournal, RunStoreError> {
    let row = connection
        .query_row(
            "SELECT cursor_json,journal_bytes FROM runs WHERE id = ?1",
            [&record.run_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|error| read_error(&error))?
        .ok_or_else(|| not_found(RunEntity::Run, &record.run_id))?;
    if row.0.len() > MAX_RUN_CURSOR_JSON_BYTES {
        return Err(corrupt("stored Run cursor exceeds its byte limit"));
    }
    let cursor: RunJournalCursor =
        serde_json::from_str(&row.0).map_err(|error| RunStoreError::Corrupt {
            message: format!("invalid stored Run cursor JSON: {error}"),
        })?;
    cursor
        .validate_run(record)
        .map_err(|error| RunStoreError::Corrupt {
            message: error.message,
        })?;
    let total_bytes = usize::try_from(row.1).map_err(|error| RunStoreError::Corrupt {
        message: format!("invalid stored Run journal byte count: {error}"),
    })?;
    Ok(StoredJournal {
        cursor,
        total_bytes,
    })
}

pub(super) fn list_runs(
    connection: &Connection,
    conversation_id: Option<&str>,
    limit: usize,
) -> Result<Vec<RunRecord>, RunStoreError> {
    validate_list_limit(limit)?;
    if let Some(id) = conversation_id {
        ensure_conversation_exists(connection, id)?;
    }
    let limit = i64::try_from(limit).map_err(|error| RunStoreError::Conflict {
        entity: RunEntity::Run,
        message: error.to_string(),
    })?;
    match conversation_id {
        Some(id) => query_runs(
            connection,
            "WHERE conversation_id = ?1 ORDER BY created_at_ms DESC, id DESC LIMIT ?2",
            params![id, limit],
        ),
        None => query_runs(
            connection,
            "ORDER BY created_at_ms DESC, id DESC LIMIT ?1",
            [limit],
        ),
    }
}

pub(super) fn find_run(
    connection: &Connection,
    run_id: &str,
) -> Result<Option<StoredRun>, RunStoreError> {
    connection
        .query_row(
            &format!("SELECT {RUN_COLUMNS} FROM runs WHERE id = ?1"),
            [run_id],
            stored_run,
        )
        .optional()
        .map_err(|error| read_error(&error))
}

pub(super) fn find_run_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<StoredRun>, RunStoreError> {
    connection
        .query_row(
            &format!("SELECT {RUN_COLUMNS} FROM runs WHERE idempotency_key = ?1"),
            [key],
            stored_run,
        )
        .optional()
        .map_err(|error| read_error(&error))
}

pub(super) fn record_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RunRecord>, RunStoreError> {
    find_run_by_key(connection, key)?
        .map(listed_record)
        .transpose()
}

pub(super) fn validate_stored_run(stored: &StoredRun) -> Result<(), RunStoreError> {
    RunInspection::validate(stored.record.clone(), Vec::new())
        .map(|_| ())
        .map_err(|error| RunStoreError::Corrupt {
            message: error.message,
        })
}

struct LoadedEvents {
    events: Vec<RuntimeEvent>,
    total_bytes: usize,
}

fn load_events(connection: &Connection, run_id: &str) -> Result<LoadedEvents, RunStoreError> {
    let mut statement = connection
        .prepare("SELECT seq, event_json FROM run_events WHERE run_id = ?1 ORDER BY seq")
        .map_err(|error| read_error(&error))?;
    let rows = statement
        .query_map([run_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| read_error(&error))?;
    let mut events = Vec::new();
    let mut total_bytes = 0usize;
    for row in rows {
        let row = row.map_err(|error| read_error(&error))?;
        validate_stored_event_size(&row.1, events.len(), &mut total_bytes)?;
        events.push(decode_event(&row)?);
    }
    Ok(LoadedEvents {
        events,
        total_bytes,
    })
}

pub(super) fn load_event_at(
    connection: &Connection,
    run_id: &str,
    sequence: u64,
) -> Result<Option<RuntimeEvent>, RunStoreError> {
    let sequence = i64::try_from(sequence).map_err(|error| RunStoreError::Corrupt {
        message: format!("invalid Run event sequence: {error}"),
    })?;
    let row = connection
        .query_row(
            "SELECT seq,event_json FROM run_events WHERE run_id = ?1 AND seq = ?2",
            params![run_id, sequence],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|error| read_error(&error))?;
    row.map(|row| {
        if row.1.len() > MAX_RUN_EVENT_JSON_BYTES {
            return Err(corrupt("stored Run event exceeds its durable limit"));
        }
        decode_event(&row)
    })
    .transpose()
}

pub(super) fn last_event_sequence(
    connection: &Connection,
    run_id: &str,
) -> Result<u64, RunStoreError> {
    let sequence: Option<i64> = connection
        .query_row(
            "SELECT MAX(seq) FROM run_events WHERE run_id = ?1",
            [run_id],
            |row| row.get(0),
        )
        .map_err(|error| read_error(&error))?;
    u64::try_from(sequence.unwrap_or(0)).map_err(|error| RunStoreError::Corrupt {
        message: format!("invalid stored Run tail sequence: {error}"),
    })
}

fn validate_cursor_snapshot(
    inspection: &RunInspection,
    stored: &StoredJournal,
    actual_bytes: usize,
) -> Result<(), RunStoreError> {
    let mut rebuilt =
        RunJournalCursor::new(&inspection.run).map_err(|error| corrupt(&error.message))?;
    for event in &inspection.events {
        rebuilt
            .append(event)
            .map_err(|error| corrupt(&error.message))?;
    }
    let stored_value = serde_json::to_value(&stored.cursor)
        .map_err(|error| corrupt(&format!("stored cursor cannot encode: {error}")))?;
    let rebuilt_value = serde_json::to_value(rebuilt)
        .map_err(|error| corrupt(&format!("rebuilt cursor cannot encode: {error}")))?;
    if stored_value != rebuilt_value || stored.total_bytes != actual_bytes {
        return Err(corrupt(
            "stored Run cursor disagrees with its event journal",
        ));
    }
    Ok(())
}

fn validate_stored_event_size(
    json: &str,
    count: usize,
    total_bytes: &mut usize,
) -> Result<(), RunStoreError> {
    if count >= MAX_RUN_EVENTS || json.len() > MAX_RUN_EVENT_JSON_BYTES {
        return Err(corrupt("stored Run event exceeds its durable limit"));
    }
    *total_bytes = total_bytes.saturating_add(json.len());
    if *total_bytes > MAX_RUN_JOURNAL_BYTES {
        return Err(corrupt("stored Run journal exceeds its durable byte limit"));
    }
    Ok(())
}

fn decode_event(row: &(i64, String)) -> Result<RuntimeEvent, RunStoreError> {
    let sequence = u64::try_from(row.0).map_err(|error| RunStoreError::Corrupt {
        message: format!("invalid stored Run event sequence: {error}"),
    })?;
    let event: RuntimeEvent =
        serde_json::from_str(&row.1).map_err(|error| RunStoreError::Corrupt {
            message: format!("invalid stored Run event JSON: {error}"),
        })?;
    if event.seq != sequence {
        return Err(RunStoreError::Corrupt {
            message: "Run event row sequence disagrees with its JSON envelope".into(),
        });
    }
    Ok(event)
}

fn corrupt(message: &str) -> RunStoreError {
    RunStoreError::Corrupt {
        message: message.into(),
    }
}

fn stored_run(row: &Row<'_>) -> rusqlite::Result<StoredRun> {
    let execution_json: String = row.get(4)?;
    if execution_json.len() > MAX_RUN_EXECUTION_JSON_BYTES {
        return Err(conversion_error(
            4,
            "stored Run execution exceeds its byte limit",
        ));
    }
    let execution: RunExecution = serde_json::from_str(&execution_json)
        .map_err(|error| conversion_error(4, &error.to_string()))?;
    let protocol_version = u16::try_from(row.get::<_, i64>(6)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            6,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    let created_at_ms = u64::try_from(row.get::<_, i64>(7)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })?;
    Ok(StoredRun {
        record: RunRecord {
            v: RUN_STORE_VERSION,
            run_id: row.get(0)?,
            conversation_id: row.get(1)?,
            prompt_id: row.get(2)?,
            project_id: row.get(3)?,
            execution,
            protocol_version,
            created_at_ms,
        },
        idempotency_key: row.get(5)?,
    })
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

fn query_runs<P>(
    connection: &Connection,
    suffix: &str,
    parameters: P,
) -> Result<Vec<RunRecord>, RunStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection
        .prepare(&format!("SELECT {RUN_COLUMNS} FROM runs {suffix}"))
        .map_err(|error| read_error(&error))?;
    statement
        .query_map(parameters, stored_run)
        .map_err(|error| read_error(&error))?
        .map(|row| listed_record(row.map_err(|error| read_error(&error))?))
        .collect()
}

fn listed_record(stored: StoredRun) -> Result<RunRecord, RunStoreError> {
    validate_stored_run(&stored)?;
    Ok(stored.record)
}

fn ensure_conversation_exists(
    connection: &Connection,
    conversation_id: &str,
) -> Result<(), RunStoreError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM conversations WHERE id = ?1",
            [conversation_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| read_error(&error))?
        .is_some();
    exists
        .then_some(())
        .ok_or_else(|| not_found(RunEntity::Conversation, conversation_id))
}

fn validate_list_limit(limit: usize) -> Result<(), RunStoreError> {
    if (1..=MAX_RUN_LIST_LIMIT).contains(&limit) {
        return Ok(());
    }
    Err(RunStoreError::Conflict {
        entity: RunEntity::Run,
        message: format!("Run list limit must be between 1 and {MAX_RUN_LIST_LIMIT}"),
    })
}

fn not_found(entity: RunEntity, id: &str) -> RunStoreError {
    RunStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn read_error(error: &rusqlite::Error) -> RunStoreError {
    match error {
        rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::InvalidColumnType(..)
        | rusqlite::Error::IntegralValueOutOfRange(..) => RunStoreError::Corrupt {
            message: error.to_string(),
        },
        _ => RunStoreError::Unavailable {
            message: error.to_string(),
        },
    }
}
