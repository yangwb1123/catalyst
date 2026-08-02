use super::{
    GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestRecord,
    GroupAgentScheduledNodeProviderRequestValidationError,
    PrepareGroupAgentScheduledNodeProviderRequest,
};
use crate::{
    GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION, GroupAgentNodeExecutionProvider,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_NODE_MODEL_BYTES,
    MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES, MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES,
    group_agent_node_destination_sha256, group_agent_node_provider_request_sha256,
};

pub(super) fn validate_record(
    record: &GroupAgentScheduledNodeProviderRequestRecord,
) -> Result<(), GroupAgentScheduledNodeProviderRequestValidationError> {
    validate_provider(record.provider, &record.endpoint, &record.model)?;
    let valid = valid_record_source(record)
        && valid_record_destination(record)
        && valid_record_envelope(record);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid scheduled-node provider request record"))
}

pub(super) fn validate_prepare(
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
) -> Result<(), GroupAgentScheduledNodeProviderRequestValidationError> {
    validate_provider(request.provider, &request.endpoint, &request.model)?;
    let valid = valid_prepare_source(request)
        && valid_prepare_destination(request)
        && valid_prepare_envelope(request)
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.prepared_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid scheduled-node provider request preparation"))
}

fn valid_record_source(record: &GroupAgentScheduledNodeProviderRequestRecord) -> bool {
    valid_source_identity(
        record.v,
        &record.graph_run_id,
        &record.schedule_id,
        &record.schedule_sha256,
        &record.scheduled_contract_id,
        &record.scheduled_contract_sha256,
        record.execution_ordinal,
        &record.node_id,
        record.attempt,
        &record.logical_request_id,
        &record.logical_request_sha256,
    )
}

fn valid_prepare_source(request: &PrepareGroupAgentScheduledNodeProviderRequest) -> bool {
    valid_source_identity(
        request.v,
        &request.graph_run_id,
        &request.schedule_id,
        &request.schedule_sha256,
        &request.scheduled_contract_id,
        &request.scheduled_contract_sha256,
        request.execution_ordinal,
        &request.node_id,
        request.attempt,
        &request.logical_request_id,
        &request.logical_request_sha256,
    )
}

#[allow(clippy::too_many_arguments)]
fn valid_source_identity(
    version: u16,
    graph_run_id: &str,
    schedule_id: &str,
    schedule_sha256: &str,
    contract_id: &str,
    contract_sha256: &str,
    ordinal: usize,
    node_id: &str,
    attempt: u16,
    logical_request_id: &str,
    logical_request_sha256: &str,
) -> bool {
    version == GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION
        && valid_identifier(graph_run_id)
        && content_id(schedule_id, "graph-execution-schedule-", schedule_sha256)
        && content_id(contract_id, "scheduled-node-contract-", contract_sha256)
        && ordinal == 0
        && valid_identifier(node_id)
        && attempt == 1
        && content_id(
            logical_request_id,
            "scheduled-node-request-",
            logical_request_sha256,
        )
}

fn valid_record_destination(record: &GroupAgentScheduledNodeProviderRequestRecord) -> bool {
    valid_destination(
        record.provider,
        &record.endpoint,
        &record.model,
        &record.destination_sha256,
        &record.project_lane_sha256,
        &record.pricing_snapshot_sha256,
    ) && digest(&record.provider_request_sha256)
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES)
            .contains(&record.provider_request_bytes)
}

fn valid_prepare_destination(request: &PrepareGroupAgentScheduledNodeProviderRequest) -> bool {
    valid_destination(
        request.provider,
        &request.endpoint,
        &request.model,
        &request.destination_sha256,
        &request.project_lane_sha256,
        &request.pricing_snapshot_sha256,
    ) && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES)
        .contains(&request.provider_request_body.len())
        && request.provider_request_sha256
            == group_agent_node_provider_request_sha256(&request.provider_request_body)
}

fn valid_destination(
    provider: crate::GroupAgentNodeProviderKind,
    endpoint: &str,
    model: &str,
    destination_sha256: &str,
    project_lane_sha256: &str,
    pricing_snapshot_sha256: &str,
) -> bool {
    digest(project_lane_sha256)
        && valid_text(endpoint, MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES)
        && valid_text(model, MAX_GROUP_AGENT_NODE_MODEL_BYTES)
        && destination_sha256 == group_agent_node_destination_sha256(provider, endpoint, model)
        && digest(pricing_snapshot_sha256)
}

fn valid_record_envelope(record: &GroupAgentScheduledNodeProviderRequestRecord) -> bool {
    valid_envelope(
        &record.provider_request_id,
        &record.prepared_request_sha256,
        record.expected_sha256().as_deref(),
        record.codec_protocol_version,
        record.expected_last_event_seq,
        &record.expected_last_event_sha256,
        record_flags(record),
    ) && i64::try_from(record.created_at_ms).is_ok()
}

