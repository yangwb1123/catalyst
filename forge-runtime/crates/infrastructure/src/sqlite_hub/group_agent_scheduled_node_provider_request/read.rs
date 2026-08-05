use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use crate::{
    OpenAiResponsesProvider,
    runtime_domain::{
        Cancellation, GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        GroupAgentGraphRunInspection, GroupAgentNodeProviderKind,
        GroupAgentScheduledNodeContractInspection,
        GroupAgentScheduledNodeProviderRequestInspection,
        GroupAgentScheduledNodeProviderRequestRecord, HubEntity, HubStoreError,
        MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        MAX_GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_LIST_LIMIT, Message, ModelRequest,
        group_agent_node_provider_request_sha256,
    },
};

use super::super::group_agent_scheduled_node_successor::rows as successor_rows;
use super::super::{
    group_agent_scheduled_node_contract, group_agent_scheduled_node_successor, group_run_codec,
    read_error,
};
use super::rows;

pub(super) fn inspect(
    connection: &mut Connection,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, provider_request_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(in crate::sqlite_hub) fn inspect_in_snapshot(
    connection: &Connection,
    provider_request_id: &str,
) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
    let stored = rows::find_by_id(connection, provider_request_id)?
        .ok_or_else(|| not_found(provider_request_id))?;
    validate_stored(connection, stored)
}

pub(in crate::sqlite_hub) fn validate_graph_run_binding(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    scheduled_contract: Option<&GroupAgentScheduledNodeContractInspection>,
) -> Result<Option<GroupAgentScheduledNodeProviderRequestInspection>, HubStoreError> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(read_error)?;
    if version < 15 {
        return Ok(None);
    }
    let Some(stored) = rows::find_by_run(connection, &run.run.graph_run_id)? else {
        return Ok(None);
    };
    let scheduled_contract = if let Some(contract) = scheduled_contract {
        contract.clone()
    } else {
        // The bound contract may live in the successor candidate table.
        // Lightweight decode only: re-inspecting the Graph Run from here
        // would recurse back into this validator.
        let stored =
            successor_rows::find_by_run(connection, &run.run.graph_run_id)?.ok_or_else(|| {
                corrupt("stored scheduled-node provider request has no scheduled contract")
            })?;
        group_agent_scheduled_node_successor::read::decode_stored(stored)?.inspection
    };
    validate_stored_with_contract(stored, scheduled_contract).map(Some)
}

pub(in crate::sqlite_hub) fn has_graph_run_child(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<bool, HubStoreError> {
    rows::exists_for_run(connection, graph_run_id)
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredRequest,
) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
    let (record, body) = decode_stored(stored)?;
    let scheduled_contract = load_source_contract(connection, &record.scheduled_contract_id)?;
    validate_decoded(record, body, scheduled_contract)
}

/// Loads the bound contract from either the initial or the successor
/// candidate table.
fn load_source_contract(
    connection: &Connection,
    scheduled_contract_id: &str,
) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
    match group_agent_scheduled_node_contract::read::inspect_in_snapshot(
        connection,
        scheduled_contract_id,
    ) {
        Ok(inspection) => Ok(inspection),
        Err(HubStoreError::NotFound { .. }) => {
            group_agent_scheduled_node_successor::read::inspect_in_snapshot(
                connection,
                scheduled_contract_id,
            )
            .map_err(stored_contract_error)
        }
        Err(other) => Err(other),
    }
}

fn validate_stored_with_contract(
    stored: rows::RawStoredRequest,
    scheduled_contract: GroupAgentScheduledNodeContractInspection,
) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
    let (record, body) = decode_stored(stored)?;
    validate_decoded(record, body, scheduled_contract)
}

