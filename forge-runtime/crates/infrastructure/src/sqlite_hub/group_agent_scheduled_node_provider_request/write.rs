use std::time::Duration;

use rusqlite::{
    Connection, Transaction, TransactionBehavior, params, params_from_iter, types::Value,
};

use crate::runtime_domain::{
    GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION, GroupAgentNodeProviderKind,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledNodeProviderRequestRecord, HubEntity, HubStoreError,
    PrepareGroupAgentScheduledNodeProviderRequest,
    PrepareGroupAgentScheduledNodeProviderRequestDisposition,
    PrepareGroupAgentScheduledNodeProviderRequestResult,
};

use super::super::{
    group_agent_scheduled_node_contract, group_agent_scheduled_node_successor, read_error,
    write_error,
};
use super::{identity, read, rows};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const INSERT_REQUEST_SQL: &str = "INSERT INTO group_agent_graph_scheduled_node_provider_requests(
       id,graph_run_id,schedule_id,scheduled_contract_id,
       provider_request_version,codec_protocol_version,execution_ordinal,node_id,attempt,
       scheduled_contract_sha256,logical_request_id,logical_request_sha256,
       schedule_sha256,project_lane_sha256,provider_kind,endpoint,model,
       destination_sha256,pricing_snapshot_sha256,provider_request_blob,
       provider_request_bytes,provider_request_sha256,prepared_request_sha256,
       expected_last_event_seq,expected_last_event_sha256,provider_request_prepared,
       provider_request_sent,lifecycle_contract_admitted,execution_authority_released,
       dispatch_authority_released,project_lane_claimed,progress_observed,
       successor_advance_authorized,idempotency_key,created_at_ms
     )
     SELECT
       ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,
       ?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,
       ?33,?34,?35
     WHERE EXISTS(
       SELECT 1
       FROM group_agent_graph_runs AS run
       JOIN group_agent_graph_run_events AS event
         ON event.graph_run_id=run.id AND event.seq=run.last_event_seq
       WHERE run.id=?2 AND run.run_version=1
         AND run.status='awaiting_execution_contract'
         AND run.execution_contract_present=0
         AND run.dispatch_request_present=0
         AND run.dispatch_authority_released=0
         AND run.last_event_seq=?24 AND event.event_sha256=?25
         AND (
           (
             EXISTS(
               SELECT 1 FROM group_agent_graph_scheduled_node_contract_candidates AS contract
               WHERE contract.graph_run_id=run.id AND contract.id=?4
                 AND contract.schedule_id=?3 AND contract.contract_sha256=?10
             )
             OR
             EXISTS(
               SELECT 1 FROM group_agent_graph_scheduled_node_successor_candidates AS successor
               WHERE successor.graph_run_id=run.id AND successor.id=?4
                 AND successor.schedule_id=?3 AND successor.contract_sha256=?10
             )
           )
           AND (
             EXISTS(
               SELECT 1 FROM group_agent_graph_scheduled_node_contract_candidates AS contract
               WHERE contract.id=?4 AND contract.provider_request_present=0
                 AND contract.execution_authority_released=0
                 AND contract.dispatch_authority_released=0
                 AND contract.progress_observed=0
                 AND contract.successor_advance_authorized=0
             )
             OR
             EXISTS(
               SELECT 1 FROM group_agent_graph_scheduled_node_successor_candidates AS successor
               WHERE successor.id=?4 AND successor.provider_request_present=0
                 AND successor.execution_authority_released=0
                 AND successor.dispatch_authority_released=0
                 AND successor.progress_observed=0
                 AND successor.successor_advance_authorized=0
             )
           )
     )
     )";

