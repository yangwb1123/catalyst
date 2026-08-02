use rusqlite::{Connection, OptionalExtension, Row};

use crate::runtime_domain::HubStoreError;

use super::super::read_error;

pub(super) struct RawClaim {
    pub dispatch_id: String,
    pub version: i64,
    pub graph_run_id: String,
    pub authorization_id: String,
    pub authorization_sha256: Vec<u8>,
    pub dispatch_request_id: String,
    pub dispatch_request_sha256: Vec<u8>,
    pub logical_request_sha256: Vec<u8>,
    pub request_body_sha256: Vec<u8>,
    pub request_body_bytes: i64,
    pub pricing_snapshot_sha256: Vec<u8>,
    pub node_id: String,
    pub attempt: i64,
    pub max_cost_usd_micros: i64,
    pub consent_contract_version: i64,
    pub lane_ownership_id: String,
    pub project_lane_sha256: Vec<u8>,
    pub expected_last_event_seq: i64,
    pub expected_last_event_sha256: Vec<u8>,
    pub claim_event_sha256: Vec<u8>,
    pub claim_blob: Vec<u8>,
    pub claim_bytes: i64,
    pub released_at_ms: i64,
}

pub(super) struct RawLane {
    pub lane_ownership_id: String,
    pub version: i64,
    pub project_lane_sha256: Vec<u8>,
    pub graph_run_id: String,
    pub node_id: String,
    pub attempt: i64,
    pub dispatch_id: String,
    pub claim_event_sha256: Vec<u8>,
    pub lane_blob: Vec<u8>,
    pub lane_bytes: i64,
    pub claimed_at_ms: i64,
}

pub(super) struct RawArtifact {
    pub id: String,
    pub graph_run_id: String,
    pub dispatch_id: String,
    pub version: i64,
    pub kind: String,
    pub node_id: String,
    pub attempt: i64,
    pub claim_event_sha256: Vec<u8>,
    pub lane_ownership_id: String,
    pub provider_polling_began: i64,
    pub terminal_observed: i64,
    pub true_eof_observed: i64,
    pub retry_authorized: i64,
    pub artifact_blob: Vec<u8>,
    pub artifact_blob_bytes: i64,
    pub artifact_bytes: i64,
    pub artifact_sha256: Vec<u8>,
    pub created_at_ms: i64,
}

pub(super) struct RawReceipt {
    pub id: String,
    pub graph_run_id: String,
    pub dispatch_id: String,
    pub artifact_id: String,
    pub version: i64,
    pub graph_status: String,
    pub claim_event_sha256: Vec<u8>,
    pub lane_ownership_id: String,
    pub artifact_sha256: Vec<u8>,
    pub retry_authorized: i64,
    pub lane_release_authorized: i64,
    pub receipt_blob: Vec<u8>,
    pub receipt_bytes: i64,
    pub receipt_sha256: Vec<u8>,
    pub terminal_at_ms: i64,
}