fn decode_stored(
    stored: rows::RawStoredRequest,
) -> Result<(GroupAgentScheduledNodeProviderRequestRecord, Vec<u8>), HubStoreError> {
    validate_stored_key(&stored.idempotency_key)?;
    let record = metadata_record(stored.metadata)?;
    let exact = stored.provider_request_blob.len() == record.provider_request_bytes
        && group_agent_node_provider_request_sha256(&stored.provider_request_blob)
            == record.provider_request_sha256;
    exact
        .then_some((record, stored.provider_request_blob))
        .ok_or_else(|| corrupt("stored scheduled-node provider request body metadata disagrees"))
}

fn validate_decoded(
    record: GroupAgentScheduledNodeProviderRequestRecord,
    body: Vec<u8>,
    scheduled_contract: GroupAgentScheduledNodeContractInspection,
) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
    validate_exact_provider_body(&scheduled_contract, &body)?;
    let inspection = GroupAgentScheduledNodeProviderRequestInspection {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        record,
        provider_request_body: body,
        scheduled_contract,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_exact_provider_body(
    scheduled_contract: &GroupAgentScheduledNodeContractInspection,
    body: &[u8],
) -> Result<(), HubStoreError> {
    let candidate = &scheduled_contract.candidate;
    let request = ModelRequest {
        system_prompt: candidate.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: candidate.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: candidate.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    };
    OpenAiResponsesProvider::validate_exact_request_bytes(&candidate.provider.model, &request, body)
        .map_err(|error| {
            corrupt(&format!(
                "stored scheduled-node provider request failed exact codec validation: {error}"
            ))
        })
}

pub(super) fn list(
    connection: &mut Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupAgentScheduledNodeProviderRequestRecord>, HubStoreError> {
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

fn metadata_record(
    raw: rows::RawRequestMetadata,
) -> Result<GroupAgentScheduledNodeProviderRequestRecord, HubStoreError> {
    let flags = parse_flags(&raw)?;
    let digests = parse_digests(&raw)?;
    let record = GroupAgentScheduledNodeProviderRequestRecord {
        v: convert(raw.provider_request_version, "provider request version")?,
        provider_request_id: raw.id,
        graph_run_id: raw.graph_run_id,
        schedule_id: raw.schedule_id,
        scheduled_contract_id: raw.scheduled_contract_id,
        execution_ordinal: convert(raw.execution_ordinal, "execution ordinal")?,
        node_id: raw.node_id,
        attempt: convert(raw.attempt, "attempt")?,
        scheduled_contract_sha256: digests.scheduled_contract,
        logical_request_id: raw.logical_request_id,
        logical_request_sha256: digests.logical_request,
        schedule_sha256: digests.schedule,
        project_lane_sha256: digests.project_lane,
        provider: parse_provider(&raw.provider_kind)?,
        endpoint: raw.endpoint,
        model: raw.model,
        destination_sha256: digests.destination,
        pricing_snapshot_sha256: digests.pricing,
        provider_request_sha256: digests.provider_request,
        provider_request_bytes: convert(raw.provider_request_bytes, "provider request bytes")?,
        prepared_request_sha256: digests.prepared_request,
        codec_protocol_version: convert(raw.codec_protocol_version, "codec protocol version")?,
        expected_last_event_seq: convert(raw.expected_last_event_seq, "expected event sequence")?,
        expected_last_event_sha256: digests.expected_head,
        provider_request_prepared: flags.prepared,
        provider_request_sent: flags.sent,
        lifecycle_contract_admitted: flags.lifecycle,
        execution_authority_released: flags.execution,
        dispatch_authority_released: flags.dispatch,
        project_lane_claimed: flags.lane,
        progress_observed: flags.progress,
        successor_advance_authorized: flags.successor,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    record
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(record)
}

struct RequestDigests {
    scheduled_contract: String,
    logical_request: String,
    schedule: String,
    project_lane: String,
    destination: String,
    pricing: String,
    provider_request: String,
    prepared_request: String,
    expected_head: String,
}

fn parse_digests(raw: &rows::RawRequestMetadata) -> Result<RequestDigests, HubStoreError> {
    Ok(RequestDigests {
        scheduled_contract: digest_hex(&raw.scheduled_contract_sha256, "scheduled contract")?,
        logical_request: digest_hex(&raw.logical_request_sha256, "logical request")?,
        schedule: digest_hex(&raw.schedule_sha256, "schedule")?,
        project_lane: digest_hex(&raw.project_lane_sha256, "project lane")?,
        destination: digest_hex(&raw.destination_sha256, "destination")?,
        pricing: digest_hex(&raw.pricing_snapshot_sha256, "pricing snapshot")?,
        provider_request: digest_hex(&raw.provider_request_sha256, "provider request")?,
        prepared_request: digest_hex(&raw.prepared_request_sha256, "prepared request")?,
        expected_head: digest_hex(&raw.expected_last_event_sha256, "expected event head")?,
    })
}

#[allow(clippy::struct_excessive_bools)]
struct RequestFlags {
    prepared: bool,
    sent: bool,
    lifecycle: bool,
    execution: bool,
    dispatch: bool,
    lane: bool,
    progress: bool,
    successor: bool,
}

fn parse_flags(raw: &rows::RawRequestMetadata) -> Result<RequestFlags, HubStoreError> {
    Ok(RequestFlags {
        prepared: parse_boolean(
            raw.provider_request_prepared,
            "provider request preparation",
        )?,
        sent: parse_boolean(raw.provider_request_sent, "provider request send")?,
        lifecycle: parse_boolean(
            raw.lifecycle_contract_admitted,
            "lifecycle contract admission",
        )?,
        execution: parse_boolean(raw.execution_authority_released, "execution authority")?,
        dispatch: parse_boolean(raw.dispatch_authority_released, "dispatch authority")?,
        lane: parse_boolean(raw.project_lane_claimed, "Project lane claim")?,
        progress: parse_boolean(raw.progress_observed, "progress observation")?,
        successor: parse_boolean(raw.successor_advance_authorized, "successor advancement")?,
    })
}

fn validate_stored_key(key: &str) -> Result<(), HubStoreError> {
    if group_run_codec::valid_text(key, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES) {
        Ok(())
    } else {
        Err(corrupt(
            "stored scheduled-node provider request idempotency key is invalid",
        ))
    }
}

fn validate_list_request(
    connection: &Connection,
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_LIST_LIMIT).contains(&limit) {
        return Err(conflict(
            "scheduled-node provider request list limit is outside its bounds",
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

fn digest_hex(bytes: &[u8], subject: &str) -> Result<String, HubStoreError> {
    let digest: [u8; 32] = bytes.try_into().map_err(|_| {
        corrupt(&format!(
            "stored scheduled-node provider request {subject} digest is not 32 bytes"
        ))
    })?;
    Ok(group_run_codec::encode_hex_digest(&digest))
}

pub(super) fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    group_run_codec::decode_hex_digest(value).ok_or_else(|| {
        conflict(&format!(
            "scheduled-node provider request {subject} digest is invalid"
        ))
    })
}

fn parse_provider(value: &str) -> Result<GroupAgentNodeProviderKind, HubStoreError> {
    match value {
        "openai_responses" => Ok(GroupAgentNodeProviderKind::OpenAiResponses),
        _ => Err(corrupt(
            "stored scheduled-node provider request provider kind is unsupported",
        )),
    }
}

fn parse_boolean(value: i64, subject: &str) -> Result<bool, HubStoreError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(corrupt(&format!(
            "stored scheduled-node provider request {subject} is not Boolean"
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
            "invalid scheduled-node provider request {subject}: {error}"
        ))
    })
}

fn stored_contract_error(error: HubStoreError) -> HubStoreError {
    match error {
        HubStoreError::NotFound { .. } => {
            corrupt("stored scheduled-node provider request references a missing contract")
        }
        other => other,
    }
}

fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentScheduledNodeProviderRequest,
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
        entity: HubEntity::GroupAgentScheduledNodeProviderRequest,
        message: message.into(),
    }
}
