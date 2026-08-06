use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::{HubStoreError, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES};

use super::super::{group_run_codec, read_error};

pub(super) const TABLE: &str = "group_agent_graph_scheduled_node_successor_candidates";

const METADATA_COLUMNS: &str = "id,graph_run_id,schedule_id,contract_version,node_id,\
 execution_ordinal,attempt,control_snapshot_sha256,schedule_sha256,contract_sha256,\
 contract_bytes,request_id,request_sha256,project_lane_sha256,expected_last_event_seq,\
 expected_last_event_sha256,predecessor_receipt_count,lifecycle_contract_admitted,\
 provider_request_present,execution_authority_released,dispatch_authority_released,\
 progress_observed,successor_advance_authorized,created_at_ms";

const STORED_COLUMNS: &str = "id,graph_run_id,schedule_id,contract_version,node_id,\
 execution_ordinal,attempt,control_snapshot_sha256,schedule_sha256,contract_sha256,\
 contract_bytes,request_id,request_sha256,project_lane_sha256,expected_last_event_seq,\
 expected_last_event_sha256,predecessor_receipt_count,lifecycle_contract_admitted,\
 provider_request_present,execution_authority_released,dispatch_authority_released,\
 progress_observed,successor_advance_authorized,created_at_ms,graph_id,\
 scheduler_protocol_version,node_execution_protocol_version,\
 execution_schedule_protocol_version,contract_scope,authored_node_index,\
 topology_wave_index,required_predecessor_node_count,idempotency_key,contract_blob";

#[derive(Clone)]
pub(in crate::sqlite_hub) struct RawCandidateMetadata {
    pub id: String,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub contract_version: i64,
    pub node_id: String,
    pub execution_ordinal: i64,
    pub attempt: i64,
    pub control_snapshot_sha256: Vec<u8>,
    pub schedule_sha256: Vec<u8>,
    pub contract_sha256: Vec<u8>,
    pub contract_bytes: i64,
    pub request_id: String,
    pub request_sha256: Vec<u8>,
    pub project_lane_sha256: Vec<u8>,
    pub expected_last_event_seq: i64,
    pub expected_last_event_sha256: Vec<u8>,
    pub predecessor_receipt_count: i64,
    pub lifecycle_contract_admitted: i64,
    pub provider_request_present: i64,
    pub execution_authority_released: i64,
    pub dispatch_authority_released: i64,
    pub progress_observed: i64,
    pub successor_advance_authorized: i64,
    pub created_at_ms: i64,
}

pub(in crate::sqlite_hub) struct RawStoredCandidate {
    pub(in crate::sqlite_hub) metadata: RawCandidateMetadata,
    pub graph_id: String,
    pub scheduler_protocol_version: i64,
    pub node_execution_protocol_version: i64,
    pub execution_schedule_protocol_version: i64,
    pub contract_scope: String,
    pub authored_node_index: i64,
    pub topology_wave_index: i64,
    pub required_predecessor_node_count: i64,
    pub idempotency_key: String,
    pub contract_blob: Vec<u8>,
}

pub(super) fn find_by_id(
    connection: &Connection,
    contract_id: &str,
) -> Result<Option<RawStoredCandidate>, HubStoreError> {
    query_one(connection, "id", &[contract_id])
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredCandidate>, HubStoreError> {
    query_one(connection, "idempotency_key", &[key])
}

/// Returns every successor candidate admitted for one Graph Run (v20 allows
/// one candidate per node), in creation order.
pub(in crate::sqlite_hub) fn find_all_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Vec<RawStoredCandidate>, HubStoreError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {STORED_COLUMNS} FROM {TABLE} WHERE graph_run_id=?1"
        ))
        .map_err(read_error)?;
    let rows = statement
        .query_map([graph_run_id], stored_row)
        .map_err(read_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(read_error)
}

pub(super) fn find_by_schedule(
    connection: &Connection,
    schedule_id: &str,
) -> Result<Option<RawStoredCandidate>, HubStoreError> {
    query_one(connection, "schedule_id", &[schedule_id])
}

