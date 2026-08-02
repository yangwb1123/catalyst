use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::HubStoreError;

use super::super::{group_run_codec, read_error};

pub(super) const TABLE: &str = "group_agent_graph_scheduled_node_provider_requests";

const METADATA_COLUMNS: &str = "id,graph_run_id,schedule_id,scheduled_contract_id,\
 provider_request_version,codec_protocol_version,execution_ordinal,node_id,attempt,\
 scheduled_contract_sha256,logical_request_id,logical_request_sha256,schedule_sha256,\
 project_lane_sha256,provider_kind,endpoint,model,destination_sha256,\
 pricing_snapshot_sha256,provider_request_bytes,provider_request_sha256,\
 prepared_request_sha256,expected_last_event_seq,expected_last_event_sha256,\
 provider_request_prepared,provider_request_sent,lifecycle_contract_admitted,\
 execution_authority_released,dispatch_authority_released,project_lane_claimed,\
 progress_observed,successor_advance_authorized,created_at_ms";

const STORED_COLUMNS: &str = "id,graph_run_id,schedule_id,scheduled_contract_id,\
 provider_request_version,codec_protocol_version,execution_ordinal,node_id,attempt,\
 scheduled_contract_sha256,logical_request_id,logical_request_sha256,schedule_sha256,\
 project_lane_sha256,provider_kind,endpoint,model,destination_sha256,\
 pricing_snapshot_sha256,provider_request_bytes,provider_request_sha256,\
 prepared_request_sha256,expected_last_event_seq,expected_last_event_sha256,\
 provider_request_prepared,provider_request_sent,lifecycle_contract_admitted,\
 execution_authority_released,dispatch_authority_released,project_lane_claimed,\
 progress_observed,successor_advance_authorized,created_at_ms,idempotency_key,\
 provider_request_blob";

#[derive(Clone)]
pub(super) struct RawRequestMetadata {
    pub id: String,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub scheduled_contract_id: String,
    pub provider_request_version: i64,
    pub codec_protocol_version: i64,
    pub execution_ordinal: i64,
    pub node_id: String,
    pub attempt: i64,
    pub scheduled_contract_sha256: Vec<u8>,
    pub logical_request_id: String,
    pub logical_request_sha256: Vec<u8>,
    pub schedule_sha256: Vec<u8>,
    pub project_lane_sha256: Vec<u8>,
    pub provider_kind: String,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: Vec<u8>,
    pub pricing_snapshot_sha256: Vec<u8>,
    pub provider_request_bytes: i64,
    pub provider_request_sha256: Vec<u8>,
    pub prepared_request_sha256: Vec<u8>,
    pub expected_last_event_seq: i64,
    pub expected_last_event_sha256: Vec<u8>,
    pub provider_request_prepared: i64,
    pub provider_request_sent: i64,
    pub lifecycle_contract_admitted: i64,
    pub execution_authority_released: i64,
    pub dispatch_authority_released: i64,
    pub project_lane_claimed: i64,
    pub progress_observed: i64,
    pub successor_advance_authorized: i64,
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
    query_one(connection, "id", id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_one(connection, "idempotency_key", key)
}

pub(super) fn find_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_one(connection, "graph_run_id", graph_run_id)
}

pub(super) fn find_by_schedule(
    connection: &Connection,
    schedule_id: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_one(connection, "schedule_id", schedule_id)
}

pub(super) fn find_by_contract(
    connection: &Connection,
    scheduled_contract_id: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_one(connection, "scheduled_contract_id", scheduled_contract_id)
}

pub(super) fn find_by_logical_request(
    connection: &Connection,
    logical_request_id: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    query_one(connection, "logical_request_id", logical_request_id)
}

pub(super) fn find_by_run_node_attempt(
    connection: &Connection,
    graph_run_id: &str,
    node_id: &str,
    attempt: u16,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    connection
        .query_row(
            &format!(
                "SELECT {STORED_COLUMNS} FROM {TABLE} \
                 WHERE graph_run_id=?1 AND node_id=?2 AND attempt=?3"
            ),
            params![graph_run_id, node_id, i64::from(attempt)],
            stored_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn find_by_schedule_ordinal_attempt(
    connection: &Connection,
    schedule_id: &str,
    execution_ordinal: usize,
    attempt: u16,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    let Ok(ordinal) = i64::try_from(execution_ordinal) else {
        return Ok(None);
    };
    connection
        .query_row(
            &format!(
                "SELECT {STORED_COLUMNS} FROM {TABLE} \
                 WHERE schedule_id=?1 AND execution_ordinal=?2 AND attempt=?3"
            ),
            params![schedule_id, ordinal, i64::from(attempt)],
            stored_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn query_metadata(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawRequestMetadata>, HubStoreError> {
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

pub(super) fn exists_for_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<bool, HubStoreError> {
    connection
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {TABLE} WHERE graph_run_id=?1)"),
            [graph_run_id],
            |row| row.get(0),
        )
        .map_err(read_error)
}

fn query_one(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawStoredRequest>, HubStoreError> {
    let predicate = match column {
        "id" => "id=?1",
        "idempotency_key" => "idempotency_key=?1",
        "graph_run_id" => "graph_run_id=?1",
        "schedule_id" => "schedule_id=?1",
        "scheduled_contract_id" => "scheduled_contract_id=?1",
        "logical_request_id" => "logical_request_id=?1",
        _ => return Err(corrupt("unsupported scheduled provider-request lookup")),
    };
    connection
        .query_row(
            &format!("SELECT {STORED_COLUMNS} FROM {TABLE} WHERE {predicate}"),
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
) -> Result<Vec<RawRequestMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection
        .prepare(&format!("SELECT {METADATA_COLUMNS} FROM {TABLE} {suffix}"))
        .map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredRequest> {
    Ok(RawStoredRequest {
        metadata: metadata_row(row)?,
        idempotency_key: row.get(33)?,
        provider_request_blob: row.get(34)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawRequestMetadata> {
    Ok(RawRequestMetadata {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        schedule_id: row.get(2)?,
        scheduled_contract_id: row.get(3)?,
        provider_request_version: row.get(4)?,
        codec_protocol_version: row.get(5)?,
        execution_ordinal: row.get(6)?,
        node_id: row.get(7)?,
        attempt: row.get(8)?,
        scheduled_contract_sha256: row.get(9)?,
        logical_request_id: row.get(10)?,
        logical_request_sha256: row.get(11)?,
        schedule_sha256: row.get(12)?,
        project_lane_sha256: row.get(13)?,
        provider_kind: row.get(14)?,
        endpoint: row.get(15)?,
        model: row.get(16)?,
        destination_sha256: row.get(17)?,
        pricing_snapshot_sha256: row.get(18)?,
        provider_request_bytes: row.get(19)?,
        provider_request_sha256: row.get(20)?,
        prepared_request_sha256: row.get(21)?,
        expected_last_event_seq: row.get(22)?,
        expected_last_event_sha256: row.get(23)?,
        provider_request_prepared: row.get(24)?,
        provider_request_sent: row.get(25)?,
        lifecycle_contract_admitted: row.get(26)?,
        execution_authority_released: row.get(27)?,
        dispatch_authority_released: row.get(28)?,
        project_lane_claimed: row.get(29)?,
        progress_observed: row.get(30)?,
        successor_advance_authorized: row.get(31)?,
        created_at_ms: row.get(32)?,
    })
}

pub(super) fn valid_lookup_id(value: &str) -> bool {
    group_run_codec::valid_text(
        value,
        crate::runtime_domain::MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    )
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
