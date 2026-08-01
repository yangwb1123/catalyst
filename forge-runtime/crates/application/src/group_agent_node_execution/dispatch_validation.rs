use std::collections::BTreeSet;

use crate::runtime_domain::{
    Cancellation, GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
    GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION, GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
    GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunStatus, GroupAgentNodeDispatchRequestInspection,
    GroupAgentNodeDispatchRequestRecord, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractInspection, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_NODE_DISPATCH_REQUEST_LIST_LIMIT, Message, ModelRequest,
    PrepareGroupAgentNodeDispatchRequest, PrepareGroupAgentNodeDispatchRequestDisposition,
    PrepareGroupAgentNodeDispatchRequestResult, group_agent_node_destination_sha256,
    group_agent_node_dispatch_request_id, group_agent_node_provider_request_sha256,
};

use super::{
    GroupAgentNodeDispatchRequestServiceError, PrepareGroupAgentNodeDispatchRequestInput,
    dispatch_error::{conflict, corrupt, invalid},
};

pub(super) fn validate_input(
    input: &PrepareGroupAgentNodeDispatchRequestInput,
) -> Result<(), GroupAgentNodeDispatchRequestServiceError> {
    super::validation::validate_identifier(&input.graph_run_id, "Graph Run ID")?;
    valid_text(
        &input.idempotency_key,
        MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    )
    .then_some(())
    .ok_or_else(|| invalid("idempotency key is invalid"))?;
    if i64::try_from(input.prepared_at_ms).is_err() {
        return Err(invalid("preparation time is invalid"));
    }
    Ok(())
}

pub(super) fn model_request(contract: &GroupAgentNodeExecutionContract) -> ModelRequest {
    ModelRequest {
        system_prompt: contract.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: contract.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: contract.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    }
}

pub(super) fn prepare_request(
    input: &PrepareGroupAgentNodeDispatchRequestInput,
    inspection: &GroupAgentNodeExecutionContractInspection,
    body: Vec<u8>,
) -> Result<PrepareGroupAgentNodeDispatchRequest, GroupAgentNodeDispatchRequestServiceError> {
    require_prepare_state(inspection)?;
    let contract = &inspection.contract;
    if contract.graph_run_id != input.graph_run_id {
        return Err(invalid("Graph Run and admitted contract bindings disagree"));
    }
    let previous_event_sha256 = inspection
        .admission_event
        .expected_sha256()
        .map_err(|error| {
            corrupt(&format!(
                "admission event digest cannot be reconstructed: {error}"
            ))
        })?;
    let mut request = candidate(input, contract, body, previous_event_sha256);
    request.dispatch_request_sha256 = request
        .expected_sha256()
        .map_err(|error| invalid(&error.message))?;
    request.dispatch_request_id =
        group_agent_node_dispatch_request_id(&request.dispatch_request_sha256);
    request.event = preparation_event(input, &request);
    request.event_json = request
        .event
        .canonical_json()
        .map_err(|error| corrupt(&error.to_string()))?;
    request
        .validate()
        .map_err(|error| invalid(&error.message))?;
    Ok(request)
}

fn candidate(
    input: &PrepareGroupAgentNodeDispatchRequestInput,
    contract: &GroupAgentNodeExecutionContract,
    body: Vec<u8>,
    previous_event_sha256: String,
) -> PrepareGroupAgentNodeDispatchRequest {
    let provider_request_sha256 = group_agent_node_provider_request_sha256(&body);
    PrepareGroupAgentNodeDispatchRequest {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        dispatch_request_id: String::new(),
        graph_run_id: contract.graph_run_id.clone(),
        contract_id: contract.contract_id.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        node_id: contract.node.node_id.clone(),
        attempt: contract.node.attempt,
        request_sha256: contract.request.request_sha256.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        provider: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        pricing_snapshot_sha256: contract.budgets.pricing_snapshot_sha256.clone(),
        provider_request_body: body,
        provider_request_sha256,
        destination_sha256: group_agent_node_destination_sha256(
            contract.provider.kind,
            &contract.provider.endpoint,
            &contract.provider.model,
        ),
        dispatch_request_sha256: String::new(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: 2,
        expected_last_event_sha256: previous_event_sha256,
        event: placeholder_event(&contract.graph_run_id),
        event_json: String::new(),
        idempotency_key: input.idempotency_key.clone(),
        prepared_at_ms: input.prepared_at_ms,
    }
}

fn require_prepare_state(
    inspection: &GroupAgentNodeExecutionContractInspection,
) -> Result<(), GroupAgentNodeDispatchRequestServiceError> {
    let run = &inspection.graph_run.run;
    let admitted = run.v == GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingCoreDispatch
        && !run.dispatch_request_present
        && run.last_event_seq == 2;
    let replay = run.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingDispatchAuthorization
        && run.dispatch_request_present
        && run.last_event_seq == 3;
    (admitted || replay)
        .then_some(())
        .ok_or_else(|| conflict("preparation requires an exact v2 or replayable v3 state"))
}

fn preparation_event(
    input: &PrepareGroupAgentNodeDispatchRequestInput,
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        graph_run_id: request.graph_run_id.clone(),
        seq: 3,
        kind: GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256: request.expected_last_event_sha256.clone(),
            contract_id: request.contract_id.clone(),
            contract_sha256: request.contract_sha256.clone(),
            dispatch_request_id: request.dispatch_request_id.clone(),
            dispatch_request_sha256: request.dispatch_request_sha256.clone(),
            request_body_sha256: request.provider_request_sha256.clone(),
            request_body_bytes: request.provider_request_body.len(),
            logical_request_sha256: request.request_sha256.clone(),
            node_id: request.node_id.clone(),
            attempt: request.attempt,
            project_lane_sha256: request.project_lane_sha256.clone(),
            codec_protocol_version: request.codec_protocol_version,
            provider_kind: request.provider,
            destination_sha256: request.destination_sha256.clone(),
            pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
            prepared_at_ms: input.prepared_at_ms,
        },
    }
}

