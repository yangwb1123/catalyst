use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::HubStoreError;

use super::super::super::read_error;

const METADATA_COLUMNS: &str = "id,graph_run_id,graph_id,schedule_version,\
 control_snapshot_sha256,schedule_sha256,schedule_bytes,node_count,wave_count,\
 expected_last_event_seq,expected_last_event_sha256,execution_contract_present,\
 dispatch_authority_released,created_at_ms";

const STORED_COLUMNS: &str = "id,graph_run_id,graph_id,schedule_version,\
 control_snapshot_sha256,schedule_sha256,schedule_bytes,node_count,wave_count,\
 expected_last_event_seq,expected_last_event_sha256,execution_contract_present,\
 dispatch_authority_released,created_at_ms,scheduler_protocol_version,\
 execution_schedule_protocol_version,initial_node,progress_observed,successor_advanced,\
 idempotency_key,schedule_blob";

#[derive(Clone)]
pub(super) struct RawScheduleMetadata {
    pub id: String,
    pub graph_run_id: String,
    pub graph_id: String,
    pub schedule_version: i64,
    pub control_snapshot_sha256: Vec<u8>,
    pub schedule_sha256: Vec<u8>,
    pub schedule_bytes: i64,
    pub node_count: i64,
    pub wave_count: i64,
    pub expected_last_event_seq: i64,
    pub expected_last_event_sha256: Vec<u8>,
    pub execution_contract_present: i64,
    pub dispatch_authority_released: i64,
    pub created_at_ms: i64,
}

pub(super) struct RawStoredSchedule {
    pub metadata: RawScheduleMetadata,
    pub scheduler_protocol_version: i64,
    pub execution_schedule_protocol_version: i64,
    pub initial_node: String,
    pub progress_observed: i64,
    pub successor_advanced: i64,
    pub idempotency_key: String,
    pub schedule_blob: Vec<u8>,
}

pub(super) fn find_by_id(
    connection: &Connection,
    schedule_id: &str,
) -> Result<Option<RawStoredSchedule>, HubStoreError> {
    query_one(connection, "id", schedule_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredSchedule>, HubStoreError> {
    query_one(connection, "idempotency_key", key)
}

pub(super) fn find_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawStoredSchedule>, HubStoreError> {
    query_one(connection, "graph_run_id", graph_run_id)
}

pub(super) fn query_metadata(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawScheduleMetadata>, HubStoreError> {
    match graph_run_id {
        Some(id) => query_many(
            connection,
            "WHERE graph_run_id=?1 ORDER BY created_at_ms DESC,id DESC LIMIT ?2",
            params![id, limit],
        ),
        None => query_many(
            connection,
            "ORDER BY created_at_ms DESC,id DESC LIMIT ?1",
            [limit],
        ),
    }
}

fn query_one(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawStoredSchedule>, HubStoreError> {
    let predicate = match column {
        "id" => "id=?1",
        "idempotency_key" => "idempotency_key=?1",
        "graph_run_id" => "graph_run_id=?1",
        _ => return Err(corrupt("unsupported execution schedule lookup")),
    };
    connection
        .query_row(
            &format!(
                "SELECT {STORED_COLUMNS} FROM group_agent_graph_execution_schedules \
                 WHERE {predicate}"
            ),
            [value],
            stored_row,
        )
        .optional()
        .map_err(read_error)
}

fn query_many<P>(
    connection: &Connection,
    suffix: &str,
    parameters: P,
) -> Result<Vec<RawScheduleMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection
        .prepare(&format!(
            "SELECT {METADATA_COLUMNS} FROM group_agent_graph_execution_schedules {suffix}"
        ))
        .map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredSchedule> {
    Ok(RawStoredSchedule {
        metadata: metadata_row(row)?,
        scheduler_protocol_version: row.get(14)?,
        execution_schedule_protocol_version: row.get(15)?,
        initial_node: row.get(16)?,
        progress_observed: row.get(17)?,
        successor_advanced: row.get(18)?,
        idempotency_key: row.get(19)?,
        schedule_blob: row.get(20)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawScheduleMetadata> {
    Ok(RawScheduleMetadata {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        graph_id: row.get(2)?,
        schedule_version: row.get(3)?,
        control_snapshot_sha256: row.get(4)?,
        schedule_sha256: row.get(5)?,
        schedule_bytes: row.get(6)?,
        node_count: row.get(7)?,
        wave_count: row.get(8)?,
        expected_last_event_seq: row.get(9)?,
        expected_last_event_sha256: row.get(10)?,
        execution_contract_present: row.get(11)?,
        dispatch_authority_released: row.get(12)?,
        created_at_ms: row.get(13)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
