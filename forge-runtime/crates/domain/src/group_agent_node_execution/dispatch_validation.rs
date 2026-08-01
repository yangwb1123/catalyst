use super::{
    GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION, GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
    GroupAgentNodeDispatchRequestInspection, GroupAgentNodeDispatchRequestRecord,
    GroupAgentNodeDispatchRequestValidationError, MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES,
    PrepareGroupAgentNodeDispatchRequest, group_agent_node_destination_sha256,
    group_agent_node_dispatch_request_id, group_agent_node_provider_request_sha256,
};
use crate::{
    GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION, GroupAgentGraphRunEventKind,
    GroupAgentGraphRunStatus, GroupAgentNodeExecutionProvider,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES, MAX_GROUP_AGENT_NODE_MODEL_BYTES,
    MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES,
};

pub(super) fn validate_record(
    record: &GroupAgentNodeDispatchRequestRecord,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    validate_provider(record.provider, &record.endpoint, &record.model)?;
    let valid = record.v == GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION
        && valid_identifier(&record.dispatch_request_id)
        && valid_identifier(&record.graph_run_id)
        && valid_identifier(&record.contract_id)
        && valid_identifier(&record.node_id)
        && record.attempt == 1
        && is_digest(&record.contract_sha256)
        && record.contract_id == format!("node-contract-{}", record.contract_sha256)
        && is_digest(&record.request_sha256)
        && is_digest(&record.project_lane_sha256)
        && valid_text(
            &record.endpoint,
            MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES,
        )
        && valid_text(&record.model, MAX_GROUP_AGENT_NODE_MODEL_BYTES)
        && is_digest(&record.pricing_snapshot_sha256)
        && is_digest(&record.provider_request_sha256)
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES)
            .contains(&record.provider_request_bytes)
        && record.destination_sha256
            == group_agent_node_destination_sha256(
                record.provider,
                &record.endpoint,
                &record.model,
            )
        && is_digest(&record.dispatch_request_sha256)
        && record.expected_sha256().as_deref() == Ok(record.dispatch_request_sha256.as_str())
        && record.codec_protocol_version == GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION
        && record.expected_last_event_seq == 2
        && is_digest(&record.expected_last_event_sha256)
        && record.dispatch_request_id
            == group_agent_node_dispatch_request_id(&record.dispatch_request_sha256)
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid prepared Node Dispatch Request record"))
}

pub(super) fn validate_record_event(
    record: &GroupAgentNodeDispatchRequestRecord,
    event: &crate::GroupAgentGraphRunEvent,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    record.validate()?;
    event.validate().map_err(|error| invalid(&error.message))?;
    (event.graph_run_id == record.graph_run_id && event_matches_record(&event.kind, record))
        .then_some(())
        .ok_or_else(|| invalid("Node Dispatch Request record and event disagree"))
}

pub(super) fn validate_prepare(
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    validate_provider(request.provider, &request.endpoint, &request.model)?;
    validate_prepare_header(request)?;
    request
        .event
        .validate()
        .map_err(|error| invalid(&error.message))?;
    if request.event.canonical_json().as_deref() != Ok(request.event_json.as_str())
        || request.event_json.len() > MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES
    {
        return Err(invalid("dispatch preparation event bytes disagree"));
    }
    validate_prepare_event(request)
}

fn validate_prepare_header(
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    let body_digest = group_agent_node_provider_request_sha256(&request.provider_request_body);
    let valid = request.v == GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION
        && valid_identifier(&request.dispatch_request_id)
        && valid_identifier(&request.graph_run_id)
        && valid_identifier(&request.contract_id)
        && valid_identifier(&request.node_id)
        && request.attempt == 1
        && is_digest(&request.contract_sha256)
        && request.contract_id == format!("node-contract-{}", request.contract_sha256)
        && is_digest(&request.request_sha256)
        && is_digest(&request.project_lane_sha256)
        && valid_text(
            &request.endpoint,
            MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES,
        )
        && valid_text(&request.model, MAX_GROUP_AGENT_NODE_MODEL_BYTES)
        && is_digest(&request.pricing_snapshot_sha256)
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES)
            .contains(&request.provider_request_body.len())
        && request.provider_request_sha256 == body_digest
        && request.destination_sha256
            == group_agent_node_destination_sha256(
                request.provider,
                &request.endpoint,
                &request.model,
            )
        && is_digest(&request.dispatch_request_sha256)
        && request.expected_sha256().as_deref() == Ok(request.dispatch_request_sha256.as_str())
        && request.codec_protocol_version == GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION
        && request.expected_last_event_seq == 2
        && is_digest(&request.expected_last_event_sha256)
        && request.dispatch_request_id
            == group_agent_node_dispatch_request_id(&request.dispatch_request_sha256)
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.prepared_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Dispatch Request preparation envelope"))
}

