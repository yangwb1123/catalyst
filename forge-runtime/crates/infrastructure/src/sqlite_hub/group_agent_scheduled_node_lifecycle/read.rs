use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior};

use crate::runtime_domain::{
    GroupAgentGraphRunInspection, GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchReleaseControl, GroupAgentScheduledNodeLifecycleInspection,
    GroupAgentScheduledNodeLifecycleStatus, HubEntity, HubStoreError,
};

use super::super::{
    group_agent_graph_run, group_agent_scheduled_node_provider_request, read_error,
};

#[path = "validate.rs"]
mod validate;
use validate::validate_raw;

pub(super) const TABLE: &str = "group_agent_graph_scheduled_node_dispatch_lifecycles";

const COLUMNS: &str = "id,graph_run_id,provider_request_id,authorization_id,authorization_sha256,\
 provider_request_sha256,request_body_blob,request_body_bytes,project_lane_sha256,node_id,attempt,\
 claim_json,claim_json_bytes,active_lane_json,active_lane_json_bytes,release_control_json,\
 release_control_json_bytes,authorization_json,authorization_json_bytes,pricing_json,\
 pricing_json_bytes,claim_event_json,claim_event_json_bytes,status,lane_active,artifact_json,\
 artifact_json_bytes,terminal_control_json,terminal_control_json_bytes,terminal_receipt_json,\
 terminal_receipt_json_bytes,created_at_ms,terminalized_at_ms";

pub(super) struct RawLifecycle {
    pub id: String,
    pub graph_run_id: String,
    pub provider_request_id: String,
    pub authorization_id: String,
    pub authorization_sha256: Vec<u8>,
    pub provider_request_sha256: Vec<u8>,
    pub request_body_blob: Vec<u8>,
    pub request_body_bytes: i64,
    pub project_lane_sha256: Vec<u8>,
    pub node_id: String,
    pub attempt: i64,
    pub claim_json: Vec<u8>,
    pub claim_json_bytes: i64,
    pub active_lane_json: Vec<u8>,
    pub active_lane_json_bytes: i64,
    pub release_control_json: Vec<u8>,
    pub release_control_json_bytes: i64,
    pub authorization_json: Vec<u8>,
    pub authorization_json_bytes: i64,
    pub pricing_json: Vec<u8>,
    pub pricing_json_bytes: i64,
    pub claim_event_json: Vec<u8>,
    pub claim_event_json_bytes: i64,
    pub status: String,
    pub lane_active: i64,
    pub artifact_json: Option<Vec<u8>>,
    pub artifact_json_bytes: Option<i64>,
    pub terminal_control_json: Option<Vec<u8>>,
    pub terminal_control_json_bytes: Option<i64>,
    pub terminal_receipt_json: Option<Vec<u8>>,
    pub terminal_receipt_json_bytes: Option<i64>,
    pub created_at_ms: i64,
    pub terminalized_at_ms: Option<i64>,
}

pub(super) fn inspect(
    connection: &mut Connection,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let raw = find_by_provider_request(&transaction, provider_request_id)?.ok_or_else(|| {
        HubStoreError::NotFound {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            id: provider_request_id.into(),
        }
    })?;
    let inspection = reconstruct(&transaction, &raw)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(super) fn inspect_in_snapshot(
    connection: &Connection,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
    let raw = find_by_provider_request(connection, provider_request_id)?.ok_or_else(|| {
        HubStoreError::NotFound {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            id: provider_request_id.into(),
        }
    })?;
    reconstruct(connection, &raw)
}

pub(super) fn find_by_provider_request(
    connection: &Connection,
    provider_request_id: &str,
) -> Result<Option<RawLifecycle>, HubStoreError> {
    connection
        .query_row(
            &format!("SELECT {COLUMNS} FROM {TABLE} WHERE provider_request_id=?1"),
            [provider_request_id],
            row,
        )
        .optional()
        .map_err(read_error)
}

pub(in crate::sqlite_hub) fn has_graph_run_child(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<bool, HubStoreError> {
    connection
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {TABLE} WHERE graph_run_id=?1)"),
            [graph_run_id],
            |value| value.get(0),
        )
        .map_err(read_error)
}

