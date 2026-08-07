use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractRecord, HubEntity,
    HubStoreError, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_LIST_LIMIT,
};

use super::super::{
    group_agent_graph, group_agent_graph_run, group_agent_node_execution_contract, group_run_codec,
    read_error,
};
use super::rows;

pub(super) fn inspect(
    connection: &mut Connection,
    contract_id: &str,
) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, contract_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(in crate::sqlite_hub) fn inspect_in_snapshot(
    connection: &Connection,
    contract_id: &str,
) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
    let stored =
        rows::find_by_id(connection, contract_id)?.ok_or_else(|| not_found(contract_id))?;
    validate_stored(connection, stored)
}

pub(super) fn list(
    connection: &mut Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAgentScheduledNodeContractRecord>, HubStoreError> {
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

pub(in crate::sqlite_hub) fn validate_existing_for_run(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<bool, HubStoreError> {
    let Some(stored) = rows::find_by_run(connection, graph_run_id)? else {
        return Ok(false);
    };
    validate_stored(connection, stored).map(|_| true)
}

pub(in crate::sqlite_hub) fn validate_graph_run_binding(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<Option<GroupAgentScheduledNodeContractInspection>, HubStoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(read_error)?;
    if version < 14 {
        return Ok(None);
    }
    let Some(stored) = rows::find_by_run(connection, &run.run.graph_run_id)? else {
        return Ok(None);
    };
    if !is_pristine_source(run) {
        decode_stored(stored)?;
        return Err(corrupt(
            "stored scheduled-node contract candidate conflicts with the Graph Run lifecycle",
        ));
    }
    let schedule = required_schedule(connection, run, graph)?;
    validate_with_sources(decode_stored(stored)?, run, graph, &schedule).map(Some)
}

pub(in crate::sqlite_hub) fn has_graph_run_child(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<bool, HubStoreError> {
    rows::exists_for_run(connection, graph_run_id)
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredCandidate,
) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
    let decoded = decode_stored(stored)?;
    let run = group_agent_graph_run::read::inspect_in_snapshot(
        connection,
        &decoded.inspection.record.graph_run_id,
    )
    .map_err(|error| parent_error(error, "Graph Run"))?;
    let graph = group_agent_graph::read::inspect_in_snapshot(connection, &run.run.graph_id)
        .map_err(|error| parent_error(error, "Graph"))?;
    let schedule = required_schedule(connection, &run, &graph)?;
    validate_with_sources(decoded, &run, &graph, &schedule)
}

fn required_schedule(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
    group_agent_graph_run::schedule::read::validate_graph_run_binding(connection, run, graph)?
        .ok_or_else(|| corrupt("stored scheduled-node contract candidate has no schedule"))
}

pub(in crate::sqlite_hub) struct DecodedStoredCandidate {
    pub(in crate::sqlite_hub) inspection: GroupAgentScheduledNodeContractInspection,
    extra: StoredExtra,
}

struct StoredExtra {
    graph_id: String,
    scheduler_protocol_version: i64,
    node_execution_protocol_version: i64,
    execution_schedule_protocol_version: i64,
    contract_scope: String,
    authored_node_index: i64,
    topology_wave_index: i64,
    required_predecessor_node_count: i64,
}

pub(in crate::sqlite_hub) fn decode_stored(
    stored: rows::RawStoredCandidate,
) -> Result<DecodedStoredCandidate, HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata)?;
    if stored.contract_blob.len() != record.contract_bytes {
        return Err(corrupt(
            "stored scheduled-node contract candidate byte count disagrees",
        ));
    }
    let candidate =
        GroupAgentScheduledNodeContractCandidate::decode_exact_bytes(&stored.contract_blob)
            .map_err(|error| corrupt(&error.to_string()))?;
    let candidate_json = String::from_utf8(stored.contract_blob).map_err(|error| {
        corrupt(&format!(
            "stored scheduled-node contract is not UTF-8: {error}"
        ))
    })?;
    let inspection = GroupAgentScheduledNodeContractInspection {
        v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
        record,
        candidate_json,
        candidate,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(DecodedStoredCandidate {
        inspection,
        extra: StoredExtra {
            graph_id: stored.graph_id,
            scheduler_protocol_version: stored.scheduler_protocol_version,
            node_execution_protocol_version: stored.node_execution_protocol_version,
            execution_schedule_protocol_version: stored.execution_schedule_protocol_version,
            contract_scope: stored.contract_scope,
            authored_node_index: stored.authored_node_index,
            topology_wave_index: stored.topology_wave_index,
            required_predecessor_node_count: stored.required_predecessor_node_count,
        },
    })
}

pub(super) fn validate_with_sources(
    decoded: DecodedStoredCandidate,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
    let candidate = &decoded.inspection.candidate;
    validate_extra_columns(&decoded.extra, candidate)?;
    let (control, _) = group_agent_node_execution_contract::snapshot::reconstruct(run, graph)?;
    candidate
        .validate_against_control_and_schedule(&control, &schedule.schedule)
        .map_err(|error| corrupt(&error.to_string()))?;
    if decoded.inspection.record.schedule_id != schedule.record.schedule_id
        || decoded.inspection.record.schedule_sha256 != schedule.record.schedule_sha256
    {
        return Err(corrupt(
            "stored scheduled-node contract and schedule identities disagree",
        ));
    }
    Ok(decoded.inspection)
}

fn validate_extra_columns(
    extra: &StoredExtra,
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), HubStoreError> {
    let exact = extra.graph_id == candidate.graph_id
        && extra.scheduler_protocol_version == i64::from(candidate.scheduler_protocol_version)
        && extra.node_execution_protocol_version
            == i64::from(candidate.node_execution_protocol_version)
        && extra.execution_schedule_protocol_version
            == i64::from(candidate.execution_schedule_protocol_version)
        && extra.contract_scope == "schedule_initial_node_only"
        && extra.authored_node_index
            == i64::try_from(candidate.node.authored_node_index).unwrap_or(-1)
        && extra.topology_wave_index
            == i64::try_from(candidate.node.topology_wave_index).unwrap_or(-1)
        && extra.required_predecessor_node_count
            == i64::try_from(candidate.request.required_predecessor_node_ids.len()).unwrap_or(-1);
    exact.then_some(()).ok_or_else(|| {
        corrupt("stored scheduled-node contract columns disagree with its candidate")
    })
}

fn metadata_record(
    raw: rows::RawCandidateMetadata,
) -> Result<GroupAgentScheduledNodeContractRecord, HubStoreError> {
    let [
        lifecycle_admitted,
        provider_present,
        execution_released,
        dispatch_released,
        progress,
        successor,
    ] = parse_candidate_flags(&raw)?;
    let record = GroupAgentScheduledNodeContractRecord {
        v: convert(raw.contract_version, "contract version")?,
        contract_id: raw.id,
        graph_run_id: raw.graph_run_id,
        schedule_id: raw.schedule_id,
        node_id: raw.node_id,
        execution_ordinal: convert(raw.execution_ordinal, "execution ordinal")?,
        attempt: convert(raw.attempt, "attempt")?,
        control_snapshot_sha256: digest_hex(&raw.control_snapshot_sha256, "control snapshot")?,
        schedule_sha256: digest_hex(&raw.schedule_sha256, "schedule")?,
        contract_sha256: digest_hex(&raw.contract_sha256, "contract")?,
        contract_bytes: convert(raw.contract_bytes, "contract byte count")?,
        request_id: raw.request_id,
        request_sha256: digest_hex(&raw.request_sha256, "request")?,
        project_lane_sha256: digest_hex(&raw.project_lane_sha256, "project lane")?,
        expected_last_event_seq: convert(raw.expected_last_event_seq, "expected event sequence")?,
        expected_last_event_sha256: digest_hex(&raw.expected_last_event_sha256, "expected event")?,
        predecessor_receipt_count: convert(
            raw.predecessor_receipt_count,
            "predecessor receipt count",
        )?,
        lifecycle_contract_admitted: lifecycle_admitted,
        provider_request_present: provider_present,
        execution_authority_released: execution_released,
        dispatch_authority_released: dispatch_released,
        progress_observed: progress,
        successor_advance_authorized: successor,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

fn parse_candidate_flags(raw: &rows::RawCandidateMetadata) -> Result<[bool; 6], HubStoreError> {
    Ok([
        parse_boolean(
            raw.lifecycle_contract_admitted,
            "lifecycle contract admission",
        )?,
        parse_boolean(raw.provider_request_present, "provider request presence")?,
        parse_boolean(raw.execution_authority_released, "execution authority")?,
        parse_boolean(raw.dispatch_authority_released, "dispatch authority")?,
        parse_boolean(raw.progress_observed, "progress observation")?,
        parse_boolean(raw.successor_advance_authorized, "successor advancement")?,
    ])
}

fn is_pristine_source(run: &GroupAgentGraphRunInspection) -> bool {
    run.run.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && run.run.last_event_seq == 1
        && run.events.len() == 1
        && !run.run.execution_contract_present
        && !run.run.dispatch_request_present
        && !run.run.dispatch_authority_released
}

fn validate_list_request(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_LIST_LIMIT).contains(&limit) {
        return Err(conflict(
            "scheduled-node contract list limit is outside its bounds",
        ));
    }
    let Some(id) = graph_run_id else {
        return Ok(());
    };
    if !rows::valid_lookup_id(id) {
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
            "stored scheduled-node contract idempotency key is invalid",
        ))
    }
}

fn digest_hex(value: &[u8], subject: &str) -> Result<String, HubStoreError> {
    let digest: [u8; 32] = value.try_into().map_err(|_| {
        corrupt(&format!(
            "stored scheduled-node contract {subject} digest is not 32 bytes"
        ))
    })?;
    Ok(group_run_codec::encode_hex_digest(&digest))
}

fn parse_boolean(value: i64, subject: &str) -> Result<bool, HubStoreError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(corrupt(&format!(
            "stored scheduled-node contract {subject} is invalid"
        ))),
    }
}

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| {
        corrupt(&format!(
            "invalid stored scheduled-node contract {subject}: {error}"
        ))
    })
}

fn parent_error(error: HubStoreError, parent: &str) -> HubStoreError {
    match error {
        HubStoreError::NotFound { .. } => corrupt(&format!(
            "stored scheduled-node contract references a missing {parent}"
        )),
        other => other,
    }
}

fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentScheduledNodeContract,
        id: id.into(),
    }
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
