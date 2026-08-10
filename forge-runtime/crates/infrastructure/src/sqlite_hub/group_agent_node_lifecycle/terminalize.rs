use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GroupAgentGraphRunStatus,
    GroupAgentNodeLifecycleInspection, GroupAgentNodeTerminalArtifactKind, HubEntity,
    HubStoreError, TerminalizeGroupAgentNodeDispatch, TerminalizeGroupAgentNodeDispatchResult,
};

use super::{
    super::{read_error, write_error},
    codec::{candidate_digest, conflict, corrupt, to_i64},
    read, rows, source,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TerminalWriteStage {
    BeforeArtifact,
    ArtifactInserted,
    ReceiptInserted,
    EventInserted,
    RunTransitioned,
    LaneDeleted,
}

pub(super) fn terminalize(
    connection: &mut Connection,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<TerminalizeGroupAgentNodeDispatchResult, HubStoreError> {
    terminalize_with_write_fault(connection, request, |_| Ok(()))
}

pub(super) fn terminalize_with_write_fault<F>(
    connection: &mut Connection,
    request: &TerminalizeGroupAgentNodeDispatch,
    write_fault: F,
) -> Result<TerminalizeGroupAgentNodeDispatchResult, HubStoreError>
where
    F: FnMut(TerminalWriteStage) -> Result<(), HubStoreError>,
{
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = terminalize_locked(&transaction, request, write_fault)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn terminalize_locked<F>(
    transaction: &Transaction<'_>,
    request: &TerminalizeGroupAgentNodeDispatch,
    mut write_fault: F,
) -> Result<TerminalizeGroupAgentNodeDispatchResult, HubStoreError>
where
    F: FnMut(TerminalWriteStage) -> Result<(), HubStoreError>,
{
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    reject_terminal_replay(transaction, &request.control.graph_run.graph_run_id)?;
    let inspection =
        read::inspect_in_snapshot(transaction, &request.control.graph_run.graph_run_id)?;
    ensure_claim_source(&inspection, request)?;
    let loaded = source::load(
        transaction,
        &request.control.graph_run.graph_run_id,
        &request.control.dispatch_request.dispatch_request_id,
    )?;
    source::validate_terminal_source(&loaded, &request.control)?;
    write_fault(TerminalWriteStage::BeforeArtifact)?;
    insert_artifact(transaction, request)?;
    write_fault(TerminalWriteStage::ArtifactInserted)?;
    insert_receipt(transaction, request)?;
    write_fault(TerminalWriteStage::ReceiptInserted)?;
    insert_event(transaction, request)?;
    write_fault(TerminalWriteStage::EventInserted)?;
    transition_run(transaction, request)?;
    write_fault(TerminalWriteStage::RunTransitioned)?;
    release_lane(transaction, request)?;
    write_fault(TerminalWriteStage::LaneDeleted)?;
    let persisted =
        read::inspect_in_snapshot(transaction, &request.control.graph_run.graph_run_id)?;
    ensure_persisted(&persisted, request)?;
    Ok(TerminalizeGroupAgentNodeDispatchResult {
        v: request.v,
        inspection: persisted,
    })
}

fn reject_terminal_replay(
    transaction: &Transaction<'_>,
    graph_run_id: &str,
) -> Result<(), HubStoreError> {
    let artifact = rows::artifact_by_run(transaction, graph_run_id)?;
    let receipt = rows::receipt_by_run(transaction, graph_run_id)?;
    if artifact.is_none() && receipt.is_none() {
        return Ok(());
    }
    let _ = read::inspect_in_snapshot(transaction, graph_run_id)?;
    Err(conflict("Node dispatch lifecycle is already terminal"))
}

fn ensure_claim_source(
    inspection: &GroupAgentNodeLifecycleInspection,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let exact = inspection.claim == request.control.claim
        && inspection.active_lane.as_ref() == Some(&request.control.active_lane)
        && inspection.artifact.is_none()
        && inspection.terminal_receipt.is_none()
        && inspection.graph_run.run == request.control.graph_run
        && inspection.graph_run.plan == request.control.plan
        && inspection.graph_run.events == request.control.journal_events;
    exact
        .then_some(())
        .ok_or_else(|| conflict("terminal control does not match the active durable claim"))
}

fn insert_artifact(
    transaction: &Transaction<'_>,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let artifact = &request.control.artifact;
    let claim_head = candidate_digest(&artifact.claim_event_sha256, "artifact claim event")?;
    let digest = candidate_digest(&artifact.artifact_sha256, "terminal artifact")?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_node_terminal_artifacts(
               id,graph_run_id,dispatch_id,artifact_version,artifact_kind,node_id,attempt,
               claim_event_sha256,lane_ownership_id,provider_polling_began,terminal_observed,
               true_eof_observed,retry_authorized,artifact_blob,artifact_bytes,
               artifact_blob_bytes,artifact_sha256,created_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                artifact.artifact_id,
                artifact.graph_run_id,
                artifact.dispatch_id,
                i64::from(artifact.v),
                artifact_kind(artifact.artifact_kind),
                artifact.node_id,
                i64::from(artifact.attempt),
                claim_head.as_slice(),
                artifact.lane_ownership_id,
                i64::from(artifact.provider_poll_started),
                i64::from(artifact.terminal_seen),
                i64::from(artifact.stream_eof_seen),
                i64::from(artifact.retry_authorized),
                request.artifact_json.as_bytes(),
                to_i64(artifact.artifact_bytes, "artifact payload byte count")?,
                to_i64(request.artifact_json.len(), "artifact blob byte count")?,
                digest.as_slice(),
                to_i64(artifact.created_at_ms, "artifact creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    Ok(())
}

fn insert_receipt(
    transaction: &Transaction<'_>,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let receipt = &request.receipt;
    let claim_head = candidate_digest(&receipt.expected_last_event_sha256, "receipt claim event")?;
    let artifact = candidate_digest(&receipt.artifact_sha256, "receipt artifact")?;
    let digest = candidate_digest(&receipt.receipt_sha256, "terminal receipt")?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_node_terminal_receipts(
               id,graph_run_id,dispatch_id,artifact_id,receipt_version,graph_status,
               claim_event_sha256,lane_ownership_id,artifact_sha256,retry_authorized,
               lane_release_authorized,receipt_blob,receipt_bytes,receipt_sha256,terminal_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                receipt.receipt_id,
                receipt.graph_run_id,
                receipt.dispatch_id,
                receipt.artifact_id,
                i64::from(receipt.v),
                read::graph_status(receipt.graph_status),
                claim_head.as_slice(),
                receipt.lane_ownership_id,
                artifact.as_slice(),
                i64::from(receipt.retry_authorized),
                i64::from(receipt.lane_release_authorized),
                request.receipt_json.as_bytes(),
                to_i64(request.receipt_json.len(), "receipt byte count")?,
                digest.as_slice(),
                to_i64(request.terminalized_at_ms, "terminal receipt time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let digest = request
        .event
        .expected_sha256()
        .map_err(|error| conflict(&error.to_string()))?;
    let digest = candidate_digest(&digest, "terminal event")?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,event_sha256,created_at_ms
             ) VALUES(?1,5,?2,'node_lifecycle_terminalized',?3,?4,?5,?6)",
            params![
                request.control.graph_run.graph_run_id,
                i64::from(request.event.v),
                request.event_json.as_bytes(),
                to_i64(request.event_json.len(), "terminal event byte count")?,
                digest.as_slice(),
                to_i64(request.terminalized_at_ms, "terminal event time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    Ok(())
}

fn transition_run(
    transaction: &Transaction<'_>,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let receipt = &request.receipt;
    let head = candidate_digest(&receipt.expected_last_event_sha256, "claim event")?;
    let changed = transaction
        .execute(
            "UPDATE group_agent_graph_runs
             SET run_version=5,status=?1,last_event_seq=5,journal_bytes=journal_bytes+?2
             WHERE id=?3 AND run_version=4 AND status='dispatch_unknown'
               AND execution_contract_present=1 AND dispatch_request_present=1
               AND dispatch_authority_released=1 AND last_event_seq=?4
               AND EXISTS(SELECT 1 FROM group_agent_graph_run_events
                 WHERE graph_run_id=?3 AND seq=?4 AND event_sha256=?5)",
            params![
                terminal_status(receipt.graph_status)?,
                to_i64(request.event_json.len(), "terminal journal byte count")?,
                receipt.graph_run_id,
                to_i64(receipt.expected_last_event_seq, "expected event sequence")?,
                head.as_slice(),
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(conflict(
            "Graph Run cursor, claim head, or terminal state changed",
        ))
    }
}

fn release_lane(
    transaction: &Transaction<'_>,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let control = &request.control;
    let project = candidate_digest(&control.active_lane.project_lane_sha256, "project lane")?;
    let claim_head = candidate_digest(&control.claim.claim_event_sha256, "claim event")?;
    let changed = transaction
        .execute(
            "DELETE FROM group_agent_project_lane_ownerships
             WHERE lane_ownership_id=?1 AND graph_run_id=?2 AND dispatch_id=?3
               AND project_lane_sha256=?4 AND claim_event_sha256=?5",
            params![
                control.active_lane.lane_ownership_id,
                control.active_lane.graph_run_id,
                control.active_lane.dispatch_id,
                project.as_slice(),
                claim_head.as_slice(),
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(conflict("exact active Project lane ownership is missing"))
    }
}

fn ensure_persisted(
    inspection: &GroupAgentNodeLifecycleInspection,
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    // Localized run-level post-state assertions (design §7.6): the CAS result
    // must be terminal at v5/seq-5 with the exact journal delta and event
    // count. These double the transitive readback chain (`reconstruct`:
    // `load_events` re-summation, `valid_terminal_record_state`, `validate_state_shape`,
    // `inspection.validate()` exact status/v/seq/event binding) so the CAS
    // post-state is self-evident to a reader of this file alone.
    let expected_journal = request
        .control
        .graph_run
        .journal_bytes
        .checked_add(request.event_json.len())
        .ok_or_else(|| corrupt("terminal journal byte count overflows"))?;
    let exact = inspection.active_lane.is_none()
        && inspection.artifact.as_ref() == Some(&request.control.artifact)
        && inspection.artifact_json.as_deref() == Some(request.artifact_json.as_str())
        && inspection.terminal_receipt.as_ref() == Some(&request.receipt)
        && inspection.terminal_receipt_json.as_deref() == Some(request.receipt_json.as_str())
        && inspection.graph_run.run.status == request.receipt.graph_status
        && inspection.graph_run.run.v == GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION
        && inspection.graph_run.run.last_event_seq == 5
        && inspection.graph_run.run.journal_bytes == expected_journal
        && inspection.graph_run.events.len() == 5
        && inspection.graph_run.events.get(4) == Some(&request.event)
        && inspection.graph_run.event_jsons.get(4) == Some(&request.event_json);
    exact
        .then_some(())
        .ok_or_else(|| corrupt("persisted terminal lifecycle disagrees with committed input"))
}

fn artifact_kind(kind: GroupAgentNodeTerminalArtifactKind) -> &'static str {
    match kind {
        GroupAgentNodeTerminalArtifactKind::Result => "result",
        GroupAgentNodeTerminalArtifactKind::Uncertainty => "uncertainty",
    }
}

fn terminal_status(status: GroupAgentGraphRunStatus) -> Result<&'static str, HubStoreError> {
    match status {
        GroupAgentGraphRunStatus::Completed => Ok("completed"),
        GroupAgentGraphRunStatus::Failed => Ok("failed"),
        GroupAgentGraphRunStatus::FailedUncertain => Ok("failed_uncertain"),
        _ => Err(conflict("Core receipt selected a nonterminal Graph status")),
    }
}
