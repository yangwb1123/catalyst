use crate::runtime_domain::{
    GroupAgentScheduledNodeDispatchClaim, GroupAgentScheduledNodeDispatchClaimEvent,
    GroupAgentScheduledNodeLifecycleStatus, HubStoreError,
    group_agent_node_provider_request_sha256,
};

use crate::sqlite_hub::group_run_codec;

use super::{RawLifecycle, corrupt, decode_claim, decode_json, parse_status};

pub(super) fn validate_raw(raw: &RawLifecycle) -> Result<(), HubStoreError> {
    let claim = decode_claim(&raw.claim_json)?;
    let claim_event =
        decode_json::<GroupAgentScheduledNodeDispatchClaimEvent>(&raw.claim_event_json)?;
    claim.validate().map_err(|error| corrupt(&error.message))?;
    claim_event
        .validate()
        .map_err(|error| corrupt(&error.message))?;
    validate_raw_bindings(raw, &claim, &claim_event)?;
    validate_raw_evidence_shape(raw, &claim)?;
    validate_raw_timing(raw, &claim)
}

/// Row identity, byte-metadata, and lane-flag checks against the decoded
/// claim and claim event.
fn validate_raw_bindings(
    raw: &RawLifecycle,
    claim: &GroupAgentScheduledNodeDispatchClaim,
    claim_event: &GroupAgentScheduledNodeDispatchClaimEvent,
) -> Result<(), HubStoreError> {
    let (auth_digest, provider_digest, lane_digest) = decode_row_digests(raw)?;
    validate_raw_identity(
        raw,
        claim,
        claim_event,
        &auth_digest,
        &provider_digest,
        &lane_digest,
    )?;
    validate_raw_byte_metadata(raw)
}

/// Decodes the three stored digest columns into canonical lowercase hex.
fn decode_row_digests(raw: &RawLifecycle) -> Result<(String, String, String), HubStoreError> {
    let auth_digest = group_run_codec::encode_hex_digest(
        raw.authorization_sha256
            .as_slice()
            .try_into()
            .map_err(|_| corrupt("scheduled authorization digest is invalid"))?,
    );
    let provider_digest = group_run_codec::encode_hex_digest(
        raw.provider_request_sha256
            .as_slice()
            .try_into()
            .map_err(|_| corrupt("scheduled provider digest is invalid"))?,
    );
    let lane_digest = group_run_codec::encode_hex_digest(
        raw.project_lane_sha256
            .as_slice()
            .try_into()
            .map_err(|_| corrupt("scheduled lane digest is invalid"))?,
    );
    Ok((auth_digest, provider_digest, lane_digest))
}

/// Row identity checks: every row field must equal the decoded claim and
/// claim event, including digests, byte counts, and time ordering.
fn validate_raw_identity(
    raw: &RawLifecycle,
    claim: &GroupAgentScheduledNodeDispatchClaim,
    claim_event: &GroupAgentScheduledNodeDispatchClaimEvent,
    auth_digest: &str,
    provider_digest: &str,
    lane_digest: &str,
) -> Result<(), HubStoreError> {
    let released_at_ms = i64::try_from(claim.released_at_ms)
        .map_err(|_| corrupt("scheduled lifecycle claim time is too large"))?;
    if raw.id != claim.dispatch_id
        || raw.graph_run_id != claim.graph_run_id
        || raw.provider_request_id != claim.provider_request_id
        || raw.authorization_id != claim.authorization_id
        || auth_digest != claim.authorization_sha256
        || provider_digest != claim.provider_request_sha256
        || lane_digest != claim.project_lane_sha256
        || raw.node_id != claim.node_id
        || raw.attempt != i64::from(claim.attempt)
        || raw.request_body_bytes != i64::try_from(claim.request_body_bytes).unwrap_or(-1)
        || raw.request_body_bytes != i64::try_from(raw.request_body_blob.len()).unwrap_or(-1)
        || group_agent_node_provider_request_sha256(&raw.request_body_blob)
            != claim.request_body_sha256
        || claim_event.graph_run_id != claim.graph_run_id
        || claim_event.provider_request_id != claim.provider_request_id
        || claim_event.dispatch_id != claim.dispatch_id
        || claim_event.authorization_id != claim.authorization_id
        || claim_event.authorization_sha256 != claim.authorization_sha256
        || claim_event.provider_request_sha256 != claim.provider_request_sha256
        || claim_event.project_lane_sha256 != claim.project_lane_sha256
        || claim_event.node_id != claim.node_id
        || claim_event.attempt != claim.attempt
        || claim_event.expected_last_event_seq != claim.expected_last_event_seq
        || claim_event.expected_last_event_sha256 != claim.expected_last_event_sha256
        || claim_event.lane_ownership_id != claim.lane_ownership_id
        || claim_event.released_at_ms != claim.released_at_ms
        || claim_event.event_sha256 != claim.claim_event_sha256
        || raw.created_at_ms != released_at_ms
    {
        return Err(corrupt("scheduled lifecycle row identity disagrees"));
    }
    validate_raw_byte_metadata(raw)
}

