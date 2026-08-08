use std::collections::BTreeSet;

use crate::runtime_domain::{
    Cancellation, GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION, GroupAgentGraphRunInspection,
    GroupAgentGraphRunStatus, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledNodeProviderRequestRecord, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_LIST_LIMIT, Message, ModelRequest,
    PrepareGroupAgentScheduledNodeProviderRequest,
    PrepareGroupAgentScheduledNodeProviderRequestDisposition,
    PrepareGroupAgentScheduledNodeProviderRequestResult, group_agent_node_destination_sha256,
    group_agent_node_provider_request_sha256, group_agent_scheduled_node_provider_request_id,
};

use super::{
    GroupAgentScheduledNodeProviderRequestServiceError,
    PrepareGroupAgentScheduledNodeProviderRequestInput,
    scheduled_provider_request_error::{corrupt, invalid},
};

pub(super) fn validate_input(
    input: &PrepareGroupAgentScheduledNodeProviderRequestInput,
) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
    super::scheduled_contract_validation::validate_identifier(
        &input.scheduled_contract_id,
        "scheduled contract ID",
    )
    .map_err(GroupAgentScheduledNodeProviderRequestServiceError::from)?;
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

pub(super) fn model_request(candidate: &GroupAgentScheduledNodeContractCandidate) -> ModelRequest {
    ModelRequest {
        system_prompt: candidate.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: candidate.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: candidate.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    }
}

pub(super) fn prepare_request(
    input: &PrepareGroupAgentScheduledNodeProviderRequestInput,
    source: &GroupAgentScheduledNodeContractInspection,
    body: Vec<u8>,
) -> Result<
    PrepareGroupAgentScheduledNodeProviderRequest,
    GroupAgentScheduledNodeProviderRequestServiceError,
> {
    let candidate = &source.candidate;
    if source.record.contract_id != input.scheduled_contract_id {
        return Err(invalid("scheduled contract identity disagrees"));
    }
    finalize_request(request_candidate(input, candidate, body))
}

fn request_candidate(
    input: &PrepareGroupAgentScheduledNodeProviderRequestInput,
    candidate: &GroupAgentScheduledNodeContractCandidate,
    body: Vec<u8>,
) -> PrepareGroupAgentScheduledNodeProviderRequest {
    PrepareGroupAgentScheduledNodeProviderRequest {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        provider_request_id: String::new(),
        graph_run_id: candidate.graph_run_id.clone(),
        schedule_id: candidate.schedule_id.clone(),
        scheduled_contract_id: candidate.contract_id.clone(),
        execution_ordinal: candidate.node.execution_ordinal,
        node_id: candidate.node.node_id.clone(),
        attempt: candidate.node.attempt,
        scheduled_contract_sha256: candidate.contract_sha256.clone(),
        logical_request_id: candidate.request.request_id.clone(),
        logical_request_sha256: candidate.request.request_sha256.clone(),
        schedule_sha256: candidate.schedule_sha256.clone(),
        project_lane_sha256: candidate.node.project_lane_sha256.clone(),
        provider: candidate.provider.kind,
        endpoint: candidate.provider.endpoint.clone(),
        model: candidate.provider.model.clone(),
        destination_sha256: group_agent_node_destination_sha256(
            candidate.provider.kind,
            &candidate.provider.endpoint,
            &candidate.provider.model,
        ),
        pricing_snapshot_sha256: candidate.budgets.pricing_snapshot_sha256.clone(),
        provider_request_sha256: group_agent_node_provider_request_sha256(&body),
        provider_request_body: body,
        prepared_request_sha256: String::new(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: candidate.expected_last_event_seq,
        expected_last_event_sha256: candidate.expected_last_event_sha256.clone(),
        provider_request_prepared: true,
        provider_request_sent: false,
        lifecycle_contract_admitted: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        project_lane_claimed: false,
        progress_observed: false,
        successor_advance_authorized: false,
        idempotency_key: input.idempotency_key.clone(),
        prepared_at_ms: input.prepared_at_ms,
    }
}

fn finalize_request(
    mut request: PrepareGroupAgentScheduledNodeProviderRequest,
) -> Result<
    PrepareGroupAgentScheduledNodeProviderRequest,
    GroupAgentScheduledNodeProviderRequestServiceError,
> {
    request.prepared_request_sha256 = request
        .expected_sha256()
        .map_err(|error| invalid(&error.to_string()))?;
    request.provider_request_id =
        group_agent_scheduled_node_provider_request_id(&request.prepared_request_sha256);
    request
        .validate()
        .map_err(|error| invalid(&error.to_string()))?;
    Ok(request)
}

pub(super) fn validate_pristine_run(
    graph_run_id: &str,
    inspection: GroupAgentGraphRunInspection,
) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
    let inspection = super::validation::checked_run(inspection).map_err(|error| match error {
        super::GroupAgentNodeExecutionContractServiceError::InvalidInput { message } => {
            GroupAgentScheduledNodeProviderRequestServiceError::InvalidInput { message }
        }
        super::GroupAgentNodeExecutionContractServiceError::Conflict { message } => {
            GroupAgentScheduledNodeProviderRequestServiceError::Conflict { message }
        }
        super::GroupAgentNodeExecutionContractServiceError::NotFound { message } => {
            GroupAgentScheduledNodeProviderRequestServiceError::NotFound { message }
        }
        super::GroupAgentNodeExecutionContractServiceError::Unavailable { message } => {
            GroupAgentScheduledNodeProviderRequestServiceError::Unavailable { message }
        }
        super::GroupAgentNodeExecutionContractServiceError::Corrupt { message } => {
            GroupAgentScheduledNodeProviderRequestServiceError::Corrupt { message }
        }
    })?;
    let run = &inspection.run;
    let pristine = inspection.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && run.v == GROUP_AGENT_GRAPH_RUN_VERSION
        && run.graph_run_id == graph_run_id
        && run.status == GroupAgentGraphRunStatus::AwaitingExecutionContract
        && !run.execution_contract_present
        && !run.dispatch_request_present
        && !run.dispatch_authority_released
        && run.last_event_seq == 1
        && inspection.events.len() == 1
        && inspection.event_jsons.len() == 1;
    pristine
        .then_some(())
        .ok_or_else(|| corrupt("scheduled provider request requires the exact pristine v1 Run"))
}

