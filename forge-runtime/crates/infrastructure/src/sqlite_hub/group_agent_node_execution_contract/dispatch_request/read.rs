use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::{
    OpenAiResponsesProvider,
    runtime_domain::{
        Cancellation, GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION, GroupAgentGraphRunEvent,
        GroupAgentGraphRunInspection, GroupAgentNodeDispatchRequestInspection,
        GroupAgentNodeDispatchRequestRecord, GroupAgentNodeExecutionContract,
        GroupAgentNodeExecutionContractInspection, GroupAgentNodeProviderKind, HubEntity,
        HubStoreError, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_NODE_DISPATCH_REQUEST_LIST_LIMIT,
        Message, ModelRequest, group_agent_node_provider_request_sha256,
    },
};

use super::super::super::{group_run_codec, read_error};
use super::super::read as contract_read;
use super::{codec, rows};

pub(in crate::sqlite_hub) fn validate_graph_run_binding(
    connection: &Connection,
    graph_run: &GroupAgentGraphRunInspection,
    contract: Option<&GroupAgentNodeExecutionContractInspection>,
) -> Result<(), HubStoreError> {
    let stored = rows::find_by_run(connection, &graph_run.run.graph_run_id)?;
    match (graph_run.run.dispatch_request_present, stored) {
        (false, None) => Ok(()),
        (true, Some(stored)) => {
            let contract = contract.ok_or_else(|| {
                corrupt("stored Graph Run dispatch request has no execution contract")
            })?;
            validate_stored_with_contract(stored, contract.clone()).map(|_| ())
        }
        (_, Some(stored)) => {
            decode_stored(stored)?;
            Err(corrupt(
                "stored Graph Run has an unexpected dispatch request",
            ))
        }
        (true, None) => Err(corrupt("stored Graph Run dispatch request is missing")),
    }
}

fn decode_stored(
    stored: rows::RawStoredRequest,
) -> Result<(GroupAgentNodeDispatchRequestRecord, Vec<u8>), HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata)?;
    let valid = stored.provider_request_blob.len() == record.provider_request_bytes
        && group_agent_node_provider_request_sha256(&stored.provider_request_blob)
            == record.provider_request_sha256;
    valid
        .then_some((record, stored.provider_request_blob))
        .ok_or_else(|| corrupt("stored Node Dispatch Request body metadata disagrees"))
}

pub(super) fn inspect(
    connection: &mut Connection,
    dispatch_request_id: &str,
) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, dispatch_request_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(super) fn inspect_in_snapshot(
    connection: &Connection,
    dispatch_request_id: &str,
) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
    let stored = rows::find_by_id(connection, dispatch_request_id)?
        .ok_or_else(|| not_found(dispatch_request_id))?;
    validate_stored(connection, stored)
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredRequest,
) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
    let (record, body) = decode_stored(stored)?;
    let contract = contract_read::inspect_in_snapshot(connection, &record.contract_id)
        .map_err(stored_contract_error)?;
    validate_decoded(record, body, contract)
}

fn validate_stored_with_contract(
    stored: rows::RawStoredRequest,
    contract: GroupAgentNodeExecutionContractInspection,
) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
    let (record, body) = decode_stored(stored)?;
    validate_decoded(record, body, contract)
}