/// Byte-metadata consistency: every persisted JSON length column must match
/// the actual stored bytes, and the lane flag must be a valid boolean.
fn validate_raw_byte_metadata(raw: &RawLifecycle) -> Result<(), HubStoreError> {
    if raw.claim_json_bytes != i64::try_from(raw.claim_json.len()).unwrap_or(-1)
        || raw.active_lane_json_bytes != i64::try_from(raw.active_lane_json.len()).unwrap_or(-1)
        || raw.release_control_json_bytes
            != i64::try_from(raw.release_control_json.len()).unwrap_or(-1)
        || raw.authorization_json_bytes != i64::try_from(raw.authorization_json.len()).unwrap_or(-1)
        || raw.pricing_json_bytes != i64::try_from(raw.pricing_json.len()).unwrap_or(-1)
        || raw.claim_event_json_bytes != i64::try_from(raw.claim_event_json.len()).unwrap_or(-1)
    {
        return Err(corrupt("scheduled lifecycle byte metadata disagrees"));
    }
    for (bytes, length) in [
        (&raw.artifact_json, &raw.artifact_json_bytes),
        (&raw.terminal_control_json, &raw.terminal_control_json_bytes),
        (&raw.terminal_receipt_json, &raw.terminal_receipt_json_bytes),
    ] {
        if bytes
            .as_ref()
            .map(Vec::len)
            .map(i64::try_from)
            .transpose()
            .unwrap_or(Some(-1))
            != *length
        {
            return Err(corrupt(
                "scheduled lifecycle nullable byte metadata disagrees",
            ));
        }
    }
    if raw.lane_active != 0 && raw.lane_active != 1 {
        return Err(corrupt("scheduled lifecycle lane flag is invalid"));
    }
    Ok(())
}

/// Status/evidence shape: each lifecycle status demands exactly the expected
/// presence or absence of the lane and terminal evidence columns.
fn validate_raw_evidence_shape(
    raw: &RawLifecycle,
    _claim: &GroupAgentScheduledNodeDispatchClaim,
) -> Result<(), HubStoreError> {
    let status = parse_status(&raw.status)?;
    let evidence_shape = match status {
        GroupAgentScheduledNodeLifecycleStatus::Claimed => {
            raw.lane_active == 1
                && raw.artifact_json.is_none()
                && raw.terminal_control_json.is_none()
                && raw.terminal_receipt_json.is_none()
                && raw.terminalized_at_ms.is_none()
                && raw.adjudicated_at_ms.is_none()
        }
        GroupAgentScheduledNodeLifecycleStatus::Terminalized => {
            raw.lane_active == 0
                && raw.artifact_json.is_some()
                && raw.terminal_control_json.is_some()
                && raw.terminal_receipt_json.is_some()
                && raw.terminalized_at_ms.is_some()
                && raw.adjudicated_at_ms.is_none()
        }
        GroupAgentScheduledNodeLifecycleStatus::Quarantined => {
            raw.lane_active == 0
                && raw.artifact_json.is_some()
                && raw.terminal_control_json.is_none()
                && raw.terminal_receipt_json.is_none()
                && raw.terminalized_at_ms.is_some()
                && raw.adjudicated_at_ms.is_none()
        }
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated => {
            raw.lane_active == 0
                && raw.artifact_json.is_none()
                && raw.terminal_control_json.is_none()
                && raw.terminal_receipt_json.is_none()
                && raw.terminalized_at_ms.is_none()
                && raw.adjudicated_at_ms.is_some()
        }
    };
    if !evidence_shape {
        return Err(corrupt(
            "scheduled lifecycle evidence/status shape disagrees",
        ));
    }
    Ok(())
}

/// Persisted lifecycle times must bind exactly to their authenticated source
/// time or follow the claim release, depending on the status transition.
fn validate_raw_timing(
    raw: &RawLifecycle,
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> Result<(), HubStoreError> {
    let released_at_ms = i64::try_from(claim.released_at_ms)
        .map_err(|_| corrupt("scheduled lifecycle claim time is too large"))?;
    if raw
        .adjudicated_at_ms
        .is_some_and(|time| time < released_at_ms)
    {
        return Err(corrupt(
            "scheduled lifecycle adjudication time predates claim",
        ));
    }
    let Some(terminalized_at_ms) = raw.terminalized_at_ms else {
        return Ok(());
    };
    let artifact_json = raw
        .artifact_json
        .as_ref()
        .ok_or_else(|| corrupt("scheduled lifecycle terminal time has no artifact"))?;
    let artifact = decode_json::<crate::runtime_domain::GroupAgentScheduledNodeTerminalArtifact>(
        artifact_json,
    )?;
    let artifact_created_at_ms = i64::try_from(artifact.created_at_ms)
        .map_err(|_| corrupt("scheduled lifecycle artifact time is too large"))?;
    if terminalized_at_ms != artifact_created_at_ms || terminalized_at_ms < released_at_ms {
        return Err(corrupt(
            "scheduled lifecycle terminal time disagrees with artifact",
        ));
    }
    Ok(())
}
