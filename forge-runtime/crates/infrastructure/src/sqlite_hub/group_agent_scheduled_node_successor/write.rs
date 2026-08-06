use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    AdmitGroupAgentScheduledNodeContractCandidate, AdmitGroupAgentScheduledNodeContractDisposition,
    AdmitGroupAgentScheduledNodeContractResult, GROUP_AGENT_GRAPH_RUN_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphInspection, GroupAgentGraphRunInspection,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractRecord,
    GroupAgentScheduledNodeContractScope, HubEntity, HubStoreError,
};

use super::super::{
    group_agent_graph, group_agent_graph_run, group_agent_node_execution_contract, group_run_codec,
    read_error, write_error,
};
use super::{read, rows};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const INSERT_CANDIDATE_SQL: &str =
    "INSERT INTO group_agent_graph_scheduled_node_successor_candidates(
       id,graph_run_id,graph_id,schedule_id,contract_version,
       scheduler_protocol_version,node_execution_protocol_version,
       execution_schedule_protocol_version,contract_scope,control_snapshot_sha256,
       schedule_sha256,expected_last_event_seq,expected_last_event_sha256,
       execution_ordinal,node_id,authored_node_index,topology_wave_index,attempt,
       project_lane_sha256,request_id,request_sha256,
       required_predecessor_node_count,predecessor_receipt_count,
       lifecycle_contract_admitted,provider_request_present,
       execution_authority_released,dispatch_authority_released,progress_observed,
       successor_advance_authorized,contract_blob,contract_bytes,contract_sha256,
       idempotency_key,created_at_ms
     ) VALUES(
       ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,
       ?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,
       ?33,?34
     )";

struct InsertCandidateValues {
    control: [u8; 32],
    schedule: [u8; 32],
    contract: [u8; 32],
    logical_request: [u8; 32],
    lane: [u8; 32],
    head: [u8; 32],
    expected_last_event_seq: i64,
    execution_ordinal: i64,
    authored_node_index: i64,
    topology_wave_index: i64,
    required_predecessor_count: i64,
    predecessor_receipt_count: i64,
    contract_bytes: i64,
    created_at_ms: i64,
}

pub(super) fn admit(
    connection: &mut Connection,
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
) -> Result<AdmitGroupAgentScheduledNodeContractResult, HubStoreError> {
    admit_with_before_reread(connection, request, || Ok(()))
}

pub(super) fn admit_with_before_reread<F>(
    connection: &mut Connection,
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
    before_reread: F,
) -> Result<AdmitGroupAgentScheduledNodeContractResult, HubStoreError>
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
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
    before_reread: F,
) -> Result<AdmitGroupAgentScheduledNodeContractResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let decoded = read::decode_stored(stored)?;
        let (run, graph, schedule) =
            load_sources(transaction, &decoded.inspection.record.graph_run_id)?;
        let inspection = read::validate_with_sources(decoded, &run, &graph, &schedule)?;
        validate_candidate(request, &run, &graph, &schedule)?;
        ensure_replay(&inspection, request)?;
        return Ok(result(
            AdmitGroupAgentScheduledNodeContractDisposition::Replayed,
            inspection,
        ));
    }
    reject_existing_candidate_identity(transaction, request)?;
    let (run, graph, schedule) = load_sources(transaction, &request.graph_run_id)?;
    validate_candidate(request, &run, &graph, &schedule)?;
    reject_legacy_contract(transaction, &request.graph_run_id)?;
    create(transaction, request, &run, &graph, &schedule, before_reread)
}

fn reject_existing_candidate_identity(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
) -> Result<(), HubStoreError> {
    let candidate = &request.candidate;
    // v21 (ADR-0035 wave-parallel): one candidate per (node, attempt) slot,
    // not per run or per schedule — same-wave siblings coexist in one run.
    // The per-run and per-schedule one-shot checks were the serial-chain
    // wall that deadlocked the second successor node; they are replaced by
    // the per-node and per-ordinal slots below (UNIQUE(graph_run_id,
    // node_id, attempt) / UNIQUE(schedule_id, execution_ordinal, attempt)).
    let matches = [
        (
            rows::find_by_id(transaction, &candidate.contract_id)?,
            "contract ID",
        ),
        (
            rows::find_by_request_id(transaction, &candidate.request.request_id)?,
            "request ID",
        ),
        (
            rows::find_by_run_node_attempt(
                transaction,
                &request.graph_run_id,
                &candidate.node.node_id,
                candidate.node.attempt,
            )?,
            "Graph Run/node/attempt slot",
        ),
        (
            rows::find_by_schedule_ordinal_attempt(
                transaction,
                &candidate.schedule_id,
                candidate.node.execution_ordinal,
                candidate.node.attempt,
            )?,
            "schedule/ordinal/attempt slot",
        ),
    ];
    reject_identity_matches(transaction, matches)
}

