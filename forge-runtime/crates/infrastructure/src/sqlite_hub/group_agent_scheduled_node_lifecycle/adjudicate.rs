use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    AdjudicateGroupAgentScheduledNodeDispatch, GroupAgentScheduledNodeAnyLifecycleInspection,
    GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStatus, HubEntity,
    HubStoreError,
};

use super::{
    super::{read_error, write_error},
    read::{self, StoredLifecycleKind, TABLE},
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(test)]
#[path = "adjudicate_tests.rs"]
mod tests;

pub(super) fn adjudicate_legacy(
    connection: &mut Connection,
    request: &AdjudicateGroupAgentScheduledNodeDispatch,
) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
    match adjudicate(connection, request, Some(StoredLifecycleKind::Legacy))? {
        GroupAgentScheduledNodeAnyLifecycleInspection::Legacy(value) => Ok(*value),
        GroupAgentScheduledNodeAnyLifecycleInspection::Ready(_) => Err(conflict(
            "legacy adjudication cannot target a ready-node lifecycle",
        )),
    }
}

pub(super) fn adjudicate_any(
    connection: &mut Connection,
    request: &AdjudicateGroupAgentScheduledNodeDispatch,
) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError> {
    adjudicate(connection, request, None)
}

fn adjudicate(
    connection: &mut Connection,
    request: &AdjudicateGroupAgentScheduledNodeDispatch,
    expected_kind: Option<StoredLifecycleKind>,
) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError> {
    validate_request(request)?;
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let raw = required_raw(&transaction, &request.provider_request_id)?;
    require_family(&raw, expected_kind)?;
    let existing = read::inspect_any_in_snapshot(&transaction, &request.provider_request_id)?;
    validate_claimed_owner(&existing, request)?;
    let adjudicated_at_ms = validate_time(raw.created_at_ms, request.adjudicated_at_ms)?;
    update_exact_claim(
        &transaction,
        request,
        &raw.active_lane_json,
        adjudicated_at_ms,
    )?;
    let inspection = read::inspect_any_in_snapshot(&transaction, &request.provider_request_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

fn required_raw(
    connection: &Connection,
    provider_request_id: &str,
) -> Result<read::RawLifecycle, HubStoreError> {
    read::find_by_provider_request(connection, provider_request_id)?.ok_or_else(|| {
        HubStoreError::NotFound {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            id: provider_request_id.into(),
        }
    })
}

fn require_family(
    raw: &read::RawLifecycle,
    expected_kind: Option<StoredLifecycleKind>,
) -> Result<(), HubStoreError> {
    let actual = read::stored_kind(raw)?;
    if expected_kind.is_none() || expected_kind == Some(actual) {
        return Ok(());
    }
    Err(conflict(
        "scheduled adjudication lifecycle protocol family disagrees",
    ))
}

fn validate_request(
    request: &AdjudicateGroupAgentScheduledNodeDispatch,
) -> Result<(), HubStoreError> {
    request
        .validate()
        .map_err(|_| conflict("scheduled adjudication request is invalid"))
}

fn validate_claimed_owner(
    inspection: &GroupAgentScheduledNodeAnyLifecycleInspection,
    request: &AdjudicateGroupAgentScheduledNodeDispatch,
) -> Result<(), HubStoreError> {
    let (status, lane, no_terminal) = match inspection {
        GroupAgentScheduledNodeAnyLifecycleInspection::Legacy(value) => (
            value.status,
            value.active_lane.as_ref(),
            value.artifact.is_none()
                && value.terminal_control.is_none()
                && value.terminal_receipt.is_none(),
        ),
        GroupAgentScheduledNodeAnyLifecycleInspection::Ready(value) => (
            value.status,
            value.active_lane.as_ref(),
            value.artifact.is_none()
                && value.terminal_control.is_none()
                && value.terminal_receipt.is_none(),
        ),
    };
    let exact_owner =
        lane.is_some_and(|value| value.lane_ownership_id == request.expected_lane_ownership_id);
    if status == GroupAgentScheduledNodeLifecycleStatus::Claimed && no_terminal && exact_owner {
        return Ok(());
    }
    Err(conflict(
        "scheduled dispatch is not the exact owned hard-crashed claim",
    ))
}

fn validate_time(created_at_ms: i64, requested: u64) -> Result<i64, HubStoreError> {
    let adjudicated_at_ms = i64::try_from(requested)
        .map_err(|_| conflict("scheduled adjudication time is too large"))?;
    if adjudicated_at_ms < created_at_ms {
        return Err(conflict("scheduled adjudication predates the claim"));
    }
    Ok(adjudicated_at_ms)
}

fn update_exact_claim(
    transaction: &Transaction<'_>,
    request: &AdjudicateGroupAgentScheduledNodeDispatch,
    active_lane_json: &[u8],
    adjudicated_at_ms: i64,
) -> Result<(), HubStoreError> {
    let sql = format!(
        "UPDATE {TABLE} SET status='adjudicated',lane_active=0,adjudicated_at_ms=?1 \
         WHERE provider_request_id=?2 AND status='claimed' AND lane_active=1 \
         AND active_lane_json=?3 AND artifact_json IS NULL \
         AND terminal_control_json IS NULL AND terminal_receipt_json IS NULL \
         AND terminalized_at_ms IS NULL AND adjudicated_at_ms IS NULL \
         AND json_extract(CAST(active_lane_json AS TEXT),'$.lane_ownership_id')=?4"
    );
    let updated = transaction
        .execute(
            &sql,
            params![
                adjudicated_at_ms,
                request.provider_request_id,
                active_lane_json,
                request.expected_lane_ownership_id
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentScheduledNodeLifecycle, error))?;
    if updated != 1 {
        return Err(conflict("scheduled adjudication compare-and-set lost"));
    }
    Ok(())
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentScheduledNodeLifecycle,
        message: message.into(),
    }
}
