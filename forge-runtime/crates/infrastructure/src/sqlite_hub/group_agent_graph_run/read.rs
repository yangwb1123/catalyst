use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use super::{
    super::{group_agent_graph, group_run_codec, read_error},
    BeginGroupAgentGraphRun, GroupAgentGraphCorePlan, GroupAgentGraphInspection,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, GroupAgentGraphRunStatus, HubEntity, HubStoreError,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_RUN_LIST_LIMIT, codec, rows,
};

pub(super) fn inspect(
    connection: &mut Connection,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
    inspect_transaction_with_hook(connection, graph_run_id, || Ok(()))
}

fn inspect_transaction_with_hook<F>(
    connection: &mut Connection,
    graph_run_id: &str,
    after_run: F,
) -> Result<GroupAgentGraphRunInspection, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot_with_hook(&transaction, graph_run_id, after_run)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(in crate::sqlite_hub) fn inspect_in_snapshot(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
    inspect_in_snapshot_with_hook(connection, graph_run_id, || Ok(()))
}

fn inspect_in_snapshot_with_hook<F>(
    connection: &Connection,
    graph_run_id: &str,
    after_run: F,
) -> Result<GroupAgentGraphRunInspection, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    let stored = rows::find_by_id(connection, graph_run_id)?
        .ok_or_else(|| not_found(HubEntity::GroupAgentGraphRun, graph_run_id))?;
    after_run()?;
    validate_stored(connection, stored)
}

#[cfg(test)]
pub(super) fn inspect_after_run<F>(
    connection: &mut Connection,
    graph_run_id: &str,
    after_run: F,
) -> Result<GroupAgentGraphRunInspection, HubStoreError>
where
    F: FnOnce() -> Result<(), HubStoreError>,
{
    inspect_transaction_with_hook(connection, graph_run_id, after_run)
}