pub(super) fn find_by_request_id(
    connection: &Connection,
    request_id: &str,
) -> Result<Option<RawStoredCandidate>, HubStoreError> {
    query_one(connection, "request_id", &[request_id])
}

pub(super) fn find_by_run_node_attempt(
    connection: &Connection,
    graph_run_id: &str,
    node_id: &str,
    attempt: u16,
) -> Result<Option<RawStoredCandidate>, HubStoreError> {
    let attempt = i64::from(attempt);
    connection
        .query_row(
            &format!(
                "SELECT {STORED_COLUMNS} FROM {TABLE} \
                 WHERE graph_run_id=?1 AND node_id=?2 AND attempt=?3"
            ),
            params![graph_run_id, node_id, attempt],
            stored_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn find_by_schedule_ordinal_attempt(
    connection: &Connection,
    schedule_id: &str,
    ordinal: usize,
    attempt: u16,
) -> Result<Option<RawStoredCandidate>, HubStoreError> {
    let ordinal = i64::try_from(ordinal).map_err(|error| corrupt(&error.to_string()))?;
    let attempt = i64::from(attempt);
    connection
        .query_row(
            &format!(
                "SELECT {STORED_COLUMNS} FROM {TABLE} \
                 WHERE schedule_id=?1 AND execution_ordinal=?2 AND attempt=?3"
            ),
            params![schedule_id, ordinal, attempt],
            stored_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn query_metadata(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawCandidateMetadata>, HubStoreError> {
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
    values: &[&str],
) -> Result<Option<RawStoredCandidate>, HubStoreError> {
    let predicate = match column {
        "id" => "id=?1",
        "idempotency_key" => "idempotency_key=?1",
        "graph_run_id" => "graph_run_id=?1",
        "schedule_id" => "schedule_id=?1",
        "request_id" => "request_id=?1",
        _ => return Err(corrupt("unsupported scheduled-node contract lookup")),
    };
    let Some(value) = values.first() else {
        return Err(corrupt("scheduled-node contract lookup has no value"));
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
) -> Result<Vec<RawCandidateMetadata>, HubStoreError>
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

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredCandidate> {
    Ok(RawStoredCandidate {
        metadata: metadata_row(row)?,
        graph_id: row.get(24)?,
        scheduler_protocol_version: row.get(25)?,
        node_execution_protocol_version: row.get(26)?,
        execution_schedule_protocol_version: row.get(27)?,
        contract_scope: row.get(28)?,
        authored_node_index: row.get(29)?,
        topology_wave_index: row.get(30)?,
        required_predecessor_node_count: row.get(31)?,
        idempotency_key: row.get(32)?,
        contract_blob: row.get(33)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawCandidateMetadata> {
    Ok(RawCandidateMetadata {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        schedule_id: row.get(2)?,
        contract_version: row.get(3)?,
        node_id: row.get(4)?,
        execution_ordinal: row.get(5)?,
        attempt: row.get(6)?,
        control_snapshot_sha256: row.get(7)?,
        schedule_sha256: row.get(8)?,
        contract_sha256: row.get(9)?,
        contract_bytes: row.get(10)?,
        request_id: row.get(11)?,
        request_sha256: row.get(12)?,
        project_lane_sha256: row.get(13)?,
        expected_last_event_seq: row.get(14)?,
        expected_last_event_sha256: row.get(15)?,
        predecessor_receipt_count: row.get(16)?,
        lifecycle_contract_admitted: row.get(17)?,
        provider_request_present: row.get(18)?,
        execution_authority_released: row.get(19)?,
        dispatch_authority_released: row.get(20)?,
        progress_observed: row.get(21)?,
        successor_advance_authorized: row.get(22)?,
        created_at_ms: row.get(23)?,
    })
}

pub(super) fn valid_lookup_id(value: &str) -> bool {
    group_run_codec::valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
