use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::runtime_domain::{
    GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION, GroupAgentGraphInspection,
    GroupAgentGraphRunEvent, GroupAgentGraphRunInspection, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractInspection, GroupAgentNodeExecutionContractRecord, HubEntity,
    HubStoreError, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_NODE_EXECUTION_CONTRACT_LIST_LIMIT,
};

use super::{
    super::{group_agent_graph, group_agent_graph_run, group_run_codec, read_error},
    codec, rows, snapshot,
};

pub(super) fn inspect(
    connection: &mut Connection,
    contract_id: &str,
) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, contract_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(super) fn inspect_in_snapshot(
    connection: &Connection,
    contract_id: &str,
) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
    let stored =
        rows::find_by_id(connection, contract_id)?.ok_or_else(|| not_found(contract_id))?;
    validate_stored(connection, stored)
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredContract,
) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
    let decoded = decode_stored(stored)?;
    let graph_run = load_graph_run_stored(connection, &decoded.record.graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(connection, &graph_run.run.graph_id)?;
    validate_with_sources(decoded, graph_run, &graph)
}

fn load_graph_run_stored(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
    group_agent_graph_run::read::inspect_in_snapshot(connection, graph_run_id).map_err(|error| {
        match error {
            HubStoreError::NotFound { .. } => {
                corrupt("stored Node Execution Contract references a missing Graph Run")
            }
            other => other,
        }
    })
}

pub(in crate::sqlite_hub) fn validate_graph_run_binding(
    connection: &Connection,
    graph_run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<Option<GroupAgentNodeExecutionContractInspection>, HubStoreError> {
    let stored = rows::find_by_run(connection, &graph_run.run.graph_run_id)?;
    match (graph_run.run.execution_contract_present, stored) {
        (false, None) => Ok(None),
        (true, Some(stored)) => {
            validate_with_sources(decode_stored(stored)?, graph_run.clone(), graph).map(Some)
        }
        (_, Some(stored)) => {
            decode_stored(stored)?;
            Err(corrupt(
                "stored Graph Run has an unexpected execution contract",
            ))
        }
        (true, None) => Err(corrupt("stored Graph Run execution contract is missing")),
    }
}

struct DecodedStoredContract {
    record: GroupAgentNodeExecutionContractRecord,
    contract_json: String,
    contract: GroupAgentNodeExecutionContract,
}

fn decode_stored(stored: rows::RawStoredContract) -> Result<DecodedStoredContract, HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata)?;
    if stored.contract_blob.len() != record.contract_bytes {
        return Err(corrupt(
            "stored Node Execution Contract byte count disagrees",
        ));
    }
    let (contract, contract_json) =
        codec::decode_contract(&stored.contract_blob, &record_digest(&record)?)?;
    Ok(DecodedStoredContract {
        record,
        contract_json,
        contract,
    })
}

fn validate_with_sources(
    decoded: DecodedStoredContract,
    graph_run: GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
    validate_reconstructed_control(&graph_run, graph, &decoded.contract)?;
    let (admission_event, admission_event_json) = admission_event(&graph_run)?;
    let inspection = GroupAgentNodeExecutionContractInspection {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        record: decoded.record,
        contract_json: decoded.contract_json,
        contract: decoded.contract,
        admission_event_json,
        admission_event,
        graph_run,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

fn validate_reconstructed_control(
    graph_run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    contract: &GroupAgentNodeExecutionContract,
) -> Result<(), HubStoreError> {
    let (control, _) = snapshot::reconstruct(graph_run, graph)?;
    contract
        .validate_against_control(&control)
        .map_err(|error| corrupt(&error.to_string()))
}

pub(super) fn list(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAgentNodeExecutionContractRecord>, HubStoreError> {
    validate_list_request(connection, graph_run_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    rows::query_metadata(connection, graph_run_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect()
}

fn admission_event(
    graph_run: &crate::runtime_domain::GroupAgentGraphRunInspection,
) -> Result<(GroupAgentGraphRunEvent, String), HubStoreError> {
    let event = graph_run
        .events
        .get(1)
        .cloned()
        .ok_or_else(|| corrupt("stored Node Execution Contract has no admission event"))?;
    let json = graph_run
        .event_jsons
        .get(1)
        .cloned()
        .ok_or_else(|| corrupt("stored Node Execution Contract has no admission event JSON"))?;
    Ok((event, json))
}

fn metadata_record(
    raw: rows::RawContractMetadata,
) -> Result<GroupAgentNodeExecutionContractRecord, HubStoreError> {
    let record = GroupAgentNodeExecutionContractRecord {
        v: convert(raw.contract_version, "contract version")?,
        contract_id: raw.id,
        graph_run_id: raw.graph_run_id,
        node_id: raw.node_id,
        attempt: convert(raw.attempt, "attempt")?,
        control_snapshot_sha256: codec::digest_hex(
            &raw.control_snapshot_sha256,
            "control snapshot",
        )?,
        contract_sha256: codec::digest_hex(&raw.contract_sha256, "contract")?,
        contract_bytes: convert(raw.contract_bytes, "contract byte count")?,
        request_sha256: codec::digest_hex(&raw.request_sha256, "request")?,
        project_lane_sha256: codec::digest_hex(&raw.project_lane_sha256, "project lane")?,
        expected_last_event_seq: convert(
            raw.expected_last_event_seq,
            "expected last event sequence",
        )?,
        expected_last_event_sha256: codec::digest_hex(
            &raw.expected_last_event_sha256,
            "expected last event",
        )?,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

fn record_digest(
    record: &GroupAgentNodeExecutionContractRecord,
) -> Result<[u8; 32], HubStoreError> {
    codec::candidate_digest(&record.contract_sha256, "contract")
        .map_err(|error| corrupt(&error.to_string()))
}

fn validate_stored_key(key: &str) -> Result<(), HubStoreError> {
    if group_run_codec::valid_text(key, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(corrupt(
            "stored Node Execution Contract idempotency key is invalid",
        ))
    }
}

fn validate_list_request(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_AGENT_NODE_EXECUTION_CONTRACT_LIST_LIMIT).contains(&limit) {
        return Err(conflict(
            "Node Execution Contract list limit is outside its bounds",
        ));
    }
    let Some(id) = graph_run_id else {
        return Ok(());
    };
    if !group_run_codec::valid_text(id, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES) {
        return Err(conflict("Graph Run filter is outside its bounds"));
    }
    connection
        .query_row(
            "SELECT 1 FROM group_agent_graph_runs WHERE id = ?1",
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

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| {
        corrupt(&format!(
            "invalid Node Execution Contract {subject}: {error}"
        ))
    })
}

fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentNodeExecutionContract,
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
        entity: HubEntity::GroupAgentNodeExecutionContract,
        message: message.into(),
    }
}