fn validate_decoded(
    record: GroupAgentNodeDispatchRequestRecord,
    body: Vec<u8>,
    contract: GroupAgentNodeExecutionContractInspection,
) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
    validate_exact_provider_body(&contract.contract, &body)?;
    let (preparation_event, preparation_event_json) = preparation_event(&contract)?;
    let inspection = GroupAgentNodeDispatchRequestInspection {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        record,
        provider_request_body: body,
        preparation_event_json,
        preparation_event,
        contract,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn list(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAgentNodeDispatchRequestRecord>, HubStoreError> {
    validate_list_request(connection, graph_run_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    rows::query_metadata(connection, graph_run_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect()
}

fn preparation_event(
    contract: &crate::runtime_domain::GroupAgentNodeExecutionContractInspection,
) -> Result<(GroupAgentGraphRunEvent, String), HubStoreError> {
    let event = contract
        .graph_run
        .events
        .get(2)
        .cloned()
        .ok_or_else(|| corrupt("stored Node Dispatch Request has no preparation event"))?;
    let json = contract
        .graph_run
        .event_jsons
        .get(2)
        .cloned()
        .ok_or_else(|| corrupt("stored Node Dispatch Request has no preparation event JSON"))?;
    Ok((event, json))
}

pub(super) fn validate_exact_provider_body(
    contract: &GroupAgentNodeExecutionContract,
    body: &[u8],
) -> Result<(), HubStoreError> {
    let request = ModelRequest {
        system_prompt: contract.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: contract.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: contract.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    };
    OpenAiResponsesProvider::validate_exact_request_bytes(&contract.provider.model, &request, body)
        .map_err(|error| {
            corrupt(&format!(
                "stored provider request body failed exact codec validation: {error}"
            ))
        })
}

fn metadata_record(
    raw: rows::RawRequestMetadata,
) -> Result<GroupAgentNodeDispatchRequestRecord, HubStoreError> {
    let record = GroupAgentNodeDispatchRequestRecord {
        v: convert(raw.request_version, "request version")?,
        dispatch_request_id: raw.id,
        graph_run_id: raw.graph_run_id,
        contract_id: raw.contract_id,
        node_id: raw.node_id,
        attempt: convert(raw.attempt, "attempt")?,
        contract_sha256: codec::digest_hex(&raw.contract_sha256, "contract")?,
        request_sha256: codec::digest_hex(&raw.request_sha256, "logical request")?,
        project_lane_sha256: codec::digest_hex(&raw.project_lane_sha256, "project lane")?,
        provider: parse_provider(&raw.provider_kind)?,
        endpoint: raw.endpoint,
        model: raw.model,
        pricing_snapshot_sha256: codec::digest_hex(
            &raw.pricing_snapshot_sha256,
            "pricing snapshot",
        )?,
        provider_request_sha256: codec::digest_hex(
            &raw.provider_request_sha256,
            "provider request",
        )?,
        provider_request_bytes: convert(raw.provider_request_bytes, "provider request bytes")?,
        destination_sha256: codec::digest_hex(&raw.destination_sha256, "destination")?,
        dispatch_request_sha256: codec::digest_hex(
            &raw.dispatch_request_sha256,
            "dispatch request",
        )?,
        codec_protocol_version: convert(raw.codec_protocol_version, "codec protocol version")?,
        expected_last_event_seq: convert(raw.expected_last_event_seq, "expected event sequence")?,
        expected_last_event_sha256: codec::digest_hex(
            &raw.expected_last_event_sha256,
            "expected event head",
        )?,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

fn validate_stored_key(key: &str) -> Result<(), HubStoreError> {
    if group_run_codec::valid_text(key, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(corrupt(
            "stored Node Dispatch Request idempotency key is invalid",
        ))
    }
}

fn validate_list_request(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_AGENT_NODE_DISPATCH_REQUEST_LIST_LIMIT).contains(&limit) {
        return Err(conflict(
            "Node Dispatch Request list limit is outside its bounds",
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

fn parse_provider(value: &str) -> Result<GroupAgentNodeProviderKind, HubStoreError> {
    match value {
        "openai_responses" => Ok(GroupAgentNodeProviderKind::OpenAiResponses),
        _ => Err(corrupt(
            "stored Node Dispatch Request provider kind is unsupported",
        )),
    }
}

fn stored_contract_error(error: HubStoreError) -> HubStoreError {
    match error {
        HubStoreError::NotFound { .. } => {
            corrupt("stored Node Dispatch Request references a missing contract")
        }
        other => other,
    }
}

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value)
        .map_err(|error| corrupt(&format!("invalid Node Dispatch Request {subject}: {error}")))
}

fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentNodeDispatchRequest,
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
        entity: HubEntity::GroupAgentNodeDispatchRequest,
        message: message.into(),
    }
}
