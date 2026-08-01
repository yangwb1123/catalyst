use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION, GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
    GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchRequestInspection, GroupAgentNodeDispatchRequestRecord,
    GroupAgentNodeExecutionContractInspection, GroupAgentNodeProviderKind, HubEntity,
    HubStoreError, PrepareGroupAgentNodeDispatchRequest,
    PrepareGroupAgentNodeDispatchRequestDisposition, PrepareGroupAgentNodeDispatchRequestResult,
};

use super::super::super::group_agent_graph_run;
use super::super::super::{read_error, write_error};
use super::super::read as contract_read;
use super::{codec, read, rows};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn prepare(
    connection: &mut Connection,
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError> {
    prepare_with_before_reread(connection, request, || Ok(()))
}

pub(super) fn prepare_with_before_reread<F>(
    connection: &mut Connection,
    request: &PrepareGroupAgentNodeDispatchRequest,
    before_reread: F,
) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError>
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
    request: &PrepareGroupAgentNodeDispatchRequest,
    before_reread: F,
) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        return replay(transaction, stored, request);
    }
    reject_existing_identity(transaction, request)?;
    let contract = load_source(transaction, &request.graph_run_id, &request.contract_id)?;
    validate_candidate(request, &contract)?;
    create(transaction, request, before_reread)
}

fn replay(
    transaction: &Transaction<'_>,
    stored: rows::RawStoredRequest,
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError> {
    let inspection = read::validate_stored(transaction, stored)?;
    validate_candidate(request, &inspection.contract)?;
    ensure_replay(&inspection, request)?;
    Ok(result(
        PrepareGroupAgentNodeDispatchRequestDisposition::Replayed,
        inspection,
    ))
}

fn reject_existing_identity(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<(), HubStoreError> {
    if let Some(stored) = rows::find_by_id(transaction, &request.dispatch_request_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "dispatch request ID belongs to another idempotency key",
        ));
    }
    if let Some(stored) = rows::find_by_run(transaction, &request.graph_run_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "Graph Run already has a dispatch request under another key",
        ));
    }
    if let Some(stored) = rows::find_by_contract(transaction, &request.contract_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "Node Execution Contract already has a dispatch request under another key",
        ));
    }
    Ok(())
}

fn load_source(
    transaction: &Transaction<'_>,
    graph_run_id: &str,
    contract_id: &str,
) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
    group_agent_graph_run::read::inspect_in_snapshot(transaction, graph_run_id)?;
    contract_read::inspect_in_snapshot(transaction, contract_id).map_err(candidate_source_error)
}

fn validate_candidate(
    request: &PrepareGroupAgentNodeDispatchRequest,
    contract: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    validate_source_state(contract)?;
    let admission_head = contract
        .admission_event
        .expected_sha256()
        .map_err(|error| corrupt(&error.to_string()))?;
    let record = &contract.record;
    let source = &contract.contract;
    let exact = request.graph_run_id == record.graph_run_id
        && request.contract_id == record.contract_id
        && request.contract_sha256 == record.contract_sha256
        && request.node_id == record.node_id
        && request.attempt == record.attempt
        && request.request_sha256 == record.request_sha256
        && request.project_lane_sha256 == record.project_lane_sha256
        && request.provider == source.provider.kind
        && request.endpoint == source.provider.endpoint
        && request.model == source.provider.model
        && request.pricing_snapshot_sha256 == source.budgets.pricing_snapshot_sha256
        && request.expected_last_event_seq == 2
        && request.expected_last_event_sha256 == admission_head;
    if !exact {
        return Err(conflict(
            "dispatch request does not exactly bind its admitted contract",
        ));
    }
    read::validate_exact_provider_body(source, &request.provider_request_body)
        .map_err(candidate_codec_error)
}

fn validate_source_state(
    contract: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), HubStoreError> {
    let run = &contract.graph_run.run;
    let admitted = run.v == GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingCoreDispatch
        && !run.dispatch_request_present
        && run.last_event_seq == 2;
    let replayable = run.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingDispatchAuthorization
        && run.dispatch_request_present
        && run.last_event_seq == 3;
    if run.execution_contract_present
        && !run.dispatch_authority_released
        && (admitted || replayable)
    {
        Ok(())
    } else {
        Err(conflict(
            "dispatch preparation requires an exact v2 or replayable v3 state",
        ))
    }
}