fn placeholder_event(graph_run_id: &str) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        graph_run_id: graph_run_id.into(),
        seq: 3,
        kind: GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256: String::new(),
            contract_id: String::new(),
            contract_sha256: String::new(),
            dispatch_request_id: String::new(),
            dispatch_request_sha256: String::new(),
            request_body_sha256: String::new(),
            request_body_bytes: 0,
            logical_request_sha256: String::new(),
            node_id: String::new(),
            attempt: 0,
            project_lane_sha256: String::new(),
            codec_protocol_version: 0,
            provider_kind: crate::runtime_domain::GroupAgentNodeProviderKind::OpenAiResponses,
            destination_sha256: String::new(),
            pricing_snapshot_sha256: String::new(),
            prepared_at_ms: 0,
        },
    }
}

pub(super) fn validate_result(
    request: &PrepareGroupAgentNodeDispatchRequest,
    result: PrepareGroupAgentNodeDispatchRequestResult,
) -> Result<PrepareGroupAgentNodeDispatchRequestResult, GroupAgentNodeDispatchRequestServiceError> {
    if result.v != GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION {
        return Err(corrupt("store returned an unsupported preparation version"));
    }
    let inspection = checked_inspection(result.inspection)?;
    validate_result_semantics(request, result.disposition, &inspection)?;
    Ok(PrepareGroupAgentNodeDispatchRequestResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

fn validate_result_semantics(
    request: &PrepareGroupAgentNodeDispatchRequest,
    disposition: PrepareGroupAgentNodeDispatchRequestDisposition,
    inspection: &GroupAgentNodeDispatchRequestInspection,
) -> Result<(), GroupAgentNodeDispatchRequestServiceError> {
    let record = &inspection.record;
    let semantics = record.dispatch_request_id == request.dispatch_request_id
        && record.graph_run_id == request.graph_run_id
        && record.contract_id == request.contract_id
        && record.provider_request_sha256 == request.provider_request_sha256
        && inspection.provider_request_body == request.provider_request_body;
    let created = disposition != PrepareGroupAgentNodeDispatchRequestDisposition::Created
        || (record.created_at_ms == request.prepared_at_ms
            && inspection.preparation_event == request.event
            && inspection.preparation_event_json == request.event_json);
    (semantics && created)
        .then_some(())
        .ok_or_else(|| corrupt("store returned different dispatch request semantics"))
}

pub(super) fn checked_inspection(
    inspection: GroupAgentNodeDispatchRequestInspection,
) -> Result<GroupAgentNodeDispatchRequestInspection, GroupAgentNodeDispatchRequestServiceError> {
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_list_input(
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentNodeDispatchRequestServiceError> {
    if !(1..=MAX_GROUP_AGENT_NODE_DISPATCH_REQUEST_LIST_LIMIT).contains(&limit) {
        return Err(invalid("dispatch request list limit is outside its bounds"));
    }
    if let Some(id) = graph_run_id {
        super::validation::validate_identifier(id, "Graph Run ID")?;
    }
    Ok(())
}

pub(super) fn validate_list(
    records: &[GroupAgentNodeDispatchRequestRecord],
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentNodeDispatchRequestServiceError> {
    if records.len() > limit {
        return Err(corrupt("store returned too many dispatch request records"));
    }
    let mut requests = BTreeSet::new();
    let mut runs = BTreeSet::new();
    for record in records {
        record.validate().map_err(|error| corrupt(&error.message))?;
        if graph_run_id.is_some_and(|id| id != record.graph_run_id)
            || !requests.insert(record.dispatch_request_id.as_str())
            || !runs.insert(record.graph_run_id.as_str())
        {
            return Err(corrupt(
                "store returned unfiltered or duplicate request metadata",
            ));
        }
    }
    Ok(())
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}
