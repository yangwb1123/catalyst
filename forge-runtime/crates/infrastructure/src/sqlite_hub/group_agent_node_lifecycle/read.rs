use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::{
    GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection, GroupAgentGraphRunStatus,
    GroupAgentNodeActiveLane, GroupAgentNodeDispatchClaim, GroupAgentNodeDispatchRequestInspection,
    GroupAgentNodeLifecycleInspection, GroupAgentNodeTerminalArtifact,
    GroupAgentNodeTerminalArtifactKind, GroupAgentNodeTerminalReceipt, HubStoreError,
};

use super::{
    super::{group_agent_graph_run, read_error},
    codec::{convert, corrupt, not_found, stored_digest, stored_json},
    rows::{self, RawArtifact, RawClaim, RawLane, RawReceipt},
};

pub(super) fn inspect(
    connection: &mut Connection,
    graph_run_id: &str,
) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, graph_run_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(super) fn inspect_in_snapshot(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError> {
    let graph_run = group_agent_graph_run::read::inspect_in_snapshot(connection, graph_run_id)?;
    reconstruct(connection, graph_run)?.ok_or_else(|| not_found(graph_run_id))
}

pub(in crate::sqlite_hub) fn validate_graph_run_binding(
    connection: &Connection,
    graph_run: &GroupAgentGraphRunInspection,
    dispatch: Option<&GroupAgentNodeDispatchRequestInspection>,
) -> Result<(), HubStoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(read_error)?;
    if version == 11 {
        return Ok(());
    }
    let Some(lifecycle) = reconstruct(connection, graph_run.clone())? else {
        return Ok(());
    };
    validate_claim_source(&lifecycle.claim, dispatch)?;
    validate_terminal_bindings(&lifecycle)
}