fn create<F>(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentNodeDispatchRequest,
    before_reread: F,
) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let encoded = codec::encode_candidate(request)?;
    let record = record(request);
    insert_request(transaction, request, &record)?;
    insert_event(transaction, request, &encoded)?;
    transition_run(transaction, request, encoded.event_bytes.len())?;
    before_reread()?;
    let inspection = read::inspect_in_snapshot(transaction, &record.dispatch_request_id)?;
    ensure_committed(&inspection, request, &record)?;
    Ok(result(
        PrepareGroupAgentNodeDispatchRequestDisposition::Created,
        inspection,
    ))
}

fn record(request: &PrepareGroupAgentNodeDispatchRequest) -> GroupAgentNodeDispatchRequestRecord {
    GroupAgentNodeDispatchRequestRecord {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        dispatch_request_id: request.dispatch_request_id.clone(),
        graph_run_id: request.graph_run_id.clone(),
        contract_id: request.contract_id.clone(),
        node_id: request.node_id.clone(),
        attempt: request.attempt,
        contract_sha256: request.contract_sha256.clone(),
        request_sha256: request.request_sha256.clone(),
        project_lane_sha256: request.project_lane_sha256.clone(),
        provider: request.provider,
        endpoint: request.endpoint.clone(),
        model: request.model.clone(),
        pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
        provider_request_sha256: request.provider_request_sha256.clone(),
        provider_request_bytes: request.provider_request_body.len(),
        destination_sha256: request.destination_sha256.clone(),
        dispatch_request_sha256: request.dispatch_request_sha256.clone(),
        codec_protocol_version: request.codec_protocol_version,
        expected_last_event_seq: request.expected_last_event_seq,
        expected_last_event_sha256: request.expected_last_event_sha256.clone(),
        created_at_ms: request.prepared_at_ms,
    }
}

