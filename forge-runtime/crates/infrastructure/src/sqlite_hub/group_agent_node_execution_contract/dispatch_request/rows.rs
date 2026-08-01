use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::HubStoreError;

use super::super::super::read_error;

const METADATA_COLUMNS: &str = "id,graph_run_id,contract_id,request_version,\
 codec_protocol_version,node_id,attempt,contract_sha256,request_sha256,\
 project_lane_sha256,provider_kind,endpoint,model,destination_sha256,\
 pricing_snapshot_sha256,provider_request_bytes,provider_request_sha256,\
 dispatch_request_sha256,expected_last_event_seq,expected_last_event_sha256,created_at_ms";
const STORED_COLUMNS: &str = "id,graph_run_id,contract_id,request_version,\
 codec_protocol_version,node_id,attempt,contract_sha256,request_sha256,\
 project_lane_sha256,provider_kind,endpoint,model,destination_sha256,\
 pricing_snapshot_sha256,provider_request_bytes,provider_request_sha256,\
 dispatch_request_sha256,expected_last_event_seq,expected_last_event_sha256,created_at_ms,\
 idempotency_key,provider_request_blob";

#[derive(Clone)]
pub(super) struct RawRequestMetadata {
    pub id: String,
    pub graph_run_id: String,
    pub contract_id: String,
    pub request_version: i64,
    pub codec_protocol_version: i64,
    pub node_id: String,
    pub attempt: i64,
    pub contract_sha256: Vec<u8>,
    pub request_sha256: Vec<u8>,
    pub project_lane_sha256: Vec<u8>,
    pub provider_kind: String,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: Vec<u8>,
    pub pricing_snapshot_sha256: Vec<u8>,
    pub provider_request_bytes: i64,
    pub provider_request_sha256: Vec<u8>,
    pub dispatch_request_sha256: Vec<u8>,
    pub expected_last_event_seq: i64,
    pub expected_last_event_sha256: Vec<u8>,
    pub created_at_ms: i64,
}

pub(super) struct RawStoredRequest {
    pub metadata: RawRequestMetadata,
    pub idempotency_key: String,
    pub provider_request_blob: Vec<u8>,
}

pub(super) fn find_by_id(
    connection: &Connection,
    id: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_stored(connection, "id", id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn find_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_stored(connection, "graph_run_id", graph_run_id)
}

pub(super) fn find_by_contract(
    connection: &Connection,
    contract_id: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_stored(connection, "contract_id", contract_id)
}

pub(super) fn query_metadata(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawRequestMetadata>, HubStoreError> {
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
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    let predicate = match column {
        "id" => "id = ?1",
        "idempotency_key" => "idempotency_key = ?1",
        "graph_run_id" => "graph_run_id = ?1",
        "contract_id" => "contract_id = ?1",
        _ => return Err(corrupt("unsupported Node Dispatch Request lookup")),
    };
    let sql = format!(
        "SELECT {STORED_COLUMNS}
         FROM group_agent_graph_node_dispatch_requests WHERE {predicate}"
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
) -> Result<Vec<RawRequestMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!(
        "SELECT {METADATA_COLUMNS}
         FROM group_agent_graph_node_dispatch_requests {suffix}"
    );
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredRequest> {
    Ok(RawStoredRequest {
        metadata: metadata_row(row)?,
        idempotency_key: row.get(21)?,
        provider_request_blob: row.get(22)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawRequestMetadata> {
    Ok(RawRequestMetadata {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        contract_id: row.get(2)?,
        request_version: row.get(3)?,
        codec_protocol_version: row.get(4)?,
        node_id: row.get(5)?,
        attempt: row.get(6)?,
        contract_sha256: row.get(7)?,
        request_sha256: row.get(8)?,
        project_lane_sha256: row.get(9)?,
        provider_kind: row.get(10)?,
        endpoint: row.get(11)?,
        model: row.get(12)?,
        destination_sha256: row.get(13)?,
        pricing_snapshot_sha256: row.get(14)?,
        provider_request_bytes: row.get(15)?,
        provider_request_sha256: row.get(16)?,
        dispatch_request_sha256: row.get(17)?,
        expected_last_event_seq: row.get(18)?,
        expected_last_event_sha256: row.get(19)?,
        created_at_ms: row.get(20)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