pub(super) fn reconstruct(
    connection: &Connection,
    graph_run: GroupAgentGraphRunInspection,
) -> Result<Option<GroupAgentNodeLifecycleInspection>, HubStoreError> {
    let graph_run_id = &graph_run.run.graph_run_id;
    let Some(raw_claim) = rows::claim_by_run(connection, graph_run_id)? else {
        reject_missing_claim(connection, &graph_run)?;
        return Ok(None);
    };
    let (claim, claim_json) = decode_claim(raw_claim)?;
    let active_lane = decode_lane(rows::lane_by_run(connection, graph_run_id)?)?;
    let artifact = decode_artifact(rows::artifact_by_run(connection, graph_run_id)?)?;
    let receipt = decode_receipt(rows::receipt_by_run(connection, graph_run_id)?)?;
    validate_state_shape(
        graph_run.run.status,
        active_lane.is_some(),
        artifact.is_some(),
        receipt.is_some(),
    )?;
    let inspection = GroupAgentNodeLifecycleInspection {
        v: claim.v,
        graph_run,
        claim,
        claim_json,
        active_lane: active_lane.as_ref().map(|pair| pair.0.clone()),
        active_lane_json: active_lane.map(|pair| pair.1),
        artifact: artifact.as_ref().map(|pair| pair.0.clone()),
        artifact_json: artifact.map(|pair| pair.1),
        terminal_receipt: receipt.as_ref().map(|decoded| decoded.receipt.clone()),
        terminal_receipt_json: receipt.as_ref().map(|decoded| decoded.json.clone()),
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    validate_terminal_time(&inspection, receipt.as_ref())?;
    Ok(Some(inspection))
}

fn reject_missing_claim(
    connection: &Connection,
    graph_run: &GroupAgentGraphRunInspection,
) -> Result<(), HubStoreError> {
    let graph_run_id = &graph_run.run.graph_run_id;
    let orphaned = rows::lane_by_run(connection, graph_run_id)?.is_some()
        || rows::artifact_by_run(connection, graph_run_id)?.is_some()
        || rows::receipt_by_run(connection, graph_run_id)?.is_some();
    if orphaned {
        return Err(corrupt(
            "stored Node lifecycle has child evidence without an immutable claim",
        ));
    }
    if is_lifecycle_status(graph_run.run.status) {
        return Err(corrupt(
            "stored lifecycle Graph Run has no immutable dispatch claim",
        ));
    }
    Ok(())
}

fn is_lifecycle_status(status: GroupAgentGraphRunStatus) -> bool {
    matches!(
        status,
        GroupAgentGraphRunStatus::DispatchUnknown
            | GroupAgentGraphRunStatus::Completed
            | GroupAgentGraphRunStatus::Failed
            | GroupAgentGraphRunStatus::FailedUncertain
    )
}

fn decode_claim(raw: RawClaim) -> Result<(GroupAgentNodeDispatchClaim, String), HubStoreError> {
    let claim = GroupAgentNodeDispatchClaim::decode_exact(&raw.claim_blob)
        .map_err(|error| corrupt(&error.to_string()))?;
    let exact = raw.dispatch_id == claim.dispatch_id
        && raw.version == i64::from(claim.v)
        && raw.graph_run_id == claim.graph_run_id
        && raw.authorization_id == claim.authorization_id
        && stored_digest(&raw.authorization_sha256, "authorization")? == claim.authorization_sha256
        && raw.dispatch_request_id == claim.dispatch_request_id
        && stored_digest(&raw.dispatch_request_sha256, "dispatch request")?
            == claim.dispatch_request_sha256
        && stored_digest(&raw.logical_request_sha256, "logical request")?
            == claim.logical_request_sha256
        && stored_digest(&raw.request_body_sha256, "request body")? == claim.request_body_sha256
        && usize_matches(raw.request_body_bytes, claim.request_body_bytes)
        && stored_digest(&raw.pricing_snapshot_sha256, "pricing snapshot")?
            == claim.pricing_snapshot_sha256
        && claim_identity_matches(&raw, &claim)
        && raw.claim_bytes == i64::try_from(raw.claim_blob.len()).unwrap_or(-1);
    if !exact {
        return Err(corrupt("stored immutable dispatch claim row disagrees"));
    }
    let json = stored_json(raw.claim_blob, "dispatch claim")?;
    Ok((claim, json))
}

fn claim_identity_matches(raw: &RawClaim, claim: &GroupAgentNodeDispatchClaim) -> bool {
    raw.node_id == claim.node_id
        && raw.attempt == i64::from(claim.attempt)
        && u64_matches(raw.max_cost_usd_micros, claim.max_cost_usd_micros)
        && raw.consent_contract_version == i64::from(claim.consent_contract_version)
        && raw.lane_ownership_id == claim.lane_ownership_id
        && stored_digest(&raw.project_lane_sha256, "project lane").as_deref()
            == Ok(claim.project_lane_sha256.as_str())
        && u64_matches(raw.expected_last_event_seq, claim.expected_last_event_seq)
        && stored_digest(&raw.expected_last_event_sha256, "expected event").as_deref()
            == Ok(claim.expected_last_event_sha256.as_str())
        && stored_digest(&raw.claim_event_sha256, "claim event").as_deref()
            == Ok(claim.claim_event_sha256.as_str())
        && u64_matches(raw.released_at_ms, claim.released_at_ms)
}

fn decode_lane(
    raw: Option<RawLane>,
) -> Result<Option<(GroupAgentNodeActiveLane, String)>, HubStoreError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let lane = GroupAgentNodeActiveLane::decode_exact(&raw.lane_blob)
        .map_err(|error| corrupt(&error.to_string()))?;
    let exact = raw.lane_ownership_id == lane.lane_ownership_id
        && raw.version == i64::from(lane.v)
        && stored_digest(&raw.project_lane_sha256, "active project lane")?
            == lane.project_lane_sha256
        && raw.graph_run_id == lane.graph_run_id
        && raw.node_id == lane.node_id
        && raw.attempt == i64::from(lane.attempt)
        && raw.dispatch_id == lane.dispatch_id
        && stored_digest(&raw.claim_event_sha256, "active claim event")? == lane.claim_event_sha256
        && raw.lane_bytes == i64::try_from(raw.lane_blob.len()).unwrap_or(-1)
        && u64_matches(raw.claimed_at_ms, lane.claimed_at_ms);
    if !exact {
        return Err(corrupt("stored active Project lane row disagrees"));
    }
    let json = stored_json(raw.lane_blob, "active Project lane")?;
    Ok(Some((lane, json)))
}

