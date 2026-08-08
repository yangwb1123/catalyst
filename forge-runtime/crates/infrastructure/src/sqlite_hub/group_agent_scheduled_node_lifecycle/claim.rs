use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    ClaimGroupAgentScheduledNodeDispatch, ClaimGroupAgentScheduledNodeDispatchResult,
    GroupAgentScheduledNodeDispatchAuthority, HubEntity, HubStoreError,
};

use super::{
    super::{
        group_agent_graph_run, group_agent_scheduled_node_provider_request, group_run_codec,
        read_error, write_error,
    },
    read::{self, TABLE},
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

///  rejects an idempotent re-claim whose input
/// disagrees with the stored lifecycle (mirrors the prepare/admit replay
/// equality contract).
fn ensure_replay_equality(
    inspection: &crate::runtime_domain::GroupAgentScheduledNodeLifecycleInspection,
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<(), HubStoreError> {
    let exact = inspection.claim.dispatch_id == request.claim.dispatch_id
        && inspection.claim.authorization_id == request.claim.authorization_id
        && inspection.provider_request.provider_request_id
            == request.provider_request.provider_request_id
        && inspection.provider_request_body == request.provider_request_body;
    exact.then_some(()).ok_or_else(|| {
        conflict("idempotency key was reused with different scheduled dispatch claim input")
    })
}

pub(super) fn claim(
    connection: &mut Connection,
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<ClaimGroupAgentScheduledNodeDispatchResult, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    if let Some(raw) =
        read::find_by_provider_request(&transaction, &request.provider_request.provider_request_id)?
    {
        let inspection = read::reconstruct(&transaction, &raw)?;
        // Stage-03 Finding 4: replay-equality — a conflicting re-claim with
        // the same provider request but different input must not silently
        // return AlreadyClaimed.
        ensure_replay_equality(&inspection, request)?;
        return Ok(ClaimGroupAgentScheduledNodeDispatchResult::AlreadyClaimed { inspection });
    }
    request
        .validate()
        .map_err(|error| conflict(&error.message))?;
    let graph_run = group_agent_graph_run::read::inspect_pristine_in_snapshot(
        &transaction,
        &request.claim.graph_run_id,
    )?;
    validate_pristine_run(&graph_run, request)?;
    let provider_request = group_agent_scheduled_node_provider_request::read::inspect_in_snapshot(
        &transaction,
        &request.provider_request.provider_request_id,
    )?;
    if provider_request.record != request.provider_request
        || provider_request.provider_request_body != request.provider_request_body
    {
        return Err(corrupt("scheduled claim provider request source disagrees"));
    }
    reject_owned_lane(&transaction, request)?;
    insert(&transaction, request)?;
    let inspection =
        read::inspect_in_snapshot(&transaction, &request.provider_request.provider_request_id)?;
    let authority = GroupAgentScheduledNodeDispatchAuthority::new(
        &inspection.provider_request,
        inspection.claim.clone(),
        inspection.provider_request_body.clone(),
    )
    .map_err(|error| corrupt(&error.message))?;
    transaction.commit().map_err(read_error)?;
    Ok(ClaimGroupAgentScheduledNodeDispatchResult::Claimed { authority })
}

fn validate_pristine_run(
    run: &group_agent_graph_run::read::PristineRunSnapshot,
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<(), HubStoreError> {
    let expected = &request.release_control.graph_run;
    let valid = run.run.v == 1
        && run.run.graph_run_id == request.claim.graph_run_id
        && run.run == *expected
        && run.run.last_event_seq == 1
        && !run.run.dispatch_authority_released
        && run.event_count == 1;
    valid
        .then_some(())
        .ok_or_else(|| conflict("scheduled claim requires exact pristine v1/seq-1 Graph Run"))
}

fn reject_owned_lane(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<(), HubStoreError> {
    let lane = group_run_codec::decode_hex_digest(&request.claim.project_lane_sha256)
        .ok_or_else(|| conflict("scheduled Project lane digest is invalid"))?;
    let owned: Option<String> = transaction
        .query_row(
            &format!(
                "SELECT graph_run_id FROM {TABLE} WHERE project_lane_sha256=?1 AND lane_active=1"
            ),
            [lane.as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_error)?;
    if owned.is_some() {
        return Err(conflict(
            "Project lane is already owned by a scheduled dispatch",
        ));
    }
    Ok(())
}

const INSERT_SQL: &str = "INSERT INTO {TABLE} (id,graph_run_id,provider_request_id,authorization_id,\
                 authorization_sha256,provider_request_sha256,request_body_blob,request_body_bytes,\
                 project_lane_sha256,node_id,attempt,claim_json,claim_json_bytes,active_lane_json,\
                 active_lane_json_bytes,release_control_json,release_control_json_bytes,\
                 authorization_json,authorization_json_bytes,pricing_json,pricing_json_bytes,\
                 claim_event_json,claim_event_json_bytes,status,lane_active,created_at_ms)\
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,\
                 ?19,?20,?21,?22,?23,'claimed',1,?24)";

#[allow(clippy::too_many_lines)]
fn insert(
    transaction: &Transaction<'_>,
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<(), HubStoreError> {
    let claim = &request.claim;
    let auth_digest = digest(&claim.authorization_sha256)?;
    let provider_digest = digest(&claim.provider_request_sha256)?;
    let lane_digest = digest(&claim.project_lane_sha256)?;
    transaction
        .execute(
            INSERT_SQL,
            params![
                claim.dispatch_id,
                claim.graph_run_id,
                claim.provider_request_id,
                claim.authorization_id,
                auth_digest,
                provider_digest,
                &request.provider_request_body,
                sized(
                    "scheduled request body",
                    request.provider_request_body.len()
                )?,
                lane_digest,
                claim.node_id,
                i64::from(claim.attempt),
                request.claim_json.as_bytes(),
                sized("scheduled claim", request.claim_json.len())?,
                request.active_lane_json.as_bytes(),
                sized("scheduled lane", request.active_lane_json.len())?,
                request.release_control_json.as_bytes(),
                sized(
                    "scheduled release control",
                    request.release_control_json.len()
                )?,
                request.authorization_json.as_bytes(),
                sized("scheduled authorization", request.authorization_json.len())?,
                request.pricing_json.as_bytes(),
                sized("scheduled pricing", request.pricing_json.len())?,
                request.claim_event_json.as_bytes(),
                sized("scheduled claim event", request.claim_event_json.len())?,
                i64::try_from(claim.released_at_ms)
                    .map_err(|_| conflict("scheduled claim time is too large"))?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentScheduledNodeLifecycle, error))?;
    Ok(())
}

/// Converts a persisted JSON byte count to an i64, failing closed when the
/// value cannot be represented as a `SQLite` integer.
fn sized(label: &str, len: usize) -> Result<i64, HubStoreError> {
    i64::try_from(len).map_err(|_| conflict(&format!("{label} is too large")))
}

fn digest(value: &str) -> Result<Vec<u8>, HubStoreError> {
    group_run_codec::decode_hex_digest(value)
        .map(|digest| digest.to_vec())
        .ok_or_else(|| corrupt("scheduled lifecycle digest is invalid"))
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