pub(in crate::sqlite_hub) fn validate_graph_run_binding(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
) -> Result<(), HubStoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |value| value.get(0))
        .map_err(read_error)?;
    if version < 16 {
        return Ok(());
    }
    // v22 (wave-parallel): a Graph Run carries one dispatch lifecycle per
    // node; every row must still bind to the run.
    let all = find_all_by_run(connection, &run.run.graph_run_id)?;
    for raw in &all {
        if raw.graph_run_id != run.run.graph_run_id || raw.attempt != 1 || raw.lane_active < 0 {
            return Err(corrupt("scheduled lifecycle Graph Run binding is invalid"));
        }
        validate_raw(raw)?;
    }
    Ok(())
}

fn find_all_by_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<Vec<RawLifecycle>, HubStoreError> {
    let mut statement = connection
        .prepare(&format!(
            "SELECT {COLUMNS} FROM {TABLE} WHERE graph_run_id=?1 ORDER BY created_at_ms DESC,id DESC"
        ))
        .map_err(read_error)?;
    statement
        .query_map([graph_run_id], row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

pub(super) fn reconstruct(
    connection: &Connection,
    raw: &RawLifecycle,
) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
    validate_raw(raw)?;
    let graph_run =
        group_agent_graph_run::read::inspect_in_snapshot(connection, &raw.graph_run_id)?;
    let provider_request = group_agent_scheduled_node_provider_request::read::inspect_in_snapshot(
        connection,
        &raw.provider_request_id,
    )?;
    let parts = decode_parts(raw)?;
    let active_lane = (raw.lane_active == 1).then_some(parts.active_lane_value);
    let inspection = GroupAgentScheduledNodeLifecycleInspection {
        v: crate::runtime_domain::GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        graph_run,
        release_control: parts.release_control,
        authorization: parts.authorization,
        pricing: parts.pricing,
        provider_request: provider_request.record,
        provider_request_body: provider_request.provider_request_body,
        claim: parts.claim,
        claim_json: parts.claim_json,
        active_lane,
        active_lane_json: (raw.lane_active == 1).then_some(parts.active_lane_json),
        artifact: parts.artifact,
        artifact_json: parts.artifact_json,
        terminal_control: parts.terminal_control,
        terminal_control_json: parts.terminal_control_json,
        terminal_receipt: parts.terminal_receipt,
        terminal_receipt_json: parts.terminal_receipt_json,
        status: parts.status,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.message))?;
    Ok(inspection)
}

/// Decodes every persisted JSON projection, converts raw byte columns to
/// UTF-8, and validates the lane/claim agreement for the active lane.
fn decode_parts(raw: &RawLifecycle) -> Result<DecodedParts, HubStoreError> {
    let release_control =
        decode_json::<GroupAgentScheduledNodeDispatchReleaseControl>(&raw.release_control_json)?;
    let authorization =
        decode_json::<GroupAgentScheduledNodeDispatchAuthorization>(&raw.authorization_json)?;
    let pricing = decode_json::<GroupAgentNodePricingSnapshot>(&raw.pricing_json)?;
    let claim = decode_claim(&raw.claim_json)?;
    let active_lane_value = decode_json::<crate::runtime_domain::GroupAgentScheduledNodeActiveLane>(
        &raw.active_lane_json,
    )?;
    active_lane_value
        .validate_against_claim(&claim)
        .map_err(|error| corrupt(&error.message))?;
    let artifact = decode_optional(raw.artifact_json.as_ref())?;
    let terminal_control = decode_optional(raw.terminal_control_json.as_ref())?;
    let terminal_receipt = decode_optional(raw.terminal_receipt_json.as_ref())?;
    Ok(DecodedParts {
        release_control,
        authorization,
        pricing,
        claim,
        claim_json: utf8(&raw.claim_json, "claim")?,
        active_lane_value,
        active_lane_json: utf8(&raw.active_lane_json, "active lane")?,
        artifact,
        artifact_json: raw
            .artifact_json
            .as_deref()
            .map(|bytes| utf8(bytes, "artifact"))
            .transpose()?,
        terminal_control,
        terminal_control_json: raw
            .terminal_control_json
            .as_deref()
            .map(|bytes| utf8(bytes, "terminal control"))
            .transpose()?,
        terminal_receipt,
        terminal_receipt_json: raw
            .terminal_receipt_json
            .as_deref()
            .map(|bytes| utf8(bytes, "terminal receipt"))
            .transpose()?,
        status: parse_status(&raw.status)?,
    })
}

