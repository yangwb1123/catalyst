use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    AdmitGroupAgentNodeExecutionContract, AdmitGroupAgentNodeExecutionContractDisposition,
    AdmitGroupAgentNodeExecutionContractResult, GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
    GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, GroupAgentNodeExecutionContractInspection,
    GroupAgentNodeExecutionContractRecord, HubEntity, HubStoreError,
};

use super::{
    super::{group_agent_graph, group_agent_graph_run, read_error, write_error},
    codec, read, rows, snapshot,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn admit(
    connection: &mut Connection,
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError> {
    admit_with_before_reread(connection, request, || Ok(()))
}

pub(super) fn admit_with_before_reread<F>(
    connection: &mut Connection,
    request: &AdmitGroupAgentNodeExecutionContract,
    before_reread: F,
) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = admit_locked(&transaction, request, before_reread)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn admit_locked<F>(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentNodeExecutionContract,
    before_reread: F,
) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        return replay(transaction, stored, request);
    }
    reject_existing_identity(transaction, request)?;
    let (run, graph) = load_source(transaction, &request.graph_run_id)?;
    validate_candidate(request, &run, &graph)?;
    create(transaction, request, before_reread)
}

fn replay(
    transaction: &Transaction<'_>,
    stored: rows::RawStoredContract,
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError> {
    let inspection = read::validate_stored(transaction, stored)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(
        transaction,
        &inspection.graph_run.run.graph_id,
    )?;
    validate_candidate(request, &inspection.graph_run, &graph)?;
    ensure_replay(&inspection, request)?;
    Ok(result(
        AdmitGroupAgentNodeExecutionContractDisposition::Replayed,
        inspection,
    ))
}

fn reject_existing_identity(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<(), HubStoreError> {
    if let Some(stored) = rows::find_by_id(transaction, &request.contract.contract_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "contract ID already belongs to another idempotency key",
        ));
    }
    if let Some(stored) = rows::find_by_run(transaction, &request.graph_run_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "Graph Run already has a contract under another idempotency key",
        ));
    }
    Ok(())
}

fn load_source(
    transaction: &Transaction<'_>,
    graph_run_id: &str,
) -> Result<(GroupAgentGraphRunInspection, GroupAgentGraphInspection), HubStoreError> {
    let run = group_agent_graph_run::read::inspect_in_snapshot(transaction, graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(transaction, &run.run.graph_id)?;
    Ok((run, graph))
}

fn validate_candidate(
    request: &AdmitGroupAgentNodeExecutionContract,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(), HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let expected = snapshot::reconstruct(run, graph)?;
    snapshot::verify_candidate(
        &expected,
        &request.control_snapshot,
        &request.control_snapshot_json,
    )
}

fn create<F>(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentNodeExecutionContract,
    before_reread: F,
) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let encoded = codec::encode_candidate(request)?;
    let record = record(request, &encoded);
    insert_contract(transaction, request, &record, &encoded)?;
    insert_event(transaction, request, &encoded)?;
    transition_run(transaction, request, encoded.event_bytes.len())?;
    before_reread()?;
    let inspection = read::inspect_in_snapshot(transaction, &record.contract_id)?;
    ensure_committed(&inspection, request, &record)?;
    Ok(result(
        AdmitGroupAgentNodeExecutionContractDisposition::Created,
        inspection,
    ))
}

fn record(
    request: &AdmitGroupAgentNodeExecutionContract,
    encoded: &codec::EncodedAdmission,
) -> GroupAgentNodeExecutionContractRecord {
    GroupAgentNodeExecutionContractRecord {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        contract_id: request.contract.contract_id.clone(),
        graph_run_id: request.graph_run_id.clone(),
        node_id: request.contract.node.node_id.clone(),
        attempt: request.contract.node.attempt,
        control_snapshot_sha256: request.control_snapshot.snapshot_sha256.clone(),
        contract_sha256: request.contract.contract_sha256.clone(),
        contract_bytes: encoded.contract_bytes.len(),
        request_sha256: request.contract.request.request_sha256.clone(),
        project_lane_sha256: request.contract.node.project_lane_sha256.clone(),
        expected_last_event_seq: request.contract.expected_last_event_seq,
        expected_last_event_sha256: request.contract.expected_last_event_sha256.clone(),
        created_at_ms: request.admitted_at_ms,
    }
}