fn validate_prepare_event(
    request: &PrepareGroupAgentNodeDispatchRequest,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    let GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
        previous_event_sha256,
        contract_id,
        contract_sha256,
        dispatch_request_id,
        dispatch_request_sha256,
        request_body_sha256,
        request_body_bytes,
        logical_request_sha256,
        node_id,
        attempt,
        project_lane_sha256,
        codec_protocol_version,
        provider_kind,
        destination_sha256,
        pricing_snapshot_sha256,
        prepared_at_ms,
    } = &request.event.kind
    else {
        return Err(invalid("dispatch preparation requires its seq-3 event"));
    };
    let valid = request.event.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION
        && request.event.graph_run_id == request.graph_run_id
        && request.event.seq == 3
        && previous_event_sha256 == &request.expected_last_event_sha256
        && contract_id == &request.contract_id
        && contract_sha256 == &request.contract_sha256
        && dispatch_request_id == &request.dispatch_request_id
        && dispatch_request_sha256 == &request.dispatch_request_sha256
        && request_body_sha256 == &request.provider_request_sha256
        && *request_body_bytes == request.provider_request_body.len()
        && logical_request_sha256 == &request.request_sha256
        && node_id == &request.node_id
        && *attempt == request.attempt
        && project_lane_sha256 == &request.project_lane_sha256
        && *codec_protocol_version == request.codec_protocol_version
        && *provider_kind == request.provider
        && destination_sha256 == &request.destination_sha256
        && pricing_snapshot_sha256 == &request.pricing_snapshot_sha256
        && *prepared_at_ms == request.prepared_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| invalid("Node Dispatch Request event bindings disagree"))
}

pub(super) fn validate_inspection(
    inspection: &GroupAgentNodeDispatchRequestInspection,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    inspection.record.validate()?;
    inspection
        .contract
        .validate()
        .map_err(|error| invalid(&error.message))?;
    inspection
        .preparation_event
        .validate()
        .map_err(|error| invalid(&error.message))?;
    if inspection.preparation_event.canonical_json().as_deref()
        != Ok(inspection.preparation_event_json.as_str())
    {
        return Err(invalid("stored dispatch event bytes disagree"));
    }
    validate_inspection_bindings(inspection)
}

fn validate_inspection_bindings(
    inspection: &GroupAgentNodeDispatchRequestInspection,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    let record = &inspection.record;
    let contract = &inspection.contract;
    let run = &contract.graph_run.run;
    let admission_head = contract
        .admission_event
        .expected_sha256()
        .map_err(|error| invalid(&error.message))?;
    let valid = inspection.v == record.v
        && record.provider_request_bytes == inspection.provider_request_body.len()
        && record.provider_request_sha256
            == group_agent_node_provider_request_sha256(&inspection.provider_request_body)
        && record.expected_sha256().as_deref() == Ok(record.dispatch_request_sha256.as_str())
        && record.contract_id == contract.record.contract_id
        && record.contract_sha256 == contract.record.contract_sha256
        && record.graph_run_id == contract.record.graph_run_id
        && record.node_id == contract.record.node_id
        && record.attempt == contract.record.attempt
        && record.request_sha256 == contract.record.request_sha256
        && record.project_lane_sha256 == contract.record.project_lane_sha256
        && record.provider == contract.contract.provider.kind
        && record.endpoint == contract.contract.provider.endpoint
        && record.model == contract.contract.provider.model
        && record.pricing_snapshot_sha256 == contract.contract.budgets.pricing_snapshot_sha256
        && record.expected_last_event_seq == 2
        && record.expected_last_event_sha256 == admission_head
        && run.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingDispatchAuthorization
        && run.execution_contract_present
        && run.dispatch_request_present
        && !run.dispatch_authority_released
        && run.last_event_seq == 3
        && contract.graph_run.events.get(2) == Some(&inspection.preparation_event)
        && contract.graph_run.event_jsons.get(2) == Some(&inspection.preparation_event_json)
        && event_matches_record(&inspection.preparation_event.kind, record);
    valid
        .then_some(())
        .ok_or_else(|| invalid("prepared dispatch inspection bindings disagree"))
}

fn event_matches_record(
    kind: &GroupAgentGraphRunEventKind,
    record: &GroupAgentNodeDispatchRequestRecord,
) -> bool {
    let GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
        previous_event_sha256,
        contract_id,
        contract_sha256,
        dispatch_request_id,
        dispatch_request_sha256,
        request_body_sha256,
        request_body_bytes,
        logical_request_sha256,
        node_id,
        attempt,
        project_lane_sha256,
        codec_protocol_version,
        provider_kind,
        destination_sha256,
        pricing_snapshot_sha256,
        prepared_at_ms,
    } = kind
    else {
        return false;
    };
    previous_event_sha256 == &record.expected_last_event_sha256
        && contract_id == &record.contract_id
        && contract_sha256 == &record.contract_sha256
        && dispatch_request_id == &record.dispatch_request_id
        && dispatch_request_sha256 == &record.dispatch_request_sha256
        && request_body_sha256 == &record.provider_request_sha256
        && *request_body_bytes == record.provider_request_bytes
        && logical_request_sha256 == &record.request_sha256
        && node_id == &record.node_id
        && *attempt == record.attempt
        && project_lane_sha256 == &record.project_lane_sha256
        && *codec_protocol_version == record.codec_protocol_version
        && *provider_kind == record.provider
        && destination_sha256 == &record.destination_sha256
        && pricing_snapshot_sha256 == &record.pricing_snapshot_sha256
        && *prepared_at_ms == record.created_at_ms
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    super::super::validation::valid_text(value, maximum)
}

fn validate_provider(
    kind: crate::GroupAgentNodeProviderKind,
    endpoint: &str,
    model: &str,
) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
    super::super::validation::validate_provider(&GroupAgentNodeExecutionProvider {
        kind,
        endpoint: endpoint.into(),
        model: model.into(),
        store: false,
        stream: true,
    })
    .map_err(|error| invalid(&error.message))
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: &str) -> GroupAgentNodeDispatchRequestValidationError {
    GroupAgentNodeDispatchRequestValidationError {
        message: message.into(),
    }
}