pub(super) fn prepare(
    connection: &mut Connection,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError> {
    prepare_with_before_reread(connection, request, || Ok(()))
}

pub(super) fn prepare_with_before_reread<F>(
    connection: &mut Connection,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    before_reread: F,
) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = prepare_locked(&transaction, request, before_reread)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn prepare_locked<F>(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    before_reread: F,
) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let inspection = read::validate_stored(transaction, stored)?;
        identity::reject_existing(
            transaction,
            request,
            Some(&inspection.record.provider_request_id),
        )?;
        validate_candidate(request, &inspection.scheduled_contract)?;
        require_pristine_source(transaction, request)?;
        ensure_replay(&inspection, request)?;
        return Ok(result(
            PrepareGroupAgentScheduledNodeProviderRequestDisposition::Replayed,
            inspection,
        ));
    }
    identity::reject_existing(transaction, request, None)?;
    let scheduled_contract = load_source(transaction, &request.scheduled_contract_id)?;
    validate_candidate(request, &scheduled_contract)?;
    require_pristine_source(transaction, request)?;
    create(transaction, request, &scheduled_contract, before_reread)
}

fn load_source(
    transaction: &Transaction<'_>,
    scheduled_contract_id: &str,
) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
    match group_agent_scheduled_node_contract::read::inspect_in_snapshot(
        transaction,
        scheduled_contract_id,
    ) {
        Ok(inspection) => Ok(inspection),
        Err(HubStoreError::NotFound { .. }) => {
            group_agent_scheduled_node_successor::read::inspect_in_snapshot(
                transaction,
                scheduled_contract_id,
            )
            .map_err(|error| match error {
                HubStoreError::NotFound { .. } => {
                    conflict("scheduled-node provider request references a missing contract")
                }
                other => other,
            })
        }
        Err(other) => Err(other),
    }
}

fn validate_candidate(
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    scheduled_contract: &GroupAgentScheduledNodeContractInspection,
) -> Result<(), HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let record = &scheduled_contract.record;
    let candidate = &scheduled_contract.candidate;
    let exact = request.graph_run_id == record.graph_run_id
        && request.schedule_id == record.schedule_id
        && request.scheduled_contract_id == record.contract_id
        && request.scheduled_contract_sha256 == record.contract_sha256
        && request.execution_ordinal == record.execution_ordinal
        && request.node_id == record.node_id
        && request.attempt == record.attempt
        && request.logical_request_id == record.request_id
        && request.logical_request_sha256 == record.request_sha256
        && request.schedule_sha256 == record.schedule_sha256
        && request.project_lane_sha256 == record.project_lane_sha256
        && request.provider == candidate.provider.kind
        && request.endpoint == candidate.provider.endpoint
        && request.model == candidate.provider.model
        && request.pricing_snapshot_sha256 == candidate.budgets.pricing_snapshot_sha256
        && request.expected_last_event_seq == record.expected_last_event_seq
        && request.expected_last_event_sha256 == record.expected_last_event_sha256;
    if !exact {
        return Err(conflict(
            "scheduled-node provider request does not exactly bind its scheduled contract",
        ));
    }
    read::validate_exact_provider_body(scheduled_contract, &request.provider_request_body)
        .map_err(candidate_codec_error)
}

fn require_pristine_source(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
) -> Result<(), HubStoreError> {
    let head = read::candidate_digest(&request.expected_last_event_sha256, "expected event head")?;
    let pristine: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM group_agent_graph_runs AS run
               JOIN group_agent_graph_run_events AS event
                 ON event.graph_run_id=run.id AND event.seq=run.last_event_seq
               WHERE run.id=?1 AND run.run_version=1
                 AND run.status='awaiting_execution_contract'
                 AND run.execution_contract_present=0
                 AND run.dispatch_request_present=0
                 AND run.dispatch_authority_released=0
                 AND run.last_event_seq=?2 AND event.event_sha256=?3
             )",
            params![
                request.graph_run_id,
                to_i64(request.expected_last_event_seq, "expected event sequence")?,
                head.as_slice(),
            ],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    pristine.then_some(()).ok_or_else(|| {
        conflict("scheduled-node provider request requires the exact pristine v1 Run")
    })
}

fn create<F>(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    scheduled_contract: &GroupAgentScheduledNodeContractInspection,
    before_reread: F,
) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let record = record(request);
    insert_request(transaction, request, &record)?;
    before_reread()?;
    let inspection = read::inspect_in_snapshot(transaction, &record.provider_request_id)?;
    ensure_committed(&inspection, request, &record, scheduled_contract)?;
    Ok(result(
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created,
        inspection,
    ))
}