fn decode_artifact(
    raw: Option<RawArtifact>,
) -> Result<Option<(GroupAgentNodeTerminalArtifact, String)>, HubStoreError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let artifact = GroupAgentNodeTerminalArtifact::decode_exact(&raw.artifact_blob)
        .map_err(|error| corrupt(&error.to_string()))?;
    let exact = raw.id == artifact.artifact_id
        && raw.graph_run_id == artifact.graph_run_id
        && raw.dispatch_id == artifact.dispatch_id
        && raw.version == i64::from(artifact.v)
        && raw.kind == artifact_kind(artifact.artifact_kind)
        && raw.node_id == artifact.node_id
        && raw.attempt == i64::from(artifact.attempt)
        && stored_digest(&raw.claim_event_sha256, "artifact claim event")?
            == artifact.claim_event_sha256
        && raw.lane_ownership_id == artifact.lane_ownership_id
        && bool_matches(raw.provider_polling_began, artifact.provider_poll_started)
        && bool_matches(raw.terminal_observed, artifact.terminal_seen)
        && bool_matches(raw.true_eof_observed, artifact.stream_eof_seen)
        && bool_matches(raw.retry_authorized, artifact.retry_authorized)
        && raw.artifact_blob_bytes == i64::try_from(raw.artifact_blob.len()).unwrap_or(-1)
        && usize_matches(raw.artifact_bytes, artifact.artifact_bytes)
        && stored_digest(&raw.artifact_sha256, "terminal artifact")? == artifact.artifact_sha256
        && u64_matches(raw.created_at_ms, artifact.created_at_ms);
    if !exact {
        return Err(corrupt("stored terminal artifact row disagrees"));
    }
    let json = stored_json(raw.artifact_blob, "terminal artifact")?;
    Ok(Some((artifact, json)))
}

struct DecodedReceipt {
    receipt: GroupAgentNodeTerminalReceipt,
    json: String,
    terminal_at_ms: u64,
}

fn decode_receipt(raw: Option<RawReceipt>) -> Result<Option<DecodedReceipt>, HubStoreError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let receipt = GroupAgentNodeTerminalReceipt::decode_exact(&raw.receipt_blob)
        .map_err(|error| corrupt(&error.to_string()))?;
    let exact = raw.id == receipt.receipt_id
        && raw.graph_run_id == receipt.graph_run_id
        && raw.dispatch_id == receipt.dispatch_id
        && raw.artifact_id == receipt.artifact_id
        && raw.version == i64::from(receipt.v)
        && raw.graph_status == graph_status(receipt.graph_status)
        && stored_digest(&raw.claim_event_sha256, "receipt claim event")?
            == receipt.expected_last_event_sha256
        && raw.lane_ownership_id == receipt.lane_ownership_id
        && stored_digest(&raw.artifact_sha256, "receipt artifact")? == receipt.artifact_sha256
        && bool_matches(raw.retry_authorized, receipt.retry_authorized)
        && bool_matches(raw.lane_release_authorized, receipt.lane_release_authorized)
        && raw.receipt_bytes == i64::try_from(raw.receipt_blob.len()).unwrap_or(-1)
        && stored_digest(&raw.receipt_sha256, "terminal receipt")? == receipt.receipt_sha256;
    if !exact {
        return Err(corrupt("stored Core terminal receipt row disagrees"));
    }
    let json = stored_json(raw.receipt_blob, "Core terminal receipt")?;
    Ok(Some(DecodedReceipt {
        receipt,
        json,
        terminal_at_ms: convert(raw.terminal_at_ms, "terminal receipt time")?,
    }))
}

fn validate_claim_source(
    claim: &GroupAgentNodeDispatchClaim,
    dispatch: Option<&GroupAgentNodeDispatchRequestInspection>,
) -> Result<(), HubStoreError> {
    let dispatch =
        dispatch.ok_or_else(|| corrupt("stored dispatch claim has no durable dispatch request"))?;
    let record = &dispatch.record;
    let exact = claim.graph_run_id == record.graph_run_id
        && claim.dispatch_request_id == record.dispatch_request_id
        && claim.dispatch_request_sha256 == record.dispatch_request_sha256
        && claim.logical_request_sha256 == record.request_sha256
        && claim.request_body_sha256 == record.provider_request_sha256
        && claim.request_body_bytes == record.provider_request_bytes
        && claim.pricing_snapshot_sha256 == record.pricing_snapshot_sha256
        && claim.node_id == record.node_id
        && claim.attempt == record.attempt
        && claim.project_lane_sha256 == record.project_lane_sha256;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("stored dispatch claim disagrees with its durable request"))
}

