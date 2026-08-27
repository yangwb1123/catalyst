use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::{
    ClaimGroupAgentScheduledReadyNodeDispatch, ClaimGroupAgentScheduledReadyNodeDispatchResult,
    GroupAgentScheduledNodeDispatchAuthority, GroupAgentScheduledReadyNodeLifecycleInspection,
    HubEntity, HubStoreError, ScheduledReadyNodeReleaseSource,
};

use super::{
    claim::{LifecycleInsert, insert, reject_owned_lane},
    read, read_ready,
};
use crate::sqlite_hub::{read_error, scheduled_graph_progress};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn claim(
    connection: &mut Connection,
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> Result<ClaimGroupAgentScheduledReadyNodeDispatchResult, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    request
        .validate()
        .map_err(|error| conflict(&error.message))?;
    if let Some(raw) =
        read::find_by_provider_request(&transaction, &request.provider_request.provider_request_id)?
    {
        let inspection = read_ready::reconstruct(&transaction, &raw)?;
        ensure_replay_equality(&inspection, request)?;
        return Ok(ClaimGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed { inspection });
    }
    validate_current_source(&transaction, request)?;
    reject_owned_lane(&transaction, &request.claim.project_lane_sha256)?;
    insert(&transaction, &insert_view(request))?;
    let inspection = read_ready::inspect_in_snapshot(
        &transaction,
        &request.provider_request.provider_request_id,
    )?;
    let authority = GroupAgentScheduledNodeDispatchAuthority::new(
        &inspection.provider_request,
        inspection.claim.clone(),
        inspection.provider_request_body.clone(),
    )
    .map_err(|error| corrupt(&error.message))?;
    transaction.commit().map_err(read_error)?;
    Ok(ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { authority })
}

fn validate_current_source(
    connection: &Connection,
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> Result<(), HubStoreError> {
    let control = &request.release_control;
    let source = scheduled_graph_progress::ready_release::inspect_in_snapshot(
        connection,
        &control.graph_run.graph_run_id,
        &control.progress_snapshot.snapshot_sha256,
        request.authorization.execution_ordinal,
        &request.authorization.node_id,
    )?;
    source_matches_control(&source, request)
        .then_some(())
        .ok_or_else(|| conflict("scheduled ready-node source changed before claim"))
}

fn source_matches_control(
    source: &ScheduledReadyNodeReleaseSource,
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> bool {
    let control = &request.release_control;
    let selected = &source.selected_provider_request;
    control.graph_run == source.graph_run.run
        && control.journal_events == source.graph_run.events
        && control.control_snapshot.plan == source.graph_run.plan
        && control.control_snapshot.manifest == source.graph.manifest
        && control.schedule_record == source.schedule.record
        && control.schedule == source.schedule.schedule
        && control.progress_snapshot == source.progress_snapshot
        && control.scheduled_contract_record == selected.scheduled_contract.record
        && control.scheduled_contract == selected.scheduled_contract.candidate
        && control.direct_predecessor_receipts == source.direct_predecessor_receipts
        && control.predecessor_content_artifact == source.predecessor_content_artifact
        && control.provider_request == selected.record
        && control.provider_request_json.as_bytes() == selected.provider_request_body
        && request.provider_request == selected.record
        && request.provider_request_body == selected.provider_request_body
}

fn ensure_replay_equality(
    inspection: &GroupAgentScheduledReadyNodeLifecycleInspection,
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> Result<(), HubStoreError> {
    let exact = inspection.release_control == request.release_control
        && inspection.authorization == request.authorization
        && inspection.pricing == request.pricing
        && inspection.provider_request == request.provider_request
        && inspection.provider_request_body == request.provider_request_body;
    exact.then_some(()).ok_or_else(|| {
        conflict("idempotency key was reused with different scheduled ready-node claim input")
    })
}

fn insert_view(request: &ClaimGroupAgentScheduledReadyNodeDispatch) -> LifecycleInsert<'_> {
    LifecycleInsert {
        claim: &request.claim,
        provider_request_body: &request.provider_request_body,
        claim_json: &request.claim_json,
        active_lane_json: &request.active_lane_json,
        release_control_json: &request.release_control_json,
        authorization_json: &request.authorization_json,
        pricing_json: &request.pricing_json,
        claim_event_json: &request.claim_event_json,
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentScheduledNodeLifecycle,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
