use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior, params};

use crate::runtime_domain::{
    GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledNodeTerminalArtifact, HubEntity,
    HubStoreError, TerminalizeGroupAgentScheduledNodeDispatch,
    TerminalizeGroupAgentScheduledNodeDispatchResult,
};

use super::{
    super::{read_error, write_error},
    read::{self, TABLE},
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn terminalize(
    connection: &mut Connection,
    request: &TerminalizeGroupAgentScheduledNodeDispatch,
) -> Result<TerminalizeGroupAgentScheduledNodeDispatchResult, HubStoreError> {
    validate_request(request)?;
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let provider_request_id = provider_request_id(request)?;
    let raw =
        read::find_by_provider_request(&transaction, &provider_request_id)?.ok_or_else(|| {
            HubStoreError::NotFound {
                entity: HubEntity::GroupAgentScheduledNodeLifecycle,
                id: provider_request_id.clone(),
            }
        })?;
    let existing = read::reconstruct(&transaction, &raw)?;
    if !matches!(
        existing.status,
        GroupAgentScheduledNodeLifecycleStatus::Claimed
    ) {
        transaction.commit().map_err(read_error)?;
        return Ok(TerminalizeGroupAgentScheduledNodeDispatchResult {
            v: request.v,
            inspection: existing,
        });
    }
    let artifact = decode_artifact(&request.artifact_json)?;
    verify_artifact_binding(&artifact, &existing)?;
    apply_terminal_update(
        &transaction,
        request,
        &artifact,
        &existing.provider_request.provider_request_id,
    )?;
    let inspection =
        read::inspect_in_snapshot(&transaction, &existing.provider_request.provider_request_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(TerminalizeGroupAgentScheduledNodeDispatchResult {
        v: request.v,
        inspection,
    })
}

/// The terminal artifact must bind every claim identity it carries to the
/// durable claim before any lane release is applied.
fn verify_artifact_binding(
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
    existing: &crate::runtime_domain::GroupAgentScheduledNodeLifecycleInspection,
) -> Result<(), HubStoreError> {
    if artifact.provider_request_id != existing.provider_request.provider_request_id
        || artifact.dispatch_id != existing.claim.dispatch_id
        || artifact.authorization_sha256 != existing.claim.authorization_sha256
        || artifact.provider_request_sha256 != existing.claim.provider_request_sha256
        || artifact.project_lane_sha256 != existing.claim.project_lane_sha256
        || artifact.claim_event_sha256 != existing.claim.claim_event_sha256
    {
        return Err(corrupt(
            "scheduled terminal artifact does not bind to claim",
        ));
    }
    Ok(())
}

/// Applies the terminal (or quarantine) UPDATE: releases the lane, stores the
/// artifact and optional Core receipt evidence, and records the terminal time.
fn apply_terminal_update(
    transaction: &rusqlite::Transaction<'_>,
    request: &TerminalizeGroupAgentScheduledNodeDispatch,
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
    provider_request_id: &str,
) -> Result<(), HubStoreError> {
    let (status, control_json, receipt_json) = terminal_evidence(request, artifact)?;
    let artifact_json = request.artifact_json.as_bytes();
    let control_json_bytes = control_json.map(<[u8]>::to_vec);
    let receipt_json_bytes = receipt_json.map(<[u8]>::to_vec);
    transaction
        .execute(
            &format!(
                "UPDATE {TABLE} SET status=?1,lane_active=0,artifact_json=?2,artifact_json_bytes=?3,\
                 terminal_control_json=?4,terminal_control_json_bytes=?5,terminal_receipt_json=?6,\
                 terminal_receipt_json_bytes=?7,terminalized_at_ms=?8 WHERE provider_request_id=?9"
            ),
            params![
                status,
                artifact_json,
                i64::try_from(artifact_json.len()).map_err(|_| corrupt("scheduled artifact is too large"))?,
                control_json_bytes.as_deref(),
                control_json_bytes
                    .as_ref()
                    .map(|bytes| i64::try_from(bytes.len()).unwrap_or(-1)),
                receipt_json_bytes.as_deref(),
                receipt_json_bytes
                    .as_ref()
                    .map(|bytes| i64::try_from(bytes.len()).unwrap_or(-1)),
                i64::try_from(request.terminalized_at_ms).map_err(|_| corrupt("scheduled terminal time is too large"))?,
                provider_request_id,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentScheduledNodeLifecycle, error))?;
    Ok(())
}

/// Selects the status and optional evidence pair for a terminal or quarantine
/// update, rejecting a partial Core receipt on quarantine.
type TerminalEvidence<'a> = (&'static str, Option<&'a [u8]>, Option<&'a [u8]>);

fn terminal_evidence<'a>(
    request: &'a TerminalizeGroupAgentScheduledNodeDispatch,
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
) -> Result<TerminalEvidence<'a>, HubStoreError> {
    match (&request.control, &request.receipt) {
        (Some(control), Some(receipt)) => {
            if control.artifact != *artifact {
                return Err(corrupt("scheduled terminal control artifact disagrees"));
            }
            receipt
                .validate_against_control(control)
                .map_err(|error| corrupt(&error.message))?;
            Ok((
                "terminalized",
                Some(
                    request
                        .control_json
                        .as_ref()
                        .ok_or_else(|| corrupt("missing terminal control JSON"))?
                        .as_bytes(),
                ),
                Some(
                    request
                        .receipt_json
                        .as_ref()
                        .ok_or_else(|| corrupt("missing terminal receipt JSON"))?
                        .as_bytes(),
                ),
            ))
        }
        (None, None) => Ok(("quarantined", None, None)),
        _ => Err(corrupt(
            "scheduled quarantine cannot carry a partial Core receipt",
        )),
    }
}