fn validate_terminal_bindings(
    lifecycle: &GroupAgentNodeLifecycleInspection,
) -> Result<(), HubStoreError> {
    let Some(artifact) = lifecycle.artifact.as_ref() else {
        return Ok(());
    };
    let claim = &lifecycle.claim;
    let exact = artifact.graph_run_id == claim.graph_run_id
        && artifact.node_id == claim.node_id
        && artifact.attempt == claim.attempt
        && artifact.dispatch_id == claim.dispatch_id
        && artifact.claim_event_sha256 == claim.claim_event_sha256
        && artifact.authorization_sha256 == claim.authorization_sha256
        && artifact.dispatch_request_sha256 == claim.dispatch_request_sha256
        && artifact.logical_request_sha256 == claim.logical_request_sha256
        && artifact.request_body_sha256 == claim.request_body_sha256
        && artifact.pricing_snapshot_sha256 == claim.pricing_snapshot_sha256
        && artifact.lane_ownership_id == claim.lane_ownership_id
        && artifact.project_lane_sha256 == claim.project_lane_sha256;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("stored terminal artifact disagrees with its immutable claim"))?;
    validate_receipt_bindings(lifecycle, artifact)
}

fn validate_receipt_bindings(
    lifecycle: &GroupAgentNodeLifecycleInspection,
    artifact: &GroupAgentNodeTerminalArtifact,
) -> Result<(), HubStoreError> {
    let receipt = lifecycle
        .terminal_receipt
        .as_ref()
        .ok_or_else(|| corrupt("stored terminal artifact has no Core receipt"))?;
    let claim = &lifecycle.claim;
    let exact = receipt.expected_last_event_seq == 4
        && receipt.expected_last_event_sha256 == claim.claim_event_sha256
        && receipt.graph_run_id == claim.graph_run_id
        && receipt.graph_id == lifecycle.graph_run.run.graph_id
        && receipt.node_id == claim.node_id
        && receipt.attempt == claim.attempt
        && receipt.dispatch_id == claim.dispatch_id
        && receipt.lane_ownership_id == claim.lane_ownership_id
        && receipt.project_lane_sha256 == claim.project_lane_sha256
        && receipt.artifact_kind == artifact.artifact_kind
        && receipt.artifact_id == artifact.artifact_id
        && receipt.artifact_sha256 == artifact.artifact_sha256;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("stored Core receipt disagrees with its terminal claim"))
}

fn validate_terminal_time(
    lifecycle: &GroupAgentNodeLifecycleInspection,
    receipt: Option<&DecodedReceipt>,
) -> Result<(), HubStoreError> {
    let Some(receipt) = receipt else {
        return Ok(());
    };
    let Some(GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
        terminalized_at_ms, ..
    }) = lifecycle.graph_run.events.get(4).map(|event| &event.kind)
    else {
        return Err(corrupt("stored terminal receipt has no seq-5 event time"));
    };
    (receipt.terminal_at_ms == *terminalized_at_ms)
        .then_some(())
        .ok_or_else(|| corrupt("stored terminal receipt time disagrees with seq-5"))
}

fn validate_state_shape(
    status: GroupAgentGraphRunStatus,
    lane: bool,
    artifact: bool,
    receipt: bool,
) -> Result<(), HubStoreError> {
    let valid = match status {
        GroupAgentGraphRunStatus::DispatchUnknown => lane && !artifact && !receipt,
        GroupAgentGraphRunStatus::Completed
        | GroupAgentGraphRunStatus::Failed
        | GroupAgentGraphRunStatus::FailedUncertain => !lane && artifact && receipt,
        _ => false,
    };
    valid
        .then_some(())
        .ok_or_else(|| corrupt("stored Node lifecycle rows disagree with Graph Run state"))
}

fn artifact_kind(kind: GroupAgentNodeTerminalArtifactKind) -> &'static str {
    match kind {
        GroupAgentNodeTerminalArtifactKind::Result => "result",
        GroupAgentNodeTerminalArtifactKind::Uncertainty => "uncertainty",
    }
}

pub(super) fn graph_status(status: GroupAgentGraphRunStatus) -> &'static str {
    match status {
        GroupAgentGraphRunStatus::Completed => "completed",
        GroupAgentGraphRunStatus::Failed => "failed",
        GroupAgentGraphRunStatus::FailedUncertain => "failed_uncertain",
        _ => "unsupported",
    }
}

fn bool_matches(stored: i64, value: bool) -> bool {
    stored == i64::from(value)
}

fn usize_matches(stored: i64, value: usize) -> bool {
    convert::<usize>(stored, "byte count").as_ref() == Ok(&value)
}

fn u64_matches(stored: i64, value: u64) -> bool {
    convert::<u64>(stored, "unsigned value").as_ref() == Ok(&value)
}
