use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::{
    AdmitGroupAgentGraphExecutionSchedule, AdmitGroupAgentGraphExecutionScheduleDisposition,
    AdmitGroupAgentGraphExecutionScheduleResult, GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
    GROUP_AGENT_GRAPH_RUN_VERSION, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphExecutionScheduleRecord, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, HubEntity, HubStoreError,
};

use super::super::super::{
    group_agent_graph, group_agent_node_execution_contract, group_run_codec, read_error,
    write_error,
};
use super::super::read as graph_run_read;
use super::{read, rows};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn admit(
    connection: &mut Connection,
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError> {
    admit_with_before_reread(connection, request, || Ok(()))
}

pub(super) fn admit_with_before_reread<F>(
    connection: &mut Connection,
    request: &AdmitGroupAgentGraphExecutionSchedule,
    before_reread: F,
) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError>
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
    request: &AdmitGroupAgentGraphExecutionSchedule,
    before_reread: F,
) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let decoded = read::decode_stored(stored)?;
        let (run, graph) = load_source(transaction, &decoded.record.graph_run_id)?;
        let inspection = read::validate_with_sources(decoded, &run, &graph)?;
        validate_candidate(request, &run, &graph)?;
        ensure_replay(&inspection, request)?;
        return Ok(result(
            AdmitGroupAgentGraphExecutionScheduleDisposition::Replayed,
            inspection,
        ));
    }
    reject_existing_identity(transaction, request)?;
    let (run, graph) = load_source(transaction, &request.graph_run_id)?;
    validate_candidate(request, &run, &graph)?;
    create(transaction, request, &run, &graph, before_reread)
}

fn reject_existing_identity(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> Result<(), HubStoreError> {
    if let Some(stored) = rows::find_by_id(transaction, &request.schedule.schedule_id)? {
        validate_existing(transaction, stored)?;
        return Err(conflict(
            "execution schedule ID already belongs to another idempotency key",
        ));
    }
    if let Some(stored) = rows::find_by_run(transaction, &request.graph_run_id)? {
        validate_existing(transaction, stored)?;
        return Err(conflict(
            "Graph Run already has an execution schedule under another idempotency key",
        ));
    }
    Ok(())
}

fn validate_existing(
    transaction: &Transaction<'_>,
    stored: rows::RawStoredSchedule,
) -> Result<(), HubStoreError> {
    let decoded = read::decode_stored(stored)?;
    let (run, graph) = load_source(transaction, &decoded.record.graph_run_id)?;
    read::validate_with_sources(decoded, &run, &graph).map(|_| ())
}

fn load_source(
    transaction: &Transaction<'_>,
    graph_run_id: &str,
) -> Result<(GroupAgentGraphRunInspection, GroupAgentGraphInspection), HubStoreError> {
    let run = graph_run_read::inspect_in_snapshot(transaction, graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(transaction, &run.run.graph_id)?;
    Ok((run, graph))
}

fn validate_candidate(
    request: &AdmitGroupAgentGraphExecutionSchedule,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(), HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))?;
    let (control, control_json) =
        group_agent_node_execution_contract::snapshot::reconstruct(run, graph)?;
    let exact = request.control_snapshot == control
        && request.control_snapshot_json.as_bytes() == control_json.as_bytes();
    if !exact {
        return Err(conflict(
            "execution schedule control does not exactly match the stored Graph Run",
        ));
    }
    if !is_pristine_schedule_source(run) {
        return Err(conflict(
            "execution schedules can only be admitted at the v1 Graph Run head",
        ));
    }
    Ok(())
}

fn is_pristine_schedule_source(run: &GroupAgentGraphRunInspection) -> bool {
    run.run.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && run.run.last_event_seq == 1
        && run.events.len() == 1
        && !run.run.execution_contract_present
        && !run.run.dispatch_request_present
        && !run.run.dispatch_authority_released
}

fn create<F>(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentGraphExecutionSchedule,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    before_reread: F,
) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let record = record(request);
    insert_schedule(transaction, request, &record)?;
    before_reread()?;
    let stored = rows::find_by_id(transaction, &record.schedule_id)?
        .ok_or_else(|| corrupt("persisted execution schedule disappeared"))?;
    let inspection = read::validate_with_sources(read::decode_stored(stored)?, run, graph)?;
    ensure_committed(&inspection, request, &record)?;
    Ok(result(
        AdmitGroupAgentGraphExecutionScheduleDisposition::Created,
        inspection,
    ))
}