pub(super) fn list(
    connection: &Connection,
    graph_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError> {
    validate_list_request(connection, graph_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    rows::query_metadata(connection, graph_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect()
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredRun,
) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata)?;
    if stored.plan_blob.len() != record.plan_bytes {
        return Err(corrupt("stored Core Plan byte count disagrees"));
    }
    let (plan, plan_json) = codec::decode_plan(&stored.plan_blob, &record_digest(&record)?)?;
    let (events, event_jsons) = load_events(connection, &record)?;
    validate_contract_presence(connection, &record)?;
    let graph = load_graph_stored(connection, &record.graph_id)?;
    validate_graph_binding(&graph, &record, &plan)?;
    let inspection = GroupAgentGraphRunInspection {
        v: record.v,
        run: record,
        plan_json,
        plan,
        event_jsons,
        events,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

fn validate_contract_presence(
    connection: &Connection,
    record: &GroupAgentGraphRunRecord,
) -> Result<(), HubStoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT count(*) FROM group_agent_graph_node_execution_contracts
             WHERE graph_run_id = ?1",
            [&record.graph_run_id],
            |row| row.get(0),
        )
        .map_err(read_error)?;
    let expected = i64::from(record.execution_contract_present);
    if count == expected {
        Ok(())
    } else {
        Err(corrupt(
            "stored Graph Run execution-contract presence disagrees",
        ))
    }
}

pub(super) fn validate_candidate_graph(
    connection: &Connection,
    request: &BeginGroupAgentGraphRun,
) -> Result<(), HubStoreError> {
    let graph = group_agent_graph::read::inspect_in_snapshot(connection, &request.graph_id)
        .map_err(candidate_graph_error)?;
    let authored_node_ids = graph
        .manifest
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let exact = request.source_snapshot_sha256 == graph.graph.source_snapshot_sha256
        && request.graph_manifest_sha256 == graph.graph.manifest_sha256
        && request.plan.graph_version == graph.graph.v
        && request.plan.graph_id == graph.graph.graph_id
        && request.plan.authored_node_ids == authored_node_ids
        && request.plan.edges == graph.manifest.edges
        && request.plan.waves == graph.manifest.waves;
    exact
        .then_some(())
        .ok_or_else(|| conflict("Graph Run plan does not exactly bind its frozen graph"))
}

fn load_events(
    connection: &Connection,
    record: &GroupAgentGraphRunRecord,
) -> Result<(Vec<GroupAgentGraphRunEvent>, Vec<String>), HubStoreError> {
    let raw_events = rows::load_events(connection, &record.graph_run_id)?;
    let expected_count: usize = u64_to_usize(record.last_event_seq, "last event sequence")?;
    if raw_events.len() != expected_count {
        return Err(corrupt("stored Graph Run event count disagrees"));
    }
    let mut events = Vec::with_capacity(raw_events.len());
    let mut jsons = Vec::with_capacity(raw_events.len());
    let mut journal_bytes = 0_usize;
    for (position, raw) in raw_events.iter().enumerate() {
        let (event, json) = decode_event_row(record, raw, position + 1)?;
        journal_bytes = journal_bytes
            .checked_add(json.len())
            .ok_or_else(|| corrupt("stored Graph Run journal byte count overflowed"))?;
        events.push(event);
        jsons.push(json);
    }
    if journal_bytes != record.journal_bytes {
        return Err(corrupt("stored Graph Run journal byte count disagrees"));
    }
    Ok((events, jsons))
}

fn decode_event_row(
    record: &GroupAgentGraphRunRecord,
    raw: &rows::RawEvent,
    expected_sequence: usize,
) -> Result<(GroupAgentGraphRunEvent, String), HubStoreError> {
    let event_bytes: usize = convert(raw.event_bytes, "event byte count")?;
    let (event, json) = codec::decode_event(&raw.event_blob, &raw.event_sha256)?;
    let valid = raw.graph_run_id == record.graph_run_id
        && raw.sequence == i64::try_from(expected_sequence).unwrap_or(-1)
        && raw.event_version == i64::from(event.v)
        && raw.kind == event_kind(&event.kind)
        && event_bytes == raw.event_blob.len()
        && raw.created_at_ms == i64::try_from(event_time(&event.kind)).unwrap_or(-1)
        && event.graph_run_id == record.graph_run_id
        && event.seq == u64::try_from(expected_sequence).unwrap_or(0);
    valid
        .then_some((event, json))
        .ok_or_else(|| corrupt("stored Graph Run event row binding is invalid"))
}

fn event_kind(kind: &GroupAgentGraphRunEventKind) -> &'static str {
    match kind {
        GroupAgentGraphRunEventKind::GraphRunPrepared { .. } => "graph_run_prepared",
        GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted { .. } => {
            "node_execution_contract_admitted"
        }
    }
}

fn event_time(kind: &GroupAgentGraphRunEventKind) -> u64 {
    match kind {
        GroupAgentGraphRunEventKind::GraphRunPrepared { prepared_at_ms, .. } => *prepared_at_ms,
        GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted { admitted_at_ms, .. } => {
            *admitted_at_ms
        }
    }
}

fn load_graph_stored(
    connection: &Connection,
    graph_id: &str,
) -> Result<GroupAgentGraphInspection, HubStoreError> {
    group_agent_graph::read::inspect_in_snapshot(connection, graph_id).map_err(
        |error| match error {
            HubStoreError::NotFound { .. } => {
                corrupt("stored Graph Run references a missing frozen graph")
            }
            other => other,
        },
    )
}

fn validate_graph_binding(
    graph: &GroupAgentGraphInspection,
    record: &GroupAgentGraphRunRecord,
    plan: &GroupAgentGraphCorePlan,
) -> Result<(), HubStoreError> {
    let authored_node_ids = graph
        .manifest
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    let exact = record.graph_id == graph.graph.graph_id
        && record.source_snapshot_sha256 == graph.graph.source_snapshot_sha256
        && record.graph_manifest_sha256 == graph.graph.manifest_sha256
        && plan.graph_version == graph.graph.v
        && plan.authored_node_ids == authored_node_ids
        && plan.edges == graph.manifest.edges
        && plan.waves == graph.manifest.waves;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("stored Graph Run disagrees with its frozen graph"))
}

