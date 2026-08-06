use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior, params};

use crate::runtime_domain::{
    AdjudicateGroupAgentScheduledNodeDispatch, GroupAgentScheduledNodeLifecycleInspection,
    GroupAgentScheduledNodeLifecycleStatus, HubEntity, HubStoreError,
};

use super::{
    super::{read_error, write_error},
    read::{self, TABLE},
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn adjudicate(
    connection: &mut Connection,
    request: &AdjudicateGroupAgentScheduledNodeDispatch,
) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let raw = read::find_by_provider_request(&transaction, &request.provider_request_id)?
        .ok_or_else(|| HubStoreError::NotFound {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            id: request.provider_request_id.clone(),
        })?;
    let existing = read::reconstruct(&transaction, &raw)?;
    // 只有硬崩溃的 claim(无任何 terminal evidence、lane 仍 active)可被裁决。
    if existing.status != GroupAgentScheduledNodeLifecycleStatus::Claimed
        || existing.active_lane.is_none()
        || existing.artifact.is_some()
        || existing.terminal_control.is_some()
        || existing.terminal_receipt.is_some()
    {
        return Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            message: "scheduled dispatch is not a hard-crashed claim; adjudication refused".into(),
        });
    }
    let adjudicated_at_ms = i64::try_from(request.adjudicated_at_ms)
        .map_err(|_| conflict("scheduled adjudication time is too large"))?;
    if adjudicated_at_ms < raw.created_at_ms {
        return Err(conflict("scheduled adjudication predates the claim"));
    }
    transaction
        .execute(
            &format!(
                "UPDATE {TABLE} SET status='adjudicated',lane_active=0,adjudicated_at_ms=?1 \\
                 WHERE provider_request_id=?2"
            ),
            params![adjudicated_at_ms, request.provider_request_id],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentScheduledNodeLifecycle, error))?;
    let inspection = read::inspect_in_snapshot(&transaction, &request.provider_request_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentScheduledNodeLifecycle,
        message: message.into(),
    }
}