fn record(
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
) -> GroupAgentScheduledNodeProviderRequestRecord {
    GroupAgentScheduledNodeProviderRequestRecord {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        provider_request_id: request.provider_request_id.clone(),
        graph_run_id: request.graph_run_id.clone(),
        schedule_id: request.schedule_id.clone(),
        scheduled_contract_id: request.scheduled_contract_id.clone(),
        execution_ordinal: request.execution_ordinal,
        node_id: request.node_id.clone(),
        attempt: request.attempt,
        scheduled_contract_sha256: request.scheduled_contract_sha256.clone(),
        logical_request_id: request.logical_request_id.clone(),
        logical_request_sha256: request.logical_request_sha256.clone(),
        schedule_sha256: request.schedule_sha256.clone(),
        project_lane_sha256: request.project_lane_sha256.clone(),
        provider: request.provider,
        endpoint: request.endpoint.clone(),
        model: request.model.clone(),
        destination_sha256: request.destination_sha256.clone(),
        pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
        provider_request_sha256: request.provider_request_sha256.clone(),
        provider_request_bytes: request.provider_request_body.len(),
        prepared_request_sha256: request.prepared_request_sha256.clone(),
        codec_protocol_version: request.codec_protocol_version,
        expected_last_event_seq: request.expected_last_event_seq,
        expected_last_event_sha256: request.expected_last_event_sha256.clone(),
        provider_request_prepared: request.provider_request_prepared,
        provider_request_sent: request.provider_request_sent,
        lifecycle_contract_admitted: request.lifecycle_contract_admitted,
        execution_authority_released: request.execution_authority_released,
        dispatch_authority_released: request.dispatch_authority_released,
        project_lane_claimed: request.project_lane_claimed,
        progress_observed: request.progress_observed,
        successor_advance_authorized: request.successor_advance_authorized,
        created_at_ms: request.prepared_at_ms,
    }
}

