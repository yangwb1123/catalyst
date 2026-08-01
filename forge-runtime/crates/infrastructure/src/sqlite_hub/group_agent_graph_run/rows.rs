use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::MAX_GROUP_AGENT_GRAPH_RUN_EVENTS;

use super::{super::read_error, HubStoreError};

const METADATA_COLUMNS: &str = "id,graph_id,run_version,status,source_snapshot_sha256,\
 graph_manifest_sha256,scheduler_protocol_version,plan_sha256,plan_bytes,node_count,wave_count,\
 execution_contract_present,dispatch_request_present,dispatch_authority_released,last_event_seq,\
 journal_bytes,created_at_ms";
const STORED_COLUMNS: &str = "id,graph_id,run_version,status,source_snapshot_sha256,\
 graph_manifest_sha256,scheduler_protocol_version,plan_sha256,plan_bytes,node_count,wave_count,\
 execution_contract_present,dispatch_request_present,dispatch_authority_released,last_event_seq,\
 journal_bytes,created_at_ms,idempotency_key,plan_blob";

#[derive(Clone)]
pub(super) struct RawRunMetadata {
    pub id: String,
    pub graph_id: String,
    pub run_version: i64,
    pub status: String,
    pub source_snapshot_sha256: Vec<u8>,
    pub graph_manifest_sha256: Vec<u8>,
    pub scheduler_protocol_version: i64,
    pub plan_sha256: Vec<u8>,
    pub plan_bytes: i64,
    pub node_count: i64,
    pub wave_count: i64,
    pub execution_contract_present: i64,
    pub dispatch_request_present: i64,
    pub dispatch_authority_released: i64,
    pub last_event_seq: i64,
    pub journal_bytes: i64,
    pub created_at_ms: i64,
}

pub(super) struct RawStoredRun {
    pub metadata: RawRunMetadata,
    pub idempotency_key: String,
    pub plan_blob: Vec<u8>,
}

pub(super) struct RawEvent {
    pub graph_run_id: String,
    pub sequence: i64,
    pub event_version: i64,
    pub kind: String,
    pub event_blob: Vec<u8>,
    pub event_bytes: i64,
    pub event_sha256: Vec<u8>,
    pub created_at_ms: i64,
}

pub(super) fn find_by_id(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawStoredRun>, HubStoreError> {
    query_stored(connection, "id", graph_run_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredRun>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn query_metadata(
    connection: &Connection,
    graph_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawRunMetadata>, HubStoreError> {
    match graph_id {
        Some(id) => query_metadata_with(
            connection,
            "WHERE graph_id = ?1 ORDER BY created_at_ms DESC,id DESC LIMIT ?2",
            params![id, limit],
        ),
        None => query_metadata_with(
            connection,
            "ORDER BY created_at_ms DESC,id DESC LIMIT ?1",
            [limit],
        ),
    }
}

pub(super) fn load_events(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Vec<RawEvent>, HubStoreError> {
    let limit = i64::try_from(MAX_GROUP_AGENT_GRAPH_RUN_EVENTS + 1)
        .map_err(|error| corrupt(&format!("invalid Graph Run event read limit: {error}")))?;
    let mut statement = connection
        .prepare(
            "SELECT graph_run_id,seq,event_version,kind,event_blob,event_bytes,
                    event_sha256,created_at_ms
             FROM group_agent_graph_run_events
             WHERE graph_run_id = ?1 ORDER BY seq LIMIT ?2",
        )
        .map_err(read_error)?;
    statement
        .query_map(params![graph_run_id, limit], event_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn query_stored(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawStoredRun>, HubStoreError> {
    let sql = match column {
        "id" => format!("SELECT {STORED_COLUMNS} FROM group_agent_graph_runs WHERE id = ?1"),
        "idempotency_key" => format!(
            "SELECT {STORED_COLUMNS} FROM group_agent_graph_runs WHERE idempotency_key = ?1"
        ),
        _ => return Err(corrupt("unsupported Group Agent Graph Run lookup")),
    };
    connection
        .query_row(&sql, [value], stored_row)
        .optional()
        .map_err(read_error)
}

fn query_metadata_with<P>(
    connection: &Connection,
    suffix: &str,
    parameters: P,
) -> Result<Vec<RawRunMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!("SELECT {METADATA_COLUMNS} FROM group_agent_graph_runs {suffix}");
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredRun> {
    Ok(RawStoredRun {
        metadata: metadata_row(row)?,
        idempotency_key: row.get(17)?,
        plan_blob: row.get(18)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawRunMetadata> {
    Ok(RawRunMetadata {
        id: row.get(0)?,
        graph_id: row.get(1)?,
        run_version: row.get(2)?,
        status: row.get(3)?,
        source_snapshot_sha256: row.get(4)?,
        graph_manifest_sha256: row.get(5)?,
        scheduler_protocol_version: row.get(6)?,
        plan_sha256: row.get(7)?,
        plan_bytes: row.get(8)?,
        node_count: row.get(9)?,
        wave_count: row.get(10)?,
        execution_contract_present: row.get(11)?,
        dispatch_request_present: row.get(12)?,
        dispatch_authority_released: row.get(13)?,
        last_event_seq: row.get(14)?,
        journal_bytes: row.get(15)?,
        created_at_ms: row.get(16)?,
    })
}

fn event_row(row: &Row<'_>) -> rusqlite::Result<RawEvent> {
    Ok(RawEvent {
        graph_run_id: row.get(0)?,
        sequence: row.get(1)?,
        event_version: row.get(2)?,
        kind: row.get(3)?,
        event_blob: row.get(4)?,
        event_bytes: row.get(5)?,
        event_sha256: row.get(6)?,
        created_at_ms: row.get(7)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
