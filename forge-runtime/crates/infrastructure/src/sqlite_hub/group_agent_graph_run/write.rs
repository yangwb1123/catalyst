use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use super::{
    super::{read_error, write_error},
    BeginGroupAgentGraphRun, BeginGroupAgentGraphRunDisposition, BeginGroupAgentGraphRunResult,
    GROUP_AGENT_GRAPH_RUN_VERSION, GroupAgentGraphRunInspection, GroupAgentGraphRunRecord,
    GroupAgentGraphRunStatus, HubEntity, HubStoreError, codec, read, rows,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn begin(
    connection: &mut Connection,
    request: &BeginGroupAgentGraphRun,
) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = begin_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn begin_locked(
    transaction: &Transaction<'_>,
    request: &BeginGroupAgentGraphRun,
) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let inspection = read::validate_stored(transaction, stored)?;
        validate_request(request)?;
        ensure_replay(&inspection, request)?;
        return Ok(result(
            BeginGroupAgentGraphRunDisposition::Replayed,
            inspection,
        ));
    }
    if let Some(stored) = rows::find_by_id(transaction, &request.graph_run_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "Graph Run ID already belongs to another idempotency key",
        ));
    }
    validate_request(request)?;
    read::validate_candidate_graph(transaction, request)?;
    create(transaction, request)
}

fn create(
    transaction: &Transaction<'_>,
    request: &BeginGroupAgentGraphRun,
) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
    let encoded = codec::encode_candidate(request)?;
    let record = record(request, &encoded);
    insert_run(transaction, request, &record, &encoded)?;
    insert_event(transaction, request, &encoded)?;
    let inspection = read::inspect_in_snapshot(transaction, &record.graph_run_id)?;
    let expected = expected_inspection(request, record);
    if inspection != expected {
        return Err(corrupt(
            "persisted Graph Run disagrees with its committed candidate",
        ));
    }
    Ok(result(
        BeginGroupAgentGraphRunDisposition::Created,
        inspection,
    ))
}

fn record(
    request: &BeginGroupAgentGraphRun,
    encoded: &codec::EncodedCandidate,
) -> GroupAgentGraphRunRecord {
    GroupAgentGraphRunRecord {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: request.graph_run_id.clone(),
        graph_id: request.graph_id.clone(),
        status: GroupAgentGraphRunStatus::AwaitingExecutionContract,
        source_snapshot_sha256: request.source_snapshot_sha256.clone(),
        graph_manifest_sha256: request.graph_manifest_sha256.clone(),
        scheduler_protocol_version: request.plan.scheduler_protocol_version,
        plan_sha256: request.plan.plan_sha256.clone(),
        plan_bytes: encoded.plan_bytes.len(),
        node_count: request.plan.authored_node_ids.len(),
        wave_count: request.plan.waves.len(),
        execution_contract_present: false,
        dispatch_request_present: false,
        dispatch_authority_released: false,
        last_event_seq: 1,
        journal_bytes: encoded.event_bytes.len(),
        created_at_ms: request.created_at_ms,
    }
}

fn insert_run(
    transaction: &Transaction<'_>,
    request: &BeginGroupAgentGraphRun,
    record: &GroupAgentGraphRunRecord,
    encoded: &codec::EncodedCandidate,
) -> Result<(), HubStoreError> {
    let source = codec::candidate_digest(&record.source_snapshot_sha256, "source")?;
    let manifest = codec::candidate_digest(&record.graph_manifest_sha256, "graph manifest")?;
    transaction
        .execute(
            "INSERT INTO group_agent_graph_runs(
               id,graph_id,run_version,status,source_snapshot_sha256,
               graph_manifest_sha256,scheduler_protocol_version,plan_blob,plan_bytes,
               plan_sha256,node_count,wave_count,execution_contract_present,
               dispatch_request_present,dispatch_authority_released,last_event_seq,journal_bytes,
               idempotency_key,created_at_ms
             ) VALUES(
               ?1,?2,?3,'awaiting_execution_contract',?4,?5,?6,?7,?8,?9,?10,?11,
               0,0,0,1,?12,?13,?14
             )",
            params![
                record.graph_run_id,
                record.graph_id,
                i64::from(record.v),
                source.as_slice(),
                manifest.as_slice(),
                i64::from(record.scheduler_protocol_version),
                encoded.plan_bytes.as_slice(),
                to_i64(record.plan_bytes, "plan byte count")?,
                encoded.plan_digest.as_slice(),
                to_i64(record.node_count, "node count")?,
                to_i64(record.wave_count, "wave count")?,
                to_i64(record.journal_bytes, "journal byte count")?,
                request.idempotency_key,
                to_i64(record.created_at_ms, "creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentGraphRun, error))?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    request: &BeginGroupAgentGraphRun,
    encoded: &codec::EncodedCandidate,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,
               event_sha256,created_at_ms
             ) VALUES(?1,1,?2,'graph_run_prepared',?3,?4,?5,?6)",
            params![
                request.graph_run_id,
                i64::from(request.event.v),
                encoded.event_bytes.as_slice(),
                to_i64(encoded.event_bytes.len(), "event byte count")?,
                encoded.event_digest.as_slice(),
                to_i64(request.created_at_ms, "event creation time")?,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupAgentGraphRun, error))?;
    Ok(())
}

fn ensure_replay(
    inspection: &GroupAgentGraphRunInspection,
    request: &BeginGroupAgentGraphRun,
) -> Result<(), HubStoreError> {
    let exact = inspection.run.graph_id == request.graph_id
        && inspection.run.source_snapshot_sha256 == request.source_snapshot_sha256
        && inspection.run.graph_manifest_sha256 == request.graph_manifest_sha256
        && inspection.plan == request.plan
        && inspection.plan_json == request.plan_json;
    exact
        .then_some(())
        .ok_or_else(|| conflict("idempotency key was reused with different Graph Run input"))
}

fn expected_inspection(
    request: &BeginGroupAgentGraphRun,
    run: GroupAgentGraphRunRecord,
) -> GroupAgentGraphRunInspection {
    GroupAgentGraphRunInspection {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        run,
        plan_json: request.plan_json.clone(),
        plan: request.plan.clone(),
        event_jsons: vec![request.event_json.clone()],
        events: vec![request.event.clone()],
    }
}

fn result(
    disposition: BeginGroupAgentGraphRunDisposition,
    inspection: GroupAgentGraphRunInspection,
) -> BeginGroupAgentGraphRunResult {
    BeginGroupAgentGraphRunResult {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        disposition,
        inspection,
    }
}

fn validate_request(request: &BeginGroupAgentGraphRun) -> Result<(), HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.to_string()))
}

fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| conflict(&format!("invalid Graph Run {subject}: {error}")))
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraphRun,
        message: message.into(),
    }
}