pub(super) fn claim_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawClaim>, HubStoreError> {
    connection
        .query_row(
            "SELECT dispatch_id,claim_version,graph_run_id,authorization_id,
                    authorization_sha256,dispatch_request_id,dispatch_request_sha256,
                    logical_request_sha256,request_body_sha256,request_body_bytes,
                    pricing_snapshot_sha256,node_id,attempt,max_cost_usd_micros,
                    consent_contract_version,lane_ownership_id,project_lane_sha256,
                    expected_last_event_seq,expected_last_event_sha256,claim_event_sha256,
                    claim_blob,claim_bytes,released_at_ms
             FROM group_agent_graph_node_dispatch_claims WHERE graph_run_id=?1",
            [graph_run_id],
            claim_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn lane_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawLane>, HubStoreError> {
    lane_query(connection, "graph_run_id", graph_run_id)
}

pub(super) fn lane_by_project(
    connection: &Connection,
    project_lane_sha256: &[u8; 32],
) -> Result<Option<RawLane>, HubStoreError> {
    connection
        .query_row(
            "SELECT lane_ownership_id,lane_version,project_lane_sha256,graph_run_id,
                    node_id,attempt,dispatch_id,claim_event_sha256,lane_blob,lane_bytes,
                    claimed_at_ms
             FROM group_agent_project_lane_ownerships WHERE project_lane_sha256=?1",
            [project_lane_sha256.as_slice()],
            lane_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn artifact_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawArtifact>, HubStoreError> {
    connection
        .query_row(
            "SELECT id,graph_run_id,dispatch_id,artifact_version,artifact_kind,node_id,
                    attempt,claim_event_sha256,lane_ownership_id,provider_polling_began,
                    terminal_observed,true_eof_observed,retry_authorized,artifact_blob,
                    artifact_blob_bytes,artifact_bytes,artifact_sha256,created_at_ms
             FROM group_agent_graph_node_terminal_artifacts WHERE graph_run_id=?1",
            [graph_run_id],
            artifact_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn receipt_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Option<RawReceipt>, HubStoreError> {
    connection
        .query_row(
            "SELECT id,graph_run_id,dispatch_id,artifact_id,receipt_version,graph_status,
                    claim_event_sha256,lane_ownership_id,artifact_sha256,retry_authorized,
                    lane_release_authorized,receipt_blob,receipt_bytes,receipt_sha256,
                    terminal_at_ms
             FROM group_agent_graph_node_terminal_receipts WHERE graph_run_id=?1",
            [graph_run_id],
            receipt_row,
        )
        .optional()
        .map_err(read_error)
}

fn lane_query(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawLane>, HubStoreError> {
    let predicate = match column {
        "graph_run_id" => "graph_run_id=?1",
        _ => return Err(super::codec::corrupt("unsupported active-lane lookup")),
    };
    let sql = format!(
        "SELECT lane_ownership_id,lane_version,project_lane_sha256,graph_run_id,
                node_id,attempt,dispatch_id,claim_event_sha256,lane_blob,lane_bytes,
                claimed_at_ms
         FROM group_agent_project_lane_ownerships WHERE {predicate}"
    );
    connection
        .query_row(&sql, [value], lane_row)
        .optional()
        .map_err(read_error)
}

fn claim_row(row: &Row<'_>) -> rusqlite::Result<RawClaim> {
    Ok(RawClaim {
        dispatch_id: row.get(0)?,
        version: row.get(1)?,
        graph_run_id: row.get(2)?,
        authorization_id: row.get(3)?,
        authorization_sha256: row.get(4)?,
        dispatch_request_id: row.get(5)?,
        dispatch_request_sha256: row.get(6)?,
        logical_request_sha256: row.get(7)?,
        request_body_sha256: row.get(8)?,
        request_body_bytes: row.get(9)?,
        pricing_snapshot_sha256: row.get(10)?,
        node_id: row.get(11)?,
        attempt: row.get(12)?,
        max_cost_usd_micros: row.get(13)?,
        consent_contract_version: row.get(14)?,
        lane_ownership_id: row.get(15)?,
        project_lane_sha256: row.get(16)?,
        expected_last_event_seq: row.get(17)?,
        expected_last_event_sha256: row.get(18)?,
        claim_event_sha256: row.get(19)?,
        claim_blob: row.get(20)?,
        claim_bytes: row.get(21)?,
        released_at_ms: row.get(22)?,
    })
}

fn lane_row(row: &Row<'_>) -> rusqlite::Result<RawLane> {
    Ok(RawLane {
        lane_ownership_id: row.get(0)?,
        version: row.get(1)?,
        project_lane_sha256: row.get(2)?,
        graph_run_id: row.get(3)?,
        node_id: row.get(4)?,
        attempt: row.get(5)?,
        dispatch_id: row.get(6)?,
        claim_event_sha256: row.get(7)?,
        lane_blob: row.get(8)?,
        lane_bytes: row.get(9)?,
        claimed_at_ms: row.get(10)?,
    })
}

fn artifact_row(row: &Row<'_>) -> rusqlite::Result<RawArtifact> {
    Ok(RawArtifact {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        dispatch_id: row.get(2)?,
        version: row.get(3)?,
        kind: row.get(4)?,
        node_id: row.get(5)?,
        attempt: row.get(6)?,
        claim_event_sha256: row.get(7)?,
        lane_ownership_id: row.get(8)?,
        provider_polling_began: row.get(9)?,
        terminal_observed: row.get(10)?,
        true_eof_observed: row.get(11)?,
        retry_authorized: row.get(12)?,
        artifact_blob: row.get(13)?,
        artifact_blob_bytes: row.get(14)?,
        artifact_bytes: row.get(15)?,
        artifact_sha256: row.get(16)?,
        created_at_ms: row.get(17)?,
    })
}

fn receipt_row(row: &Row<'_>) -> rusqlite::Result<RawReceipt> {
    Ok(RawReceipt {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        dispatch_id: row.get(2)?,
        artifact_id: row.get(3)?,
        version: row.get(4)?,
        graph_status: row.get(5)?,
        claim_event_sha256: row.get(6)?,
        lane_ownership_id: row.get(7)?,
        artifact_sha256: row.get(8)?,
        retry_authorized: row.get(9)?,
        lane_release_authorized: row.get(10)?,
        receipt_blob: row.get(11)?,
        receipt_bytes: row.get(12)?,
        receipt_sha256: row.get(13)?,
        terminal_at_ms: row.get(14)?,
    })
}