fn record(
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> GroupAgentGraphExecutionScheduleRecord {
    let schedule = &request.schedule;
    GroupAgentGraphExecutionScheduleRecord {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        schedule_id: schedule.schedule_id.clone(),
        graph_run_id: schedule.graph_run_id.clone(),
        graph_id: schedule.graph_id.clone(),
        control_snapshot_sha256: schedule.control_snapshot_sha256.clone(),
        schedule_sha256: schedule.schedule_sha256.clone(),
        schedule_bytes: request.schedule_json.len(),
        node_count: schedule.node_count,
        wave_count: schedule.wave_count,
        expected_last_event_seq: schedule.expected_last_event_seq,
        expected_last_event_sha256: schedule.expected_last_event_sha256.clone(),
        execution_contract_present: schedule.execution_contract_present,
        dispatch_authority_released: schedule.dispatch_authority_released,
        created_at_ms: request.admitted_at_ms,
    }
}

fn insert_schedule(
    transaction: &Transaction<'_>,
    request: &AdmitGroupAgentGraphExecutionSchedule,
    record: &GroupAgentGraphExecutionScheduleRecord,
) -> Result<(), HubStoreError> {
    let control = candidate_digest(&record.control_snapshot_sha256, "control snapshot")?;
    let head = candidate_digest(&record.expected_last_event_sha256, "expected event")?;
    let schedule_digest = candidate_digest(&record.schedule_sha256, "schedule")?;
    let schedule = &request.schedule;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_execution_schedules(
               id,graph_run_id,graph_id,schedule_version,scheduler_protocol_version,
               execution_schedule_protocol_version,control_snapshot_sha256,
               expected_last_event_seq,expected_last_event_sha256,initial_node,
               node_count,wave_count,execution_contract_present,dispatch_authority_released,
               progress_observed,successor_advanced,schedule_blob,schedule_bytes,
               schedule_sha256,idempotency_key,created_at_ms
             ) VALUES(
               ?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,
               ?17,?18,?19,?20,?21
             )",
            params![
                record.schedule_id,
                record.graph_run_id,
                record.graph_id,
                i64::from(record.v),
                i64::from(schedule.scheduler_protocol_version),
                i64::from(schedule.execution_schedule_protocol_version),
                control.as_slice(),
                to_i64(record.expected_last_event_seq, "expected event sequence")?,
                head.as_slice(),
                schedule.initial_node,
                to_i64(record.node_count, "node count")?,
                to_i64(record.wave_count, "wave count")?,
                i64::from(record.execution_contract_present),
                i64::from(record.dispatch_authority_released),
                i64::from(schedule.progress_observed),
                i64::from(schedule.successor_advanced),
                request.schedule_json.as_bytes(),
                to_i64(record.schedule_bytes, "schedule byte count")?,
                schedule_digest.as_slice(),
                request.idempotency_key,
                to_i64(record.created_at_ms, "creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentGraphExecutionSchedule, error))?;
    Ok(())
}

fn ensure_replay(
    inspection: &GroupAgentGraphExecutionScheduleInspection,
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> Result<(), HubStoreError> {
    let exact = inspection.record.graph_run_id == request.graph_run_id
        && inspection.schedule == request.schedule
        && inspection.schedule_json.as_bytes() == request.schedule_json.as_bytes();
    exact
        .then_some(())
        .ok_or_else(|| conflict("idempotency key was reused with different schedule input"))
}

fn ensure_committed(
    inspection: &GroupAgentGraphExecutionScheduleInspection,
    request: &AdmitGroupAgentGraphExecutionSchedule,
    record: &GroupAgentGraphExecutionScheduleRecord,
) -> Result<(), HubStoreError> {
    let exact = inspection.v == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION
        && inspection.record == *record
        && inspection.schedule == request.schedule
        && inspection.schedule_json.as_bytes() == request.schedule_json.as_bytes();
    exact.then_some(()).ok_or_else(|| {
        corrupt("persisted execution schedule disagrees with its committed candidate")
    })
}

fn result(
    disposition: AdmitGroupAgentGraphExecutionScheduleDisposition,
    inspection: GroupAgentGraphExecutionScheduleInspection,
) -> AdmitGroupAgentGraphExecutionScheduleResult {
    AdmitGroupAgentGraphExecutionScheduleResult {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        disposition,
        inspection,
    }
}

fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    group_run_codec::decode_hex_digest(value)
        .ok_or_else(|| conflict(&format!("execution schedule {subject} digest is invalid")))
}

fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value)
        .map_err(|error| conflict(&format!("invalid execution schedule {subject}: {error}")))
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraphExecutionSchedule,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