fn insert_request(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    record: &GroupAgentScheduledNodeProviderRequestRecord,
) -> Result<(), HubStoreError> {
    let digests = record_digests(record)?;
    let values = insert_values(request, record, &digests)?;
    let changed = transaction
        .execute(INSERT_REQUEST_SQL, params_from_iter(values))
        .map_err(|error| write_error(HubEntity::GroupAgentScheduledNodeProviderRequest, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(conflict(
            "scheduled contract, Graph Run cursor, or sequence-1 journal head changed",
        ))
    }
}

fn insert_values(
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    record: &GroupAgentScheduledNodeProviderRequestRecord,
    digests: &RecordDigests,
) -> Result<Vec<Value>, HubStoreError> {
    Ok(vec![
        record.provider_request_id.clone().into(),
        record.graph_run_id.clone().into(),
        record.schedule_id.clone().into(),
        record.scheduled_contract_id.clone().into(),
        i64::from(record.v).into(),
        i64::from(record.codec_protocol_version).into(),
        to_i64(record.execution_ordinal, "execution ordinal")?.into(),
        record.node_id.clone().into(),
        i64::from(record.attempt).into(),
        digests.scheduled_contract.to_vec().into(),
        record.logical_request_id.clone().into(),
        digests.logical_request.to_vec().into(),
        digests.schedule.to_vec().into(),
        digests.project_lane.to_vec().into(),
        provider_kind(record.provider).to_owned().into(),
        record.endpoint.clone().into(),
        record.model.clone().into(),
        digests.destination.to_vec().into(),
        digests.pricing.to_vec().into(),
        request.provider_request_body.clone().into(),
        to_i64(record.provider_request_bytes, "provider request bytes")?.into(),
        digests.provider_request.to_vec().into(),
        digests.prepared_request.to_vec().into(),
        to_i64(record.expected_last_event_seq, "expected event sequence")?.into(),
        digests.expected_head.to_vec().into(),
        bool_i64(record.provider_request_prepared).into(),
        bool_i64(record.provider_request_sent).into(),
        bool_i64(record.lifecycle_contract_admitted).into(),
        bool_i64(record.execution_authority_released).into(),
        bool_i64(record.dispatch_authority_released).into(),
        bool_i64(record.project_lane_claimed).into(),
        bool_i64(record.progress_observed).into(),
        bool_i64(record.successor_advance_authorized).into(),
        request.idempotency_key.clone().into(),
        to_i64(record.created_at_ms, "creation time")?.into(),
    ])
}

struct RecordDigests {
    scheduled_contract: [u8; 32],
    logical_request: [u8; 32],
    schedule: [u8; 32],
    project_lane: [u8; 32],
    destination: [u8; 32],
    pricing: [u8; 32],
    provider_request: [u8; 32],
    prepared_request: [u8; 32],
    expected_head: [u8; 32],
}

fn record_digests(
    record: &GroupAgentScheduledNodeProviderRequestRecord,
) -> Result<RecordDigests, HubStoreError> {
    Ok(RecordDigests {
        scheduled_contract: read::candidate_digest(
            &record.scheduled_contract_sha256,
            "scheduled contract",
        )?,
        logical_request: read::candidate_digest(&record.logical_request_sha256, "logical request")?,
        schedule: read::candidate_digest(&record.schedule_sha256, "schedule")?,
        project_lane: read::candidate_digest(&record.project_lane_sha256, "project lane")?,
        destination: read::candidate_digest(&record.destination_sha256, "destination")?,
        pricing: read::candidate_digest(&record.pricing_snapshot_sha256, "pricing snapshot")?,
        provider_request: read::candidate_digest(
            &record.provider_request_sha256,
            "provider request",
        )?,
        prepared_request: read::candidate_digest(
            &record.prepared_request_sha256,
            "prepared request",
        )?,
        expected_head: read::candidate_digest(
            &record.expected_last_event_sha256,
            "expected event head",
        )?,
    })
}

fn ensure_replay(
    inspection: &GroupAgentScheduledNodeProviderRequestInspection,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
) -> Result<(), HubStoreError> {
    let mut expected = record(request);
    expected.created_at_ms = inspection.record.created_at_ms;
    let exact = inspection.record == expected
        && inspection.provider_request_body == request.provider_request_body;
    exact.then_some(()).ok_or_else(|| {
        conflict("idempotency key was reused with different scheduled provider-request input")
    })
}

fn ensure_committed(
    inspection: &GroupAgentScheduledNodeProviderRequestInspection,
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    record: &GroupAgentScheduledNodeProviderRequestRecord,
    scheduled_contract: &GroupAgentScheduledNodeContractInspection,
) -> Result<(), HubStoreError> {
    let exact = inspection.v == GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION
        && inspection.record == *record
        && inspection.provider_request_body == request.provider_request_body
        && inspection.scheduled_contract == *scheduled_contract;
    exact.then_some(()).ok_or_else(|| {
        corrupt("persisted scheduled-node provider request disagrees with its candidate")
    })
}

fn result(
    disposition: PrepareGroupAgentScheduledNodeProviderRequestDisposition,
    inspection: GroupAgentScheduledNodeProviderRequestInspection,
) -> PrepareGroupAgentScheduledNodeProviderRequestResult {
    PrepareGroupAgentScheduledNodeProviderRequestResult {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        disposition,
        inspection,
    }
}

fn provider_kind(provider: GroupAgentNodeProviderKind) -> &'static str {
    match provider {
        GroupAgentNodeProviderKind::OpenAiResponses => "openai_responses",
    }
}

const fn bool_i64(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn candidate_codec_error(error: HubStoreError) -> HubStoreError {
    match error {
        HubStoreError::Corrupt { message } => conflict(&message),
        other => other,
    }
}

fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| {
        conflict(&format!(
            "invalid scheduled-node provider request {subject}: {error}"
        ))
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentScheduledNodeProviderRequest,
        message: message.into(),
    }
}
