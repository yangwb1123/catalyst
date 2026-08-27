use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    ClaimGroupAgentNodeDispatch, ClaimGroupAgentNodeDispatchResult,
    GroupAgentNodeDispatchAuthority, GroupAgentNodeLifecycleInspection, HubEntity, HubStoreError,
};

use super::{
    super::{read_error, write_error},
    codec::{candidate_digest, conflict, corrupt, to_i64},
    read, rows, source,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClaimWriteStage {
    BeforeClaim,
    ClaimInserted,
    LaneInserted,
    EventInserted,
    RunTransitioned,
}

pub(super) fn claim(
    connection: &mut Connection,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<ClaimGroupAgentNodeDispatchResult, HubStoreError> {
    claim_with_write_fault(connection, request, |_| Ok(()))
}

pub(super) fn claim_with_write_fault<F>(
    connection: &mut Connection,
    request: &ClaimGroupAgentNodeDispatch,
    write_fault: F,
) -> Result<ClaimGroupAgentNodeDispatchResult, HubStoreError>
where
    F: FnMut(ClaimWriteStage) -> Result<(), HubStoreError>,
{
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = claim_locked(&transaction, request, write_fault)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn claim_locked<F>(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentNodeDispatch,
    mut write_fault: F,
) -> Result<ClaimGroupAgentNodeDispatchResult, HubStoreError>
where
    F: FnMut(ClaimWriteStage) -> Result<(), HubStoreError>,
{
    if rows::claim_by_run(transaction, &request.claim.graph_run_id)?.is_some() {
        let inspection = read::inspect_in_snapshot(transaction, &request.claim.graph_run_id)?;
        return Ok(ClaimGroupAgentNodeDispatchResult::AlreadyClaimed { inspection });
    }
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let loaded = source::load(
        transaction,
        &request.claim.graph_run_id,
        &request.claim.dispatch_request_id,
    )?;
    source::validate_release_source(&loaded, &request.release_control)?;
    reject_owned_lane(transaction, request)?;
    write_fault(ClaimWriteStage::BeforeClaim)?;
    insert_claim(transaction, request)?;
    write_fault(ClaimWriteStage::ClaimInserted)?;
    insert_lane(transaction, request)?;
    write_fault(ClaimWriteStage::LaneInserted)?;
    insert_event(transaction, request)?;
    write_fault(ClaimWriteStage::EventInserted)?;
    transition_run(transaction, request)?;
    write_fault(ClaimWriteStage::RunTransitioned)?;
    let inspection = read::inspect_in_snapshot(transaction, &request.claim.graph_run_id)?;
    ensure_persisted(&inspection, request)?;
    let claim_event = inspection
        .graph_run
        .events
        .get(3)
        .ok_or_else(|| corrupt("persisted dispatch claim has no seq-4 event"))?;
    let authority = GroupAgentNodeDispatchAuthority::new(
        &loaded.dispatch.record,
        inspection.claim.clone(),
        claim_event,
        loaded.dispatch.provider_request_body,
    )
    .map_err(|error| corrupt(&error.to_string()))?;
    Ok(ClaimGroupAgentNodeDispatchResult::Claimed { authority })
}

fn reject_owned_lane(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let lane = candidate_digest(&request.claim.project_lane_sha256, "project lane")?;
    if let Some(stored) = rows::lane_by_project(transaction, &lane)? {
        read::inspect_in_snapshot(transaction, &stored.graph_run_id)?;
        return Err(conflict(
            "Project lane is already owned by another durable dispatch claim",
        ));
    }
    let scheduled_owned: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1
               FROM group_agent_graph_scheduled_node_dispatch_lifecycles
               WHERE project_lane_sha256=?1 AND lane_active=1
             )",
            [lane.as_slice()],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    if scheduled_owned {
        return Err(conflict(
            "Project lane is already owned by another durable dispatch claim",
        ));
    }
    Ok(())
}

fn insert_claim(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let claim = &request.claim;
    let digests = ClaimDigests::new(request)?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_node_dispatch_claims(
               dispatch_id,claim_version,graph_run_id,authorization_id,authorization_sha256,
               dispatch_request_id,dispatch_request_sha256,logical_request_sha256,
               request_body_sha256,request_body_bytes,pricing_snapshot_sha256,node_id,attempt,
               max_cost_usd_micros,consent_contract_version,lane_ownership_id,
               project_lane_sha256,expected_last_event_seq,expected_last_event_sha256,
               claim_event_sha256,claim_blob,claim_bytes,released_at_ms
             ) VALUES(
               ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,
               ?18,?19,?20,?21,?22,?23
             )",
            params![
                claim.dispatch_id,
                i64::from(claim.v),
                claim.graph_run_id,
                claim.authorization_id,
                digests.authorization.as_slice(),
                claim.dispatch_request_id,
                digests.dispatch_request.as_slice(),
                digests.logical_request.as_slice(),
                digests.request_body.as_slice(),
                to_i64(claim.request_body_bytes, "request body byte count")?,
                digests.pricing.as_slice(),
                claim.node_id,
                i64::from(claim.attempt),
                to_i64(claim.max_cost_usd_micros, "maximum cost")?,
                i64::from(claim.consent_contract_version),
                claim.lane_ownership_id,
                digests.project_lane.as_slice(),
                to_i64(claim.expected_last_event_seq, "expected event sequence")?,
                digests.expected_head.as_slice(),
                digests.claim_event.as_slice(),
                request.claim_json.as_bytes(),
                to_i64(request.claim_json.len(), "claim byte count")?,
                to_i64(claim.released_at_ms, "claim release time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    Ok(())
}

fn insert_lane(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let lane = &request.active_lane;
    let project = candidate_digest(&lane.project_lane_sha256, "active project lane")?;
    let claim_head = candidate_digest(&lane.claim_event_sha256, "active claim event")?;
    transaction
        .execute(
            "INSERT INTO group_agent_project_lane_ownerships(
               lane_ownership_id,lane_version,project_lane_sha256,graph_run_id,node_id,
               attempt,dispatch_id,claim_event_sha256,lane_blob,lane_bytes,claimed_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                lane.lane_ownership_id,
                i64::from(lane.v),
                project.as_slice(),
                lane.graph_run_id,
                lane.node_id,
                i64::from(lane.attempt),
                lane.dispatch_id,
                claim_head.as_slice(),
                request.active_lane_json.as_bytes(),
                to_i64(request.active_lane_json.len(), "active lane byte count")?,
                to_i64(lane.claimed_at_ms, "active lane claim time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let digest = candidate_digest(&request.claim.claim_event_sha256, "claim event")?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,event_sha256,created_at_ms
             ) VALUES(?1,4,?2,'node_dispatch_released',?3,?4,?5,?6)",
            params![
                request.claim.graph_run_id,
                i64::from(request.event.v),
                request.event_json.as_bytes(),
                to_i64(request.event_json.len(), "claim event byte count")?,
                digest.as_slice(),
                to_i64(request.claim.released_at_ms, "claim event time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    Ok(())
}

fn transition_run(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let claim = &request.claim;
    let head = candidate_digest(&claim.expected_last_event_sha256, "expected event")?;
    let changed = transaction
        .execute(
            "UPDATE group_agent_graph_runs
             SET run_version=4,status='dispatch_unknown',dispatch_authority_released=1,
                 last_event_seq=4,journal_bytes=journal_bytes+?1
             WHERE id=?2 AND run_version=3 AND status='awaiting_dispatch_authorization'
               AND execution_contract_present=1 AND dispatch_request_present=1
               AND dispatch_authority_released=0 AND last_event_seq=?3
               AND EXISTS(SELECT 1 FROM group_agent_graph_run_events
                 WHERE graph_run_id=?2 AND seq=?3 AND event_sha256=?4)",
            params![
                to_i64(request.event_json.len(), "claim journal byte count")?,
                claim.graph_run_id,
                to_i64(claim.expected_last_event_seq, "expected event sequence")?,
                head.as_slice(),
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeLifecycle, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(conflict(
            "Graph Run cursor, journal head, or dispatch claim state changed",
        ))
    }
}

fn ensure_persisted(
    inspection: &GroupAgentNodeLifecycleInspection,
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<(), HubStoreError> {
    let exact = inspection.claim == request.claim
        && inspection.claim_json == request.claim_json
        && inspection.active_lane.as_ref() == Some(&request.active_lane)
        && inspection.active_lane_json.as_deref() == Some(request.active_lane_json.as_str())
        && inspection.graph_run.events.get(3) == Some(&request.event)
        && inspection.graph_run.event_jsons.get(3) == Some(&request.event_json);
    exact
        .then_some(())
        .ok_or_else(|| corrupt("persisted dispatch claim disagrees with committed input"))
}

struct ClaimDigests {
    authorization: [u8; 32],
    dispatch_request: [u8; 32],
    logical_request: [u8; 32],
    request_body: [u8; 32],
    pricing: [u8; 32],
    project_lane: [u8; 32],
    expected_head: [u8; 32],
    claim_event: [u8; 32],
}

impl ClaimDigests {
    fn new(request: &ClaimGroupAgentNodeDispatch) -> Result<Self, HubStoreError> {
        let claim = &request.claim;
        Ok(Self {
            authorization: candidate_digest(&claim.authorization_sha256, "authorization")?,
            dispatch_request: candidate_digest(&claim.dispatch_request_sha256, "dispatch request")?,
            logical_request: candidate_digest(&claim.logical_request_sha256, "logical request")?,
            request_body: candidate_digest(&claim.request_body_sha256, "request body")?,
            pricing: candidate_digest(&claim.pricing_snapshot_sha256, "pricing snapshot")?,
            project_lane: candidate_digest(&claim.project_lane_sha256, "project lane")?,
            expected_head: candidate_digest(&claim.expected_last_event_sha256, "expected event")?,
            claim_event: candidate_digest(&claim.claim_event_sha256, "claim event")?,
        })
    }
}