fn insert_request(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentNodeDispatchRequest,
    record: &GroupAgentNodeDispatchRequestRecord,
) -> Result<(), HubStoreError> {
    let digests = record_digests(record)?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_node_dispatch_requests(
               id,graph_run_id,contract_id,request_version,codec_protocol_version,node_id,
               attempt,contract_sha256,request_sha256,project_lane_sha256,provider_kind,
               endpoint,model,destination_sha256,pricing_snapshot_sha256,
               provider_request_blob,provider_request_bytes,provider_request_sha256,
               dispatch_request_sha256,expected_last_event_seq,expected_last_event_sha256,
               idempotency_key,created_at_ms
             ) VALUES(
               ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,
               ?18,?19,?20,?21,?22,?23
             )",
            params![
                record.dispatch_request_id,
                record.graph_run_id,
                record.contract_id,
                i64::from(record.v),
                i64::from(record.codec_protocol_version),
                record.node_id,
                i64::from(record.attempt),
                digests.contract.as_slice(),
                digests.request.as_slice(),
                digests.project_lane.as_slice(),
                provider_kind(record.provider),
                record.endpoint,
                record.model,
                digests.destination.as_slice(),
                digests.pricing.as_slice(),
                request.provider_request_body.as_slice(),
                to_i64(record.provider_request_bytes, "provider request byte count")?,
                digests.provider_request.as_slice(),
                digests.dispatch_request.as_slice(),
                to_i64(record.expected_last_event_seq, "expected event sequence")?,
                digests.expected_head.as_slice(),
                request.idempotency_key,
                to_i64(record.created_at_ms, "creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeDispatchRequest, error))?;
    Ok(())
}

struct RecordDigests {
    contract: [u8; 32],
    request: [u8; 32],
    project_lane: [u8; 32],
    destination: [u8; 32],
    pricing: [u8; 32],
    provider_request: [u8; 32],
    dispatch_request: [u8; 32],
    expected_head: [u8; 32],
}

fn record_digests(
    record: &GroupAgentNodeDispatchRequestRecord,
) -> Result<RecordDigests, HubStoreError> {
    Ok(RecordDigests {
        contract: codec::candidate_digest(&record.contract_sha256, "contract")?,
        request: codec::candidate_digest(&record.request_sha256, "logical request")?,
        project_lane: codec::candidate_digest(&record.project_lane_sha256, "project lane")?,
        destination: codec::candidate_digest(&record.destination_sha256, "destination")?,
        pricing: codec::candidate_digest(&record.pricing_snapshot_sha256, "pricing snapshot")?,
        provider_request: codec::candidate_digest(
            &record.provider_request_sha256,
            "provider request",
        )?,
        dispatch_request: codec::candidate_digest(
            &record.dispatch_request_sha256,
            "dispatch request",
        )?,
        expected_head: codec::candidate_digest(
            &record.expected_last_event_sha256,
            "expected event head",
        )?,
    })
}

fn insert_event(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentNodeDispatchRequest,
    encoded: &codec::EncodedPreparation,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,
               event_sha256,created_at_ms
             ) VALUES(?1,3,?2,'node_dispatch_request_prepared',?3,?4,?5,?6)",
            params![
                request.graph_run_id,
                i64::from(request.event.v),
                encoded.event_bytes.as_slice(),
                to_i64(encoded.event_bytes.len(), "event byte count")?,
                encoded.event_digest.as_slice(),
                to_i64(request.prepared_at_ms, "event creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeDispatchRequest, error))?;
    Ok(())
}

fn transition_run(
    transaction: &Transaction<'_>,
    request: &PrepareGroupAgentNodeDispatchRequest,
    event_bytes: usize,
) -> Result<(), HubStoreError> {
    let head = codec::candidate_digest(&request.expected_last_event_sha256, "event head")?;
    let changed = transaction
        .execute(
            "UPDATE group_agent_graph_runs
             SET run_version = 3,status = 'awaiting_dispatch_authorization',
                 execution_contract_present = 1,dispatch_request_present = 1,
                 dispatch_authority_released = 0,last_event_seq = 3,
                 journal_bytes = journal_bytes + ?1
             WHERE id = ?2 AND run_version = 2 AND status = 'awaiting_core_dispatch'
               AND execution_contract_present = 1 AND dispatch_request_present = 0
               AND dispatch_authority_released = 0 AND last_event_seq = ?3
               AND EXISTS(
                 SELECT 1 FROM group_agent_graph_run_events
                 WHERE graph_run_id = ?2 AND seq = ?3 AND event_sha256 = ?4
               )",
            params![
                to_i64(event_bytes, "event byte count")?,
                request.graph_run_id,
                to_i64(request.expected_last_event_seq, "expected event sequence")?,
                head.as_slice(),
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeDispatchRequest, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(conflict(
            "Graph Run cursor, journal head, or dispatch-request state changed",
        ))
    }
}

fn ensure_replay(
    inspection: &GroupAgentNodeDispatchRequestInspection,
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<(), HubStoreError> {
    let exact = inspection.record.dispatch_request_id == request.dispatch_request_id
        && inspection.record.dispatch_request_sha256 == request.dispatch_request_sha256
        && inspection.provider_request_body == request.provider_request_body;
    exact
        .then_some(())
        .ok_or_else(|| conflict("idempotency key was reused with different request input"))
}

fn ensure_committed(
    inspection: &GroupAgentNodeDispatchRequestInspection,
    request: &PrepareGroupAgentNodeDispatchRequest,
    record: &GroupAgentNodeDispatchRequestRecord,
) -> Result<(), HubStoreError> {
    let exact = inspection.v == GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION
        && inspection.record == *record
        && inspection.provider_request_body == request.provider_request_body
        && inspection.preparation_event == request.event
        && inspection.preparation_event_json.as_bytes() == request.event_json.as_bytes()
        && inspection.contract.graph_run.run.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("persisted Node Dispatch Request disagrees with its candidate"))
}

fn result(
    disposition: PrepareGroupAgentNodeDispatchRequestDisposition,
    inspection: GroupAgentNodeDispatchRequestInspection,
) -> PrepareGroupAgentNodeDispatchRequestResult {
    PrepareGroupAgentNodeDispatchRequestResult {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        disposition,
        inspection,
    }
}

fn provider_kind(provider: GroupAgentNodeProviderKind) -> &'static str {
    match provider {
        GroupAgentNodeProviderKind::OpenAiResponses => "openai_responses",
    }
}

fn candidate_source_error(error: HubStoreError) -> HubStoreError {
    match error {
        HubStoreError::NotFound { .. } => {
            conflict("dispatch request references a missing contract")
        }
        other => other,
    }
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
    i64::try_from(value)
        .map_err(|error| conflict(&format!("invalid Node Dispatch Request {subject}: {error}")))
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentNodeDispatchRequest,
        message: message.into(),
    }
}