fn metadata_record(raw: rows::RawRunMetadata) -> Result<GroupAgentGraphRunRecord, HubStoreError> {
    let record = GroupAgentGraphRunRecord {
        v: convert(raw.run_version, "run version")?,
        graph_run_id: raw.id,
        graph_id: raw.graph_id,
        status: parse_status(&raw.status)?,
        source_snapshot_sha256: codec::digest_hex(&raw.source_snapshot_sha256, "source")?,
        graph_manifest_sha256: codec::digest_hex(&raw.graph_manifest_sha256, "graph manifest")?,
        scheduler_protocol_version: convert(raw.scheduler_protocol_version, "scheduler protocol")?,
        plan_sha256: codec::digest_hex(&raw.plan_sha256, "plan")?,
        plan_bytes: convert(raw.plan_bytes, "plan byte count")?,
        node_count: convert(raw.node_count, "node count")?,
        wave_count: convert(raw.wave_count, "wave count")?,
        execution_contract_present: parse_boolean(
            raw.execution_contract_present,
            "execution contract presence",
        )?,
        dispatch_authority_released: parse_boolean(
            raw.dispatch_authority_released,
            "dispatch authority",
        )?,
        last_event_seq: convert(raw.last_event_seq, "last event sequence")?,
        journal_bytes: convert(raw.journal_bytes, "journal byte count")?,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

fn record_digest(record: &GroupAgentGraphRunRecord) -> Result<[u8; 32], HubStoreError> {
    codec::candidate_digest(&record.plan_sha256, "plan")
        .map_err(|error| corrupt(&error.to_string()))
}

fn validate_stored_key(key: &str) -> Result<(), HubStoreError> {
    if group_run_codec::valid_text(key, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(corrupt("stored Graph Run idempotency key is invalid"))
    }
}

fn validate_list_request(
    connection: &Connection,
    graph_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_AGENT_GRAPH_RUN_LIST_LIMIT).contains(&limit) {
        return Err(conflict("Graph Run list limit is outside its bounds"));
    }
    let Some(id) = graph_id else {
        return Ok(());
    };
    if !group_run_codec::valid_text(id, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES) {
        return Err(conflict("graph filter is outside its bounds"));
    }
    connection
        .query_row(
            "SELECT 1 FROM group_agent_graphs WHERE id = ?1",
            [id],
            |_| Ok(()),
        )
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::GroupAgentGraph, id))
}

fn parse_status(value: &str) -> Result<GroupAgentGraphRunStatus, HubStoreError> {
    match value {
        "awaiting_execution_contract" => Ok(GroupAgentGraphRunStatus::AwaitingExecutionContract),
        "awaiting_core_dispatch" => Ok(GroupAgentGraphRunStatus::AwaitingCoreDispatch),
        _ => Err(corrupt(
            "stored Group Agent Graph Run status is unsupported",
        )),
    }
}

fn parse_boolean(value: i64, subject: &str) -> Result<bool, HubStoreError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(corrupt(&format!(
            "stored Group Agent Graph Run {subject} is not Boolean"
        ))),
    }
}

fn candidate_graph_error(error: HubStoreError) -> HubStoreError {
    match error {
        HubStoreError::NotFound { .. } => conflict("Graph Run references a missing frozen graph"),
        other => other,
    }
}

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| corrupt(&format!("invalid Graph Run {subject}: {error}")))
}

fn u64_to_usize(value: u64, subject: &str) -> Result<usize, HubStoreError> {
    usize::try_from(value)
        .map_err(|error| corrupt(&format!("invalid Graph Run {subject}: {error}")))
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
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
