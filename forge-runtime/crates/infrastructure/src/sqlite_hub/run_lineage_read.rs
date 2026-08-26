use std::fmt::Write as _;

use crate::runtime_domain::{
    RunBranchMode, RunInspection, RunLineageRecord, RunRecoveryState, RunStoreError,
    RuntimeEventKind, source_event_sha256,
};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior};

use super::run_read;

const LINEAGE_COLUMNS: &str = "child_run_id,lineage_version,relation_kind,branch_mode,parent_run_id,\
                               source_event_seq,source_event_sha256,lineage_sha256,created_at_ms";

pub(super) fn find_validated(
    connection: &mut Connection,
    run_id: &str,
) -> Result<Option<RunLineageRecord>, RunStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(|error| unavailable(&error))?;
    let child = run_read::inspect_base(&transaction, run_id)?;
    validate_inspection(&transaction, &child)?;
    let lineage = find(&transaction, run_id)?;
    transaction.commit().map_err(|error| unavailable(&error))?;
    Ok(lineage)
}

pub(super) fn find(
    connection: &Connection,
    run_id: &str,
) -> Result<Option<RunLineageRecord>, RunStoreError> {
    let record = connection
        .query_row(
            &format!("SELECT {LINEAGE_COLUMNS} FROM run_lineages WHERE child_run_id=?1"),
            [run_id],
            lineage_row,
        )
        .optional()
        .map_err(|error| read_error(&error))?;
    record.map(validate_record).transpose()
}

pub(super) fn validate_inspection(
    connection: &Connection,
    child: &RunInspection,
) -> Result<(), RunStoreError> {
    let Some(lineage) = find(connection, &child.run.run_id)? else {
        return Ok(());
    };
    let parent = run_read::inspect_base(connection, &lineage.parent_run_id)?;
    validate_parent(&parent, &lineage)?;
    validate_child(child, &parent, &lineage)
}

fn validate_parent(
    parent: &RunInspection,
    lineage: &RunLineageRecord,
) -> Result<(), RunStoreError> {
    if !matches!(parent.recovery.state, RunRecoveryState::Terminal { .. }) {
        return Err(corrupt("Run lineage parent is no longer terminal"));
    }
    let source = parent
        .events
        .first()
        .ok_or_else(|| corrupt("Run lineage parent has no source event"))?;
    if !matches!(source.kind, RuntimeEventKind::RunStarted { .. }) {
        return Err(corrupt(
            "Run lineage parent does not begin with run_started",
        ));
    }
    let digest =
        source_event_sha256(&parent.run, source).map_err(|error| corrupt(&error.message))?;
    if digest != lineage.source_event_sha256 {
        return Err(corrupt("Run lineage source event binding diverged"));
    }
    Ok(())
}

fn validate_child(
    child: &RunInspection,
    parent: &RunInspection,
    lineage: &RunLineageRecord,
) -> Result<(), RunStoreError> {
    if child.run.conversation_id != parent.run.conversation_id
        || child.run.prompt_id != parent.run.prompt_id
        || child.run.project_id != parent.run.project_id
        || child.run.execution != parent.run.execution
        || child.run.created_at_ms != lineage.created_at_ms
    {
        return Err(corrupt("Run branch input diverges from its parent"));
    }
    let parent_prompt = start_prompt(parent)?;
    let child_prompt = start_prompt(child)?;
    if parent_prompt != child_prompt {
        return Err(corrupt("Run branch root prompt diverges from its parent"));
    }
    Ok(())
}

fn start_prompt(inspection: &RunInspection) -> Result<&str, RunStoreError> {
    match inspection.events.first().map(|event| &event.kind) {
        Some(RuntimeEventKind::RunStarted { prompt }) => Ok(prompt),
        _ => Err(corrupt("Run branch is missing its root run_started event")),
    }
}

fn lineage_row(row: &Row<'_>) -> rusqlite::Result<RunLineageRecord> {
    let relation: String = row.get(2)?;
    if relation != "branch" {
        return Err(conversion_error(2, "invalid Run lineage relation"));
    }
    let mode: String = row.get(3)?;
    let source_digest: Vec<u8> = row.get(6)?;
    let lineage_digest: Vec<u8> = row.get(7)?;
    Ok(RunLineageRecord {
        v: u16_value(row, 1)?,
        child_run_id: row.get(0)?,
        parent_run_id: row.get(4)?,
        branch_mode: match mode.as_str() {
            "root_input" => RunBranchMode::RootInput,
            _ => return Err(conversion_error(3, "invalid Run branch mode")),
        },
        source_event_seq: u64_value(row, 5)?,
        source_event_sha256: encode_digest(6, &source_digest)?,
        lineage_sha256: encode_digest(7, &lineage_digest)?,
        created_at_ms: u64_value(row, 8)?,
    })
}

fn validate_record(record: RunLineageRecord) -> Result<RunLineageRecord, RunStoreError> {
    record.validate().map_err(|error| corrupt(&error.message))?;
    Ok(record)
}

fn encode_digest(index: usize, bytes: &[u8]) -> rusqlite::Result<String> {
    if bytes.len() != 32 {
        return Err(conversion_error(index, "invalid Run lineage digest length"));
    }
    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok(output)
}

fn u16_value(row: &Row<'_>, index: usize) -> rusqlite::Result<u16> {
    u16::try_from(row.get::<_, i64>(index)?)
        .map_err(|_| conversion_error(index, "invalid Run lineage version"))
}

fn u64_value(row: &Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?)
        .map_err(|_| conversion_error(index, "invalid Run lineage integer"))
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

fn read_error(error: &rusqlite::Error) -> RunStoreError {
    match error {
        rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::InvalidColumnType(..)
        | rusqlite::Error::IntegralValueOutOfRange(..) => corrupt(&error.to_string()),
        _ => RunStoreError::Unavailable {
            message: error.to_string(),
        },
    }
}

fn unavailable(error: &rusqlite::Error) -> RunStoreError {
    RunStoreError::Unavailable {
        message: error.to_string(),
    }
}

fn corrupt(message: &str) -> RunStoreError {
    RunStoreError::Corrupt {
        message: message.into(),
    }
}
