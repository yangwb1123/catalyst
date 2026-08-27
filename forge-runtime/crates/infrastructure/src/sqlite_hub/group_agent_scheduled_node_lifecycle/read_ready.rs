use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::{
    GroupAgentGraphRunInspection, GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeDispatchReleaseControl,
    GroupAgentScheduledReadyNodeLifecycleInspection, HubEntity, HubStoreError,
};

use super::{
    super::{group_agent_graph_run, group_agent_scheduled_node_provider_request, read_error},
    read::{
        RawLifecycle, StoredLifecycleKind, corrupt, decode_claim, decode_json, decode_optional,
        find_by_provider_request, optional_nonnegative_millis, parse_status, require_kind, utf8,
        validate_raw_for_family,
    },
};

pub(super) fn inspect(
    connection: &mut Connection,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, provider_request_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(super) fn inspect_in_snapshot(
    connection: &Connection,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, HubStoreError> {
    let raw = find_by_provider_request(connection, provider_request_id)?.ok_or_else(|| {
        HubStoreError::NotFound {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            id: provider_request_id.into(),
        }
    })?;
    reconstruct(connection, &raw)
}

pub(super) fn reconstruct(
    connection: &Connection,
    raw: &RawLifecycle,
) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, HubStoreError> {
    validate_raw_for_family(raw)?;
    require_kind(raw, StoredLifecycleKind::Ready)?;
    let graph_run =
        group_agent_graph_run::read::inspect_in_snapshot(connection, &raw.graph_run_id)?;
    let provider_request = group_agent_scheduled_node_provider_request::read::inspect_in_snapshot(
        connection,
        &raw.provider_request_id,
    )?;
    reconstruct_with_sources(raw, &graph_run, &provider_request)
}

pub(super) fn reconstruct_with_sources(
    raw: &RawLifecycle,
    graph_run: &GroupAgentGraphRunInspection,
    provider_request: &GroupAgentScheduledNodeProviderRequestInspection,
) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, HubStoreError> {
    validate_raw_for_family(raw)?;
    require_kind(raw, StoredLifecycleKind::Ready)?;
    if raw.graph_run_id != graph_run.run.graph_run_id
        || raw.provider_request_id != provider_request.record.provider_request_id
    {
        return Err(corrupt(
            "scheduled ready-node lifecycle source binding disagrees",
        ));
    }
    let parts = decode_parts(raw)?;
    let active = raw.lane_active == 1;
    let inspection = GroupAgentScheduledReadyNodeLifecycleInspection {
        v: crate::runtime_domain::GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
        graph_run: graph_run.clone(),
        release_control: parts.release_control,
        authorization: parts.authorization,
        pricing: parts.pricing,
        provider_request: provider_request.record.clone(),
        provider_request_body: provider_request.provider_request_body.clone(),
        claim: parts.claim,
        claim_json: parts.claim_json,
        active_lane: active.then_some(parts.active_lane),
        active_lane_json: active.then_some(parts.active_lane_json),
        artifact: parts.artifact,
        artifact_json: parts.artifact_json,
        terminal_control: parts.terminal_control,
        terminal_control_json: parts.terminal_control_json,
        terminal_receipt: parts.terminal_receipt,
        terminal_receipt_json: parts.terminal_receipt_json,
        status: parts.status,
        adjudicated_at_ms: optional_nonnegative_millis(
            raw.adjudicated_at_ms,
            "scheduled ready-node lifecycle adjudication time",
        )?,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.message))?;
    Ok(inspection)
}

fn decode_parts(raw: &RawLifecycle) -> Result<DecodedReadyParts, HubStoreError> {
    let release_control = decode_json::<GroupAgentScheduledReadyNodeDispatchReleaseControl>(
        &raw.release_control_json,
    )?;
    let authorization =
        decode_json::<GroupAgentScheduledReadyNodeDispatchAuthorization>(&raw.authorization_json)?;
    let pricing = decode_json::<GroupAgentNodePricingSnapshot>(&raw.pricing_json)?;
    let claim = decode_claim(&raw.claim_json)?;
    let active_lane: crate::runtime_domain::GroupAgentScheduledNodeActiveLane =
        decode_json(&raw.active_lane_json)?;
    active_lane
        .validate_against_claim(&claim)
        .map_err(|error| corrupt(&error.message))?;
    Ok(DecodedReadyParts {
        release_control,
        authorization,
        pricing,
        claim,
        claim_json: utf8(&raw.claim_json, "claim")?,
        active_lane,
        active_lane_json: utf8(&raw.active_lane_json, "active lane")?,
        artifact: decode_optional(raw.artifact_json.as_ref())?,
        artifact_json: optional_utf8(raw.artifact_json.as_deref(), "artifact")?,
        terminal_control: decode_optional(raw.terminal_control_json.as_ref())?,
        terminal_control_json: optional_utf8(
            raw.terminal_control_json.as_deref(),
            "terminal control",
        )?,
        terminal_receipt: decode_optional(raw.terminal_receipt_json.as_ref())?,
        terminal_receipt_json: optional_utf8(
            raw.terminal_receipt_json.as_deref(),
            "terminal receipt",
        )?,
        status: parse_status(&raw.status)?,
    })
}

fn optional_utf8(bytes: Option<&[u8]>, label: &str) -> Result<Option<String>, HubStoreError> {
    bytes.map(|value| utf8(value, label)).transpose()
}

struct DecodedReadyParts {
    release_control: GroupAgentScheduledReadyNodeDispatchReleaseControl,
    authorization: GroupAgentScheduledReadyNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
    claim: crate::runtime_domain::GroupAgentScheduledNodeDispatchClaim,
    claim_json: String,
    active_lane: crate::runtime_domain::GroupAgentScheduledNodeActiveLane,
    active_lane_json: String,
    artifact: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalArtifact>,
    artifact_json: Option<String>,
    terminal_control: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalControl>,
    terminal_control_json: Option<String>,
    terminal_receipt: Option<crate::runtime_domain::GroupAgentScheduledNodeTerminalReceipt>,
    terminal_receipt_json: Option<String>,
    status: crate::runtime_domain::GroupAgentScheduledNodeLifecycleStatus,
}
