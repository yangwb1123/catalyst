use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::HubStoreError;

use super::super::read_error;

const METADATA_COLUMNS: &str = "id,graph_run_id,contract_version,node_id,attempt,\
 control_snapshot_sha256,contract_sha256,contract_bytes,request_sha256,\
 project_lane_sha256,expected_last_event_seq,expected_last_event_sha256,created_at_ms";
const STORED_COLUMNS: &str = "id,graph_run_id,contract_version,node_id,attempt,\
 control_snapshot_sha256,contract_sha256,contract_bytes,request_sha256,\
 project_lane_sha256,expected_last_event_seq,expected_last_event_sha256,created_at_ms,\
 idempotency_key,contract_blob";

#[derive(Clone)]
pub(super) struct RawContractMetadata {
    pub id: String,
    pub graph_run_id: String,
    pub contract_version: i64,
    pub node_id: String,
    pub attempt: i64,
    pub control_snapshot_sha256: Vec<u8>,
    pub contract_sha256: Vec<u8>,
    pub contract_bytes: i64,
    pub request_sha256: Vec<u8>,
    pub project_lane_sha256: Vec<u8>,
    pub expected_last_event_seq: i64,
    pub expected_last_event_sha256: Vec<u8>,
    pub created_at_ms: i64,
}

pub(super) struct RawStoredContract {
    pub metadata: RawContractMetadata,
    pub idempotency_key: String,
    pub contract_blob: Vec<u8>,
}

pub(super) fn find_by_id(
    connection: &Connection,
    contract_id: &str,
) -> Result<Option<RawStoredContract>, HubStoreError> {
    query_stored(connection, "id", contract_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredContract>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn find_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawStoredContract>, HubStoreError> {
    query_stored(connection, "graph_run_id", graph_run_id)
}

pub(super) fn query_metadata(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawContractMetadata>, HubStoreError> {
    match graph_run_id {
        Some(id) => query_metadata_with(
            connection,
            "WHERE graph_run_id = ?1 ORDER BY created_at_ms DESC,id DESC LIMIT ?2",
            params![id, limit],
        ),
        None => query_metadata_with(
            connection,
            "ORDER BY created_at_ms DESC,id DESC LIMIT ?1",
            [limit],
        ),
    }
}

fn query_stored(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawStoredContract>, HubStoreError> {
    let predicate = match column {
        "id" => "id = ?1",
        "idempotency_key" => "idempotency_key = ?1",
        "graph_run_id" => "graph_run_id = ?1",
        _ => return Err(corrupt("unsupported Node Execution Contract lookup")),
    };
    let sql = format!(
        "SELECT {STORED_COLUMNS}
         FROM group_agent_graph_node_execution_contracts WHERE {predicate}"
    );
    connection
        .query_row(&sql, [value], stored_row)
        .optional()
        .map_err(read_error)
}

fn query_metadata_with<P>(
    connection: &Connection,
    suffix: &str,
    parameters: P,
) -> Result<Vec<RawContractMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!(
        "SELECT {METADATA_COLUMNS}
         FROM group_agent_graph_node_execution_contracts {suffix}"
    );
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredContract> {
    Ok(RawStoredContract {
        metadata: metadata_row(row)?,
        idempotency_key: row.get(13)?,
        contract_blob: row.get(14)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawContractMetadata> {
    Ok(RawContractMetadata {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        contract_version: row.get(2)?,
        node_id: row.get(3)?,
        attempt: row.get(4)?,
        control_snapshot_sha256: row.get(5)?,
        contract_sha256: row.get(6)?,
        contract_bytes: row.get(7)?,
        request_sha256: row.get(8)?,
        project_lane_sha256: row.get(9)?,
        expected_last_event_seq: row.get(10)?,
        expected_last_event_sha256: row.get(11)?,
        created_at_ms: row.get(12)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