fn reject_identity_matches(
    transaction: &Transaction<'_>,
    matches: [(Option<rows::RawStoredCandidate>, &str); 4],
) -> Result<(), HubStoreError> {
    for (stored, identity) in matches {
        if let Some(stored) = stored {
            read::validate_stored(transaction, stored)?;
            return Err(conflict(&format!(
                "scheduled-node contract {identity} already belongs to another idempotency key"
            )));
        }
    }
    Ok(())
}

fn reject_legacy_contract(
    transaction: &Transaction<'_>,
    graph_run_id: &str,
) -> Result<(), HubStoreError> {
    if group_agent_node_execution_contract::read::validate_existing_for_run(
        transaction,
        graph_run_id,
    )? {
        Err(conflict(
            "Graph Run already belongs to the legacy contract-v1 family",
        ))
    } else {
        Ok(())
    }
}

fn load_sources(
    transaction: &Transaction<'_>,
    graph_run_id: &str,
) -> Result<
    (
        GroupAgentGraphRunInspection,
        GroupAgentGraphInspection,
        GroupAgentGraphExecutionScheduleInspection,
    ),
    HubStoreError,
> {
    let run = group_agent_graph_run::read::inspect_in_snapshot(transaction, graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(transaction, &run.run.graph_id)?;
    let schedule = group_agent_graph_run::schedule::read::validate_graph_run_binding(
        transaction,
        &run,
        &graph,
    )?
    .ok_or_else(|| conflict("Graph Run has no admitted execution schedule"))?;
    Ok((run, graph, schedule))
}

fn validate_candidate(
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
) -> Result<(), HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    if !is_pristine_source(run) {
        return Err(conflict(
            "scheduled-node contracts can only be admitted at the v1 Graph Run head",
        ));
    }
    let (control, control_json) =
        group_agent_node_execution_contract::snapshot::reconstruct(run, graph)?;
    let exact = request.control_snapshot == control
        && request.control_snapshot_json.as_bytes() == control_json.as_bytes()
        && request.schedule == schedule.schedule
        && request.schedule_json.as_bytes() == schedule.schedule_json.as_bytes()
        && request.candidate.schedule_id == schedule.record.schedule_id
        && request.candidate.schedule_sha256 == schedule.record.schedule_sha256;
    exact.then_some(()).ok_or_else(|| {
        conflict("scheduled-node contract sources do not exactly match durable state")
    })
}

fn is_pristine_source(run: &GroupAgentGraphRunInspection) -> bool {
    run.run.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && run.run.last_event_seq == 1
        && run.events.len() == 1
        && !run.run.execution_contract_present
        && !run.run.dispatch_request_present
        && !run.run.dispatch_authority_released
}

fn create<F>(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
    before_reread: F,
) -> Result<AdmitGroupAgentScheduledNodeContractResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let record = record(request);
    insert_candidate(transaction, request, &record)?;
    before_reread()?;
    let stored = rows::find_by_id(transaction, &record.contract_id)?
        .ok_or_else(|| corrupt("persisted scheduled-node contract disappeared"))?;
    let inspection =
        read::validate_with_sources(read::decode_stored(stored)?, run, graph, schedule)?;
    ensure_committed(&inspection, request, &record)?;
    Ok(result(
        AdmitGroupAgentScheduledNodeContractDisposition::Created,
        inspection,
    ))
}

fn record(
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
) -> GroupAgentScheduledNodeContractRecord {
    let candidate = &request.candidate;
    GroupAgentScheduledNodeContractRecord {
        v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
        contract_id: candidate.contract_id.clone(),
        graph_run_id: candidate.graph_run_id.clone(),
        schedule_id: candidate.schedule_id.clone(),
        node_id: candidate.node.node_id.clone(),
        execution_ordinal: candidate.node.execution_ordinal,
        attempt: candidate.node.attempt,
        control_snapshot_sha256: candidate.control_snapshot_sha256.clone(),
        schedule_sha256: candidate.schedule_sha256.clone(),
        contract_sha256: candidate.contract_sha256.clone(),
        contract_bytes: request.candidate_json.len(),
        request_id: candidate.request.request_id.clone(),
        request_sha256: candidate.request.request_sha256.clone(),
        project_lane_sha256: candidate.node.project_lane_sha256.clone(),
        expected_last_event_seq: candidate.expected_last_event_seq,
        expected_last_event_sha256: candidate.expected_last_event_sha256.clone(),
        predecessor_receipt_count: candidate.request.predecessor_terminal_receipts.len(),
        lifecycle_contract_admitted: candidate.lifecycle_contract_admitted,
        provider_request_present: candidate.provider_request_present,
        execution_authority_released: candidate.execution_authority_released,
        dispatch_authority_released: candidate.dispatch_authority_released,
        progress_observed: candidate.progress_observed,
        successor_advance_authorized: candidate.successor_advance_authorized,
        created_at_ms: request.admitted_at_ms,
    }
}