fn insert_contract(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentNodeExecutionContract,
    record: &GroupAgentNodeExecutionContractRecord,
    encoded: &codec::EncodedAdmission,
) -> Result<(), HubStoreError> {
    let snapshot_digest =
        codec::candidate_digest(&record.control_snapshot_sha256, "control snapshot")?;
    let request_digest = codec::candidate_digest(&record.request_sha256, "request")?;
    let lane_digest = codec::candidate_digest(&record.project_lane_sha256, "project lane")?;
    let head_digest =
        codec::candidate_digest(&record.expected_last_event_sha256, "expected last event")?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_node_execution_contracts(
               id,graph_run_id,contract_version,node_id,attempt,control_snapshot_sha256,
               contract_blob,contract_bytes,contract_sha256,request_sha256,
               project_lane_sha256,expected_last_event_seq,expected_last_event_sha256,
               idempotency_key,created_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                record.contract_id,
                record.graph_run_id,
                i64::from(record.v),
                record.node_id,
                i64::from(record.attempt),
                snapshot_digest.as_slice(),
                encoded.contract_bytes.as_slice(),
                to_i64(record.contract_bytes, "contract byte count")?,
                encoded.contract_digest.as_slice(),
                request_digest.as_slice(),
                lane_digest.as_slice(),
                to_i64(record.expected_last_event_seq, "expected event sequence")?,
                head_digest.as_slice(),
                request.idempotency_key,
                to_i64(record.created_at_ms, "creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeExecutionContract, error))?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentNodeExecutionContract,
    encoded: &codec::EncodedAdmission,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,
               event_sha256,created_at_ms
             ) VALUES(?1,2,?2,'node_execution_contract_admitted',?3,?4,?5,?6)",
            params![
                request.graph_run_id,
                i64::from(request.event.v),
                encoded.event_bytes.as_slice(),
                to_i64(encoded.event_bytes.len(), "event byte count")?,
                encoded.event_digest.as_slice(),
                to_i64(request.admitted_at_ms, "event creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeExecutionContract, error))?;
    Ok(())
}

fn transition_run(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentNodeExecutionContract,
    event_bytes: usize,
) -> Result<(), HubStoreError> {
    let changed = transaction
        .execute(
            "UPDATE group_agent_graph_runs
             SET run_version = 2,status = 'awaiting_core_dispatch',
                 execution_contract_present = 1,dispatch_authority_released = 0,
                 last_event_seq = 2,journal_bytes = journal_bytes + ?1
             WHERE id = ?2 AND run_version = 1
               AND status = 'awaiting_execution_contract'
               AND execution_contract_present = 0
               AND dispatch_authority_released = 0
               AND last_event_seq = ?3",
            params![
                to_i64(event_bytes, "event byte count")?,
                request.graph_run_id,
                to_i64(
                    request.contract.expected_last_event_seq,
                    "expected event sequence",
                )?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentNodeExecutionContract, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(conflict(
            "Graph Run cursor or execution-contract state changed",
        ))
    }
}

fn ensure_replay(
    inspection: &GroupAgentNodeExecutionContractInspection,
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<(), HubStoreError> {
    let exact = inspection.record.graph_run_id == request.graph_run_id
        && inspection.contract == request.contract
        && inspection.contract_json.as_bytes() == request.contract_json.as_bytes();
    exact
        .then_some(())
        .ok_or_else(|| conflict("idempotency key was reused with different contract input"))
}

fn ensure_committed(
    inspection: &GroupAgentNodeExecutionContractInspection,
    request: &AdmitGroupAgentNodeExecutionContract,
    record: &GroupAgentNodeExecutionContractRecord,
) -> Result<(), HubStoreError> {
    let exact = inspection.v == GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION
        && inspection.record == *record
        && inspection.contract == request.contract
        && inspection.contract_json.as_bytes() == request.contract_json.as_bytes()
        && inspection.admission_event == request.event
        && inspection.admission_event_json.as_bytes() == request.event_json.as_bytes()
        && inspection.graph_run.run.v == GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("persisted Node Execution Contract disagrees with its candidate"))
}

fn result(
    disposition: AdmitGroupAgentNodeExecutionContractDisposition,
    inspection: GroupAgentNodeExecutionContractInspection,
) -> AdmitGroupAgentNodeExecutionContractResult {
    AdmitGroupAgentNodeExecutionContractResult {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        disposition,
        inspection,
    }
}

fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| {
        conflict(&format!(
            "invalid Node Execution Contract {subject}: {error}"
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
        entity: HubEntity::GroupAgentNodeExecutionContract,
        message: message.into(),
    }
}