fn validate_request(
    request: &TerminalizeGroupAgentScheduledNodeDispatch,
) -> Result<(), HubStoreError> {
    let artifact = decode_artifact(&request.artifact_json)?;
    match (&request.control, &request.receipt) {
        (Some(control), Some(receipt)) => {
            control
                .validate()
                .map_err(|error| corrupt(&error.message))?;
            receipt
                .validate()
                .map_err(|error| corrupt(&error.message))?;
            receipt
                .validate_against_control(control)
                .map_err(|error| corrupt(&error.message))?;
            let control_json = control
                .canonical_json()
                .map_err(|error| corrupt(&error.message))?;
            let receipt_json = receipt
                .canonical_json()
                .map_err(|error| corrupt(&error.message))?;
            if request.control_json.as_deref() != Some(control_json.as_str())
                || request.receipt_json.as_deref() != Some(receipt_json.as_str())
                || control.artifact != artifact
            {
                return Err(corrupt("scheduled terminal JSON is not canonical"));
            }
        }
        (None, None) => {
            if request.control_json.is_some() || request.receipt_json.is_some() {
                return Err(corrupt("scheduled quarantine has unexpected Core JSON"));
            }
        }
        _ => return Err(corrupt("scheduled terminal request is incomplete")),
    }
    artifact
        .validate()
        .map_err(|error| corrupt(&error.message))?;
    Ok(())
}

fn provider_request_id(
    request: &TerminalizeGroupAgentScheduledNodeDispatch,
) -> Result<String, HubStoreError> {
    let artifact = decode_artifact(&request.artifact_json)?;
    Ok(artifact.provider_request_id)
}

fn decode_artifact(json: &str) -> Result<GroupAgentScheduledNodeTerminalArtifact, HubStoreError> {
    let artifact: GroupAgentScheduledNodeTerminalArtifact = serde_json::from_str(json)
        .map_err(|_| corrupt("scheduled terminal artifact is invalid"))?;
    let canonical = artifact
        .canonical_json()
        .map_err(|error| corrupt(&error.message))?;
    (canonical == json)
        .then_some(artifact)
        .ok_or_else(|| corrupt("scheduled terminal artifact is not canonical"))
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
