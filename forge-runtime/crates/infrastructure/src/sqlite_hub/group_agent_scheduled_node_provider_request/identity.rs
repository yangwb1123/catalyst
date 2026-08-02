use rusqlite::Transaction;

use crate::runtime_domain::{
    HubEntity, HubStoreError, PrepareGroupAgentScheduledNodeProviderRequest,
};

use super::{read, rows};

pub(super) fn reject_existing(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    allowed_request_id: Option<&str>,
) -> Result<(), HubStoreError> {
    let mut first_match = None;
    for (stored, identity) in matches(transaction, request)? {
        if let Some(stored) = stored {
            let stored_id = stored.metadata.id.clone();
            read::validate_stored(transaction, stored)?;
            if Some(stored_id.as_str()) != allowed_request_id {
                first_match.get_or_insert(identity);
            }
        }
    }
    first_match.map_or(Ok(()), |identity| {
        Err(conflict(&format!(
            "scheduled-node provider request {identity} belongs to another idempotency key"
        )))
    })
}

fn matches(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
) -> Result<Vec<(Option<rows::RawStoredRequest>, &'static str)>, HubStoreError> {
    Ok(vec![
        (
            rows::find_by_id(transaction, &request.provider_request_id)?,
            "provider request ID",
        ),
        (
            rows::find_by_run(transaction, &request.graph_run_id)?,
            "Graph Run",
        ),
        (
            rows::find_by_schedule(transaction, &request.schedule_id)?,
            "schedule",
        ),
        (
            rows::find_by_contract(transaction, &request.scheduled_contract_id)?,
            "scheduled contract",
        ),
        (
            rows::find_by_logical_request(transaction, &request.logical_request_id)?,
            "logical request",
        ),
        (
            rows::find_by_run_node_attempt(
                transaction,
                &request.graph_run_id,
                &request.node_id,
                request.attempt,
            )?,
            "Graph Run node attempt",
        ),
        (
            rows::find_by_schedule_ordinal_attempt(
                transaction,
                &request.schedule_id,
                request.execution_ordinal,
                request.attempt,
            )?,
            "schedule execution slot",
        ),
    ])
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentScheduledNodeProviderRequest,
        message: message.into(),
    }
}