fn valid_prepare_envelope(request: &PrepareGroupAgentScheduledNodeProviderRequest) -> bool {
    valid_envelope(
        &request.provider_request_id,
        &request.prepared_request_sha256,
        request.expected_sha256().as_deref(),
        request.codec_protocol_version,
        request.expected_last_event_seq,
        &request.expected_last_event_sha256,
        prepare_flags(request),
    )
}

#[allow(clippy::too_many_arguments)]
fn valid_envelope(
    id: &str,
    sha256: &str,
    expected_sha256: Result<&str, &GroupAgentScheduledNodeProviderRequestValidationError>,
    codec_version: u16,
    expected_seq: u64,
    expected_head: &str,
    flags: [bool; 8],
) -> bool {
    content_id(id, "scheduled-node-provider-request-", sha256)
        && expected_sha256 == Ok(sha256)
        && codec_version == GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION
        && expected_seq == 1
        && digest(expected_head)
        && flags_are_passive(flags)
}

fn record_flags(record: &GroupAgentScheduledNodeProviderRequestRecord) -> [bool; 8] {
    [
        record.provider_request_prepared,
        record.provider_request_sent,
        record.lifecycle_contract_admitted,
        record.execution_authority_released,
        record.dispatch_authority_released,
        record.project_lane_claimed,
        record.progress_observed,
        record.successor_advance_authorized,
    ]
}

fn prepare_flags(request: &PrepareGroupAgentScheduledNodeProviderRequest) -> [bool; 8] {
    [
        request.provider_request_prepared,
        request.provider_request_sent,
        request.lifecycle_contract_admitted,
        request.execution_authority_released,
        request.dispatch_authority_released,
        request.project_lane_claimed,
        request.progress_observed,
        request.successor_advance_authorized,
    ]
}

pub(super) fn validate_inspection(
    inspection: &GroupAgentScheduledNodeProviderRequestInspection,
) -> Result<(), GroupAgentScheduledNodeProviderRequestValidationError> {
    inspection.record.validate()?;
    inspection
        .scheduled_contract
        .validate()
        .map_err(|error| invalid(&error.to_string()))?;
    let record = &inspection.record;
    let source = &inspection.scheduled_contract;
    let candidate = &source.candidate;
    let valid = inspection.v == GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION
        && record.provider_request_bytes == inspection.provider_request_body.len()
        && record.provider_request_sha256
            == group_agent_node_provider_request_sha256(&inspection.provider_request_body)
        && record.scheduled_contract_id == source.record.contract_id
        && record.scheduled_contract_sha256 == source.record.contract_sha256
        && record.graph_run_id == source.record.graph_run_id
        && record.schedule_id == source.record.schedule_id
        && record.schedule_sha256 == source.record.schedule_sha256
        && record.execution_ordinal == source.record.execution_ordinal
        && record.node_id == source.record.node_id
        && record.attempt == source.record.attempt
        && record.logical_request_id == source.record.request_id
        && record.logical_request_sha256 == source.record.request_sha256
        && record.project_lane_sha256 == source.record.project_lane_sha256
        && record.expected_last_event_seq == source.record.expected_last_event_seq
        && record.expected_last_event_sha256 == source.record.expected_last_event_sha256
        && record.provider == candidate.provider.kind
        && record.endpoint == candidate.provider.endpoint
        && record.model == candidate.provider.model
        && record.pricing_snapshot_sha256 == candidate.budgets.pricing_snapshot_sha256
        && !candidate.provider_request_present
        && !candidate.lifecycle_contract_admitted
        && !candidate.execution_authority_released
        && !candidate.dispatch_authority_released
        && !candidate.progress_observed
        && !candidate.successor_advance_authorized;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled-node provider request inspection bindings disagree"))
}

const fn flags_are_passive(flags: [bool; 8]) -> bool {
    flags[0]
        && !flags[1]
        && !flags[2]
        && !flags[3]
        && !flags[4]
        && !flags[5]
        && !flags[6]
        && !flags[7]
}

fn validate_provider(
    kind: crate::GroupAgentNodeProviderKind,
    endpoint: &str,
    model: &str,
) -> Result<(), GroupAgentScheduledNodeProviderRequestValidationError> {
    super::super::validation::validate_provider(&GroupAgentNodeExecutionProvider {
        kind,
        endpoint: endpoint.into(),
        model: model.into(),
        store: false,
        stream: true,
    })
    .map_err(|error| invalid(&error.message))
}

fn content_id(value: &str, prefix: &str, sha256: &str) -> bool {
    digest(sha256) && valid_identifier(value) && value == format!("{prefix}{sha256}")
}

fn valid_identifier(value: &str) -> bool {
    super::super::validation::valid_identifier(value)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    super::super::validation::valid_text(value, maximum)
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: &str) -> GroupAgentScheduledNodeProviderRequestValidationError {
    GroupAgentScheduledNodeProviderRequestValidationError {
        message: message.into(),
    }
}