fn insert_candidate(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
    record: &GroupAgentScheduledNodeContractRecord,
) -> Result<(), HubStoreError> {
    let candidate = &request.candidate;
    let values = insert_candidate_values(record, candidate)?;
    transaction
        .execute(
            INSERT_CANDIDATE_SQL,
            params![
                record.contract_id,
                record.graph_run_id,
                candidate.graph_id,
                record.schedule_id,
                i64::from(record.v),
                i64::from(candidate.scheduler_protocol_version),
                i64::from(candidate.node_execution_protocol_version),
                i64::from(candidate.execution_schedule_protocol_version),
                scope_label(candidate.contract_scope),
                values.control.as_slice(),
                values.schedule.as_slice(),
                values.expected_last_event_seq,
                values.head.as_slice(),
                values.execution_ordinal,
                record.node_id,
                values.authored_node_index,
                values.topology_wave_index,
                i64::from(record.attempt),
                values.lane.as_slice(),
                record.request_id,
                values.logical_request.as_slice(),
                values.required_predecessor_count,
                values.predecessor_receipt_count,
                i64::from(record.lifecycle_contract_admitted),
                i64::from(record.provider_request_present),
                i64::from(record.execution_authority_released),
                i64::from(record.dispatch_authority_released),
                i64::from(record.progress_observed),
                i64::from(record.successor_advance_authorized),
                request.candidate_json.as_bytes(),
                values.contract_bytes,
                values.contract.as_slice(),
                request.idempotency_key,
                values.created_at_ms,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentScheduledNodeContract, error))?;
    Ok(())
}

fn insert_candidate_values(
    record: &GroupAgentScheduledNodeContractRecord,
    candidate: &crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
) -> Result<InsertCandidateValues, HubStoreError> {
    Ok(InsertCandidateValues {
        control: candidate_digest(&record.control_snapshot_sha256, "control snapshot")?,
        schedule: candidate_digest(&record.schedule_sha256, "schedule")?,
        contract: candidate_digest(&record.contract_sha256, "contract")?,
        logical_request: candidate_digest(&record.request_sha256, "request")?,
        lane: candidate_digest(&record.project_lane_sha256, "project lane")?,
        head: candidate_digest(&record.expected_last_event_sha256, "expected event")?,
        expected_last_event_seq: to_i64(record.expected_last_event_seq, "expected event sequence")?,
        execution_ordinal: to_i64(record.execution_ordinal, "execution ordinal")?,
        authored_node_index: to_i64(candidate.node.authored_node_index, "authored node index")?,
        topology_wave_index: to_i64(candidate.node.topology_wave_index, "topology wave index")?,
        required_predecessor_count: to_i64(
            candidate.request.required_predecessor_node_ids.len(),
            "required predecessor count",
        )?,
        predecessor_receipt_count: to_i64(
            record.predecessor_receipt_count,
            "predecessor receipt count",
        )?,
        contract_bytes: to_i64(record.contract_bytes, "contract byte count")?,
        created_at_ms: to_i64(record.created_at_ms, "creation time")?,
    })
}

fn ensure_replay(
    inspection: &GroupAgentScheduledNodeContractInspection,
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
) -> Result<(), HubStoreError> {
    let exact = inspection.record.graph_run_id == request.graph_run_id
        && inspection.candidate == request.candidate
        && inspection.candidate_json.as_bytes() == request.candidate_json.as_bytes();
    exact.then_some(()).ok_or_else(|| {
        conflict("idempotency key was reused with different scheduled contract input")
    })
}

fn ensure_committed(
    inspection: &GroupAgentScheduledNodeContractInspection,
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
    record: &GroupAgentScheduledNodeContractRecord,
) -> Result<(), HubStoreError> {
    let exact = inspection.v == GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION
        && inspection.record == *record
        && inspection.candidate == request.candidate
        && inspection.candidate_json.as_bytes() == request.candidate_json.as_bytes();
    exact.then_some(()).ok_or_else(|| {
        corrupt("persisted scheduled-node contract disagrees with its committed candidate")
    })
}

fn result(
    disposition: AdmitGroupAgentScheduledNodeContractDisposition,
    inspection: GroupAgentScheduledNodeContractInspection,
) -> AdmitGroupAgentScheduledNodeContractResult {
    AdmitGroupAgentScheduledNodeContractResult {
        v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
        disposition,
        inspection,
    }
}

fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    group_run_codec::decode_hex_digest(value).ok_or_else(|| {
        conflict(&format!(
            "scheduled-node contract {subject} digest is invalid"
        ))
    })
}

fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| {
        conflict(&format!(
            "invalid scheduled-node contract {subject}: {error}"
        ))
    })
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentScheduledNodeContract,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn scope_label(scope: GroupAgentScheduledNodeContractScope) -> &'static str {
    match scope {
        GroupAgentScheduledNodeContractScope::ScheduleInitialNodeOnly => {
            "schedule_initial_node_only"
        }
        GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly => "schedule_successor_only",
    }
}
