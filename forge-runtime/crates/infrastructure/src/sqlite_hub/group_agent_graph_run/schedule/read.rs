use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleRecord,
    GroupAgentGraphInspection, GroupAgentGraphRunInspection, HubEntity, HubStoreError,
    MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_LIST_LIMIT,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
};

use super::super::super::{
    group_agent_graph, group_agent_node_execution_contract, group_run_codec, read_error,
};
use super::{
    super::{codec as graph_run_codec, read as graph_run_read},
    rows,
};

pub(super) fn inspect(
    connection: &mut Connection,
    schedule_id: &str,
) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, schedule_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

fn inspect_in_snapshot(
    connection: &Connection,
    schedule_id: &str,
) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
    let stored =
        rows::find_by_id(connection, schedule_id)?.ok_or_else(|| not_found(schedule_id))?;
    validate_stored(connection, stored)
}

pub(super) fn list(
    connection: &mut Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAgentGraphExecutionScheduleRecord>, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    validate_list_request(&transaction, graph_run_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    let records = rows::query_metadata(&transaction, graph_run_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect::<Result<_, _>>()?;
    transaction.commit().map_err(read_error)?;
    Ok(records)
}

fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredSchedule,
) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
    let decoded = decode_stored(stored)?;
    let run = load_graph_run(connection, &decoded.record.graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(connection, &run.run.graph_id)?;
    validate_with_sources(decoded, &run, &graph)
}

pub(in crate::sqlite_hub) fn validate_graph_run_binding(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<Option<GroupAgentGraphExecutionScheduleInspection>, HubStoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(read_error)?;
    if version < 13 {
        return Ok(None);
    }
    let Some(stored) = rows::find_by_run(connection, &run.run.graph_run_id)? else {
        return Ok(None);
    };
    validate_with_sources(decode_stored(stored)?, run, graph).map(Some)
}

pub(in crate::sqlite_hub) fn has_graph_run_child(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<bool, HubStoreError> {
    rows::find_by_run(connection, graph_run_id).map(|stored| stored.is_some())
}

pub(super) fn validate_with_sources(
    decoded: DecodedStoredSchedule,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
    let (control, _) = group_agent_node_execution_contract::snapshot::reconstruct(run, graph)?;
    decoded
        .inspection
        .schedule
        .validate_against_control(&control)
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(decoded.inspection)
}

pub(super) struct DecodedStoredSchedule {
    pub(super) inspection: GroupAgentGraphExecutionScheduleInspection,
    pub(super) record: GroupAgentGraphExecutionScheduleRecord,
}

pub(super) fn decode_stored(
    stored: rows::RawStoredSchedule,
) -> Result<DecodedStoredSchedule, HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata.clone())?;
    if stored.schedule_blob.len() != record.schedule_bytes {
        return Err(corrupt("stored execution schedule byte count disagrees"));
    }
    let schedule = GroupAgentGraphExecutionSchedule::decode_exact_bytes(&stored.schedule_blob)
        .map_err(|error| corrupt(&error.to_string()))?;
    validate_extra_columns(&stored, &schedule)?;
    let schedule_json = String::from_utf8(stored.schedule_blob)
        .map_err(|error| corrupt(&format!("stored execution schedule is not UTF-8: {error}")))?;
    let inspection = GroupAgentGraphExecutionScheduleInspection {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        record: record.clone(),
        schedule_json,
        schedule,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(DecodedStoredSchedule { inspection, record })
}

fn validate_extra_columns(
    stored: &rows::RawStoredSchedule,
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), HubStoreError> {
    let exact = stored.scheduler_protocol_version == i64::from(schedule.scheduler_protocol_version)
        && stored.execution_schedule_protocol_version
            == i64::from(schedule.execution_schedule_protocol_version)
        && stored.initial_node == schedule.initial_node
        && parse_boolean(stored.progress_observed, "progress observation")?
            == schedule.progress_observed
        && parse_boolean(stored.successor_advanced, "successor advancement")?
            == schedule.successor_advanced;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("stored execution schedule columns disagree with its artifact"))
}

fn metadata_record(
    raw: rows::RawScheduleMetadata,
) -> Result<GroupAgentGraphExecutionScheduleRecord, HubStoreError> {
    let record = GroupAgentGraphExecutionScheduleRecord {
        v: convert(raw.schedule_version, "schedule version")?,
        schedule_id: raw.id,
        graph_run_id: raw.graph_run_id,
        graph_id: raw.graph_id,
        control_snapshot_sha256: graph_run_codec::digest_hex(
            &raw.control_snapshot_sha256,
            "schedule control snapshot",
        )?,
        schedule_sha256: graph_run_codec::digest_hex(&raw.schedule_sha256, "schedule")?,
        schedule_bytes: convert(raw.schedule_bytes, "schedule byte count")?,
        node_count: convert(raw.node_count, "schedule node count")?,
        wave_count: convert(raw.wave_count, "schedule wave count")?,
        expected_last_event_seq: convert(raw.expected_last_event_seq, "expected event sequence")?,
        expected_last_event_sha256: graph_run_codec::digest_hex(
            &raw.expected_last_event_sha256,
            "schedule expected event",
        )?,
        execution_contract_present: parse_boolean(
            raw.execution_contract_present,
            "execution contract presence",
        )?,
        dispatch_authority_released: parse_boolean(
            raw.dispatch_authority_released,
            "dispatch authority",
        )?,
        created_at_ms: convert(raw.created_at_ms, "schedule creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

fn load_graph_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
    graph_run_read::inspect_in_snapshot(connection, graph_run_id).map_err(|error| match error {
        HubStoreError::NotFound { .. } => {
            corrupt("stored execution schedule references a missing Graph Run")
        }
        other => other,
    })
}

fn validate_list_request(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_LIST_LIMIT).contains(&limit) {
        return Err(conflict(
            "execution schedule list limit is outside its bounds",
        ));
    }
    let Some(id) = graph_run_id else {
        return Ok(());
    };
    if !group_run_codec::valid_text(id, 128) {
        return Err(conflict("Graph Run filter is outside its bounds"));
    }
    connection
        .query_row(
            "SELECT 1 FROM group_agent_graph_runs WHERE id=?1",
            [id],
            |_| Ok(()),
        )
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| HubStoreError::NotFound {
            entity: HubEntity::GroupAgentGraphRun,
            id: id.into(),
        })
}

fn validate_stored_key(key: &str) -> Result<(), HubStoreError> {
    if group_run_codec::valid_text(key, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(corrupt(
            "stored execution schedule idempotency key is invalid",
        ))
    }
}

fn parse_boolean(value: i64, subject: &str) -> Result<bool, HubStoreError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(corrupt(&format!(
            "stored execution schedule {subject} is invalid"
        ))),
    }
}

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| corrupt(&format!("invalid stored {subject}: {error}")))
}

fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentGraphExecutionSchedule,
        id: id.into(),
    }
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