fn utf8(bytes: &[u8], label: &str) -> Result<String, HubStoreError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| corrupt(&format!("{label} is not UTF-8")))
}

fn decode_optional<T>(bytes: Option<&Vec<u8>>) -> Result<Option<T>, HubStoreError>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    bytes.map(|bytes| decode_json(bytes)).transpose()
}

struct DecodedParts {
    release_control: GroupAgentScheduledNodeDispatchReleaseControl,
    authorization: GroupAgentScheduledNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
    claim: GroupAgentScheduledNodeDispatchClaim,
    claim_json: String,
    active_lane_value: crate::runtime_domain::GroupAgentScheduledNodeActiveLane,
    active_lane_json: String,
    artifact: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalArtifact>,
    artifact_json: Option<String>,
    terminal_control: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalControl>,
    terminal_control_json: Option<String>,
    terminal_receipt: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalReceipt>,
    terminal_receipt_json: Option<String>,
    status: crate::runtime_domain::GroupAgentScheduledNodeLifecycleStatus,
}

fn decode_claim(bytes: &[u8]) -> Result<GroupAgentScheduledNodeDispatchClaim, HubStoreError> {
    decode_json(bytes)
}

fn decode_json<T>(bytes: &[u8]) -> Result<T, HubStoreError>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let value = serde_json::from_slice(bytes)
        .map_err(|_| corrupt("scheduled lifecycle JSON is invalid"))?;
    let canonical = serde_json::to_vec(&value)
        .map_err(|_| corrupt("scheduled lifecycle JSON cannot be encoded"))?;
    (canonical == bytes)
        .then_some(value)
        .ok_or_else(|| corrupt("scheduled lifecycle JSON is not canonical"))
}

fn parse_status(value: &str) -> Result<GroupAgentScheduledNodeLifecycleStatus, HubStoreError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| corrupt("scheduled lifecycle status is invalid"))
}

fn row(row: &Row<'_>) -> rusqlite::Result<RawLifecycle> {
    Ok(RawLifecycle {
        id: row.get(0)?,
        graph_run_id: row.get(1)?,
        provider_request_id: row.get(2)?,
        authorization_id: row.get(3)?,
        authorization_sha256: row.get(4)?,
        provider_request_sha256: row.get(5)?,
        request_body_blob: row.get(6)?,
        request_body_bytes: row.get(7)?,
        project_lane_sha256: row.get(8)?,
        node_id: row.get(9)?,
        attempt: row.get(10)?,
        claim_json: row.get(11)?,
        claim_json_bytes: row.get(12)?,
        active_lane_json: row.get(13)?,
        active_lane_json_bytes: row.get(14)?,
        release_control_json: row.get(15)?,
        release_control_json_bytes: row.get(16)?,
        authorization_json: row.get(17)?,
        authorization_json_bytes: row.get(18)?,
        pricing_json: row.get(19)?,
        pricing_json_bytes: row.get(20)?,
        claim_event_json: row.get(21)?,
        claim_event_json_bytes: row.get(22)?,
        status: row.get(23)?,
        lane_active: row.get(24)?,
        artifact_json: row.get(25)?,
        artifact_json_bytes: row.get(26)?,
        terminal_control_json: row.get(27)?,
        terminal_control_json_bytes: row.get(28)?,
        terminal_receipt_json: row.get(29)?,
        terminal_receipt_json_bytes: row.get(30)?,
        created_at_ms: row.get(31)?,
        terminalized_at_ms: row.get(32)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