pub(super) fn validate_result(
    request: &PrepareGroupAgentScheduledNodeProviderRequest,
    source: &GroupAgentScheduledNodeContractInspection,
    result: PrepareGroupAgentScheduledNodeProviderRequestResult,
) -> Result<
    PrepareGroupAgentScheduledNodeProviderRequestResult,
    GroupAgentScheduledNodeProviderRequestServiceError,
> {
    if result.v != GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION {
        return Err(corrupt(
            "store returned an unsupported scheduled provider request version",
        ));
    }
    let inspection = checked_inspection(result.inspection)?;
    let record = &inspection.record;
    let same = record.provider_request_id == request.provider_request_id
        && record.graph_run_id == request.graph_run_id
        && record.schedule_id == request.schedule_id
        && record.scheduled_contract_id == request.scheduled_contract_id
        && record.prepared_request_sha256 == request.prepared_request_sha256
        && record.provider_request_sha256 == request.provider_request_sha256
        && inspection.provider_request_body == request.provider_request_body
        && inspection.scheduled_contract == *source;
    let created = result.disposition
        != PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created
        || record.created_at_ms == request.prepared_at_ms;
    if !same || !created {
        return Err(corrupt(
            "store returned different scheduled provider request semantics",
        ));
    }
    Ok(PrepareGroupAgentScheduledNodeProviderRequestResult {
        v: result.v,
        disposition: result.disposition,
        inspection,
    })
}

pub(super) fn checked_inspection(
    inspection: GroupAgentScheduledNodeProviderRequestInspection,
) -> Result<
    GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledNodeProviderRequestServiceError,
> {
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

pub(super) fn validate_list_input(
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
    if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_LIST_LIMIT).contains(&limit) {
        return Err(invalid(
            "scheduled provider request list limit is outside its bounds",
        ));
    }
    if let Some(id) = graph_run_id {
        super::scheduled_contract_validation::validate_identifier(id, "Graph Run ID")
            .map_err(GroupAgentScheduledNodeProviderRequestServiceError::from)?;
    }
    Ok(())
}

pub(super) fn validate_list(
    records: &[GroupAgentScheduledNodeProviderRequestRecord],
    graph_run_id: Option<&str>,
    limit: usize,
) -> Result<(), GroupAgentScheduledNodeProviderRequestServiceError> {
    if records.len() > limit {
        return Err(corrupt(
            "store returned more scheduled provider requests than requested",
        ));
    }
    let mut ids = BTreeSet::new();
    let mut contracts = BTreeSet::new();
    let mut logical_requests = BTreeSet::new();
    let mut run_slots = BTreeSet::new();
    let mut schedule_slots = BTreeSet::new();
    for record in records {
        record
            .validate()
            .map_err(|error| corrupt(&error.to_string()))?;
        if graph_run_id.is_some_and(|id| id != record.graph_run_id)
            || !ids.insert(record.provider_request_id.as_str())
            || !contracts.insert(record.scheduled_contract_id.as_str())
            || !logical_requests.insert(record.logical_request_id.as_str())
            || !run_slots.insert((
                record.graph_run_id.as_str(),
                record.node_id.as_str(),
                record.attempt,
            ))
            || !schedule_slots.insert((
                record.schedule_id.as_str(),
                record.execution_ordinal,
                record.attempt,
            ))
        {
            return Err(corrupt(
                "store returned unfiltered or duplicate scheduled provider request metadata",
            ));
        }
    }
    Ok(())
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}
