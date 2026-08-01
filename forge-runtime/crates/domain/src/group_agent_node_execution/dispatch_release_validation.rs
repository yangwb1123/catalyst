use super::{
    GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchConsentRequirement, GroupAgentNodeDispatchCredentialPreflight,
    GroupAgentNodeDispatchDestinationPreflight, GroupAgentNodeDispatchPricingPreflight,
    GroupAgentNodeDispatchProjectLaneClaim, GroupAgentNodeDispatchProviderHealthCheck,
    GroupAgentNodeDispatchReleaseControl, GroupAgentNodeDispatchReleaseValidationError,
    MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES,
    MAX_GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
    group_agent_node_dispatch_authorization_id,
};
use crate::{
    GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION, GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphControlSnapshot, GroupAgentGraphRunInspection, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchRequestInspection, GroupAgentNodeExecutionContractInspection,
    GroupAgentNodeExecutionFailurePolicy, GroupAgentNodeExecutionProvider,
    GroupAgentNodeFailurePropagationOwner, GroupAgentNodePostClaimUncertainty,
    GroupAgentNodeProviderKind, GroupAgentNodeSameProjectPolicy,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_NODE_COST_USD_MICROS,
    MAX_GROUP_AGENT_NODE_MODEL_EVENTS, MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES,
    MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS, MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES,
    MAX_GROUP_AGENT_NODE_TIMEOUT_MS, group_agent_node_destination_sha256,
    group_agent_node_dispatch_request_id, group_agent_node_provider_request_sha256,
    group_agent_project_lane_sha256,
};

pub(super) fn validate_release_control(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    validate_release_header(control)?;
    let run = run_inspection(control)?;
    validate_manifest_bindings(control)?;
    let contract = contract_inspection(control, &run)?;
    request_inspection(control, &contract)?
        .validate()
        .map_err(|error| invalid(&error.message))?;
    validate_original_control(control)?;
    validate_release_digest(control)
}

fn validate_release_header(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    let run = &control.graph_run;
    let valid = control.v == GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION
        && control.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && control.release_control_protocol_version
            == GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION
        && run.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION
        && run.status == GroupAgentGraphRunStatus::AwaitingDispatchAuthorization
        && run.execution_contract_present
        && run.dispatch_request_present
        && !run.dispatch_authority_released
        && run.last_event_seq == 3
        && control.journal_events.len() == 3
        && is_digest(&control.snapshot_sha256);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Dispatch Release Control header"))
}

fn run_inspection(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<GroupAgentGraphRunInspection, GroupAgentNodeDispatchReleaseValidationError> {
    let plan_json = control
        .plan
        .canonical_json()
        .map_err(|error| invalid(&error.message))?;
    let event_jsons = control
        .journal_events
        .iter()
        .map(|event| {
            event
                .canonical_json()
                .map_err(|error| invalid(&error.message))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let inspection = GroupAgentGraphRunInspection {
        v: control.graph_run.v,
        run: control.graph_run.clone(),
        plan_json,
        plan: control.plan.clone(),
        event_jsons,
        events: control.journal_events.clone(),
    };
    inspection
        .validate()
        .map_err(|error| invalid(&error.message))?;
    Ok(inspection)
}

fn validate_manifest_bindings(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    control
        .manifest
        .validate()
        .map_err(|error| invalid(&error.message))?;
    let authored = control
        .manifest
        .nodes
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<Vec<_>>();
    let planned = control
        .plan
        .authored_node_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let valid = control.manifest.expected_sha256().as_deref()
        == Ok(control.graph_run.graph_manifest_sha256.as_str())
        && control.manifest.source.snapshot_sha256 == control.graph_run.source_snapshot_sha256
        && authored == planned
        && control.manifest.edges == control.plan.edges
        && control.manifest.waves == control.plan.waves;
    valid
        .then_some(())
        .ok_or_else(|| invalid("release-control manifest, plan, and Graph Run disagree"))
}

fn contract_inspection(
    control: &GroupAgentNodeDispatchReleaseControl,
    run: &GroupAgentGraphRunInspection,
) -> Result<GroupAgentNodeExecutionContractInspection, GroupAgentNodeDispatchReleaseValidationError>
{
    let admission_event = control
        .journal_events
        .get(1)
        .ok_or_else(|| invalid("release-control journal has no seq-2 contract event"))?;
    let inspection = GroupAgentNodeExecutionContractInspection {
        v: control.contract_record.v,
        record: control.contract_record.clone(),
        contract_json: control
            .contract
            .canonical_json()
            .map_err(|error| invalid(&error.message))?,
        contract: control.contract.clone(),
        admission_event_json: admission_event
            .canonical_json()
            .map_err(|error| invalid(&error.message))?,
        admission_event: admission_event.clone(),
        graph_run: run.clone(),
    };
    inspection
        .validate()
        .map_err(|error| invalid(&error.message))?;
    Ok(inspection)
}

fn request_inspection(
    control: &GroupAgentNodeDispatchReleaseControl,
    contract: &GroupAgentNodeExecutionContractInspection,
) -> Result<GroupAgentNodeDispatchRequestInspection, GroupAgentNodeDispatchReleaseValidationError> {
    let event = control
        .journal_events
        .get(2)
        .ok_or_else(|| invalid("release-control journal has no seq-3 request event"))?;
    let body = control.provider_request_json.as_bytes();
    if control.dispatch_request.provider_request_bytes != body.len()
        || control.dispatch_request.provider_request_sha256
            != group_agent_node_provider_request_sha256(body)
    {
        return Err(invalid(
            "release-control exact provider request bytes disagree",
        ));
    }
    Ok(GroupAgentNodeDispatchRequestInspection {
        v: control.dispatch_request.v,
        record: control.dispatch_request.clone(),
        provider_request_body: body.to_vec(),
        preparation_event_json: event
            .canonical_json()
            .map_err(|error| invalid(&error.message))?,
        preparation_event: event.clone(),
        contract: contract.clone(),
    })
}

fn validate_original_control(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    let head = control
        .journal_events
        .first()
        .ok_or_else(|| invalid("release-control journal has no preparation event"))?
        .expected_sha256()
        .map_err(|error| invalid(&error.message))?;
    let snapshot = GroupAgentGraphControlSnapshot {
        v: GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        graph_run_version: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: control.graph_run.graph_run_id.clone(),
        graph_id: control.graph_run.graph_id.clone(),
        source_snapshot_sha256: control.graph_run.source_snapshot_sha256.clone(),
        graph_manifest_sha256: control.graph_run.graph_manifest_sha256.clone(),
        core_plan_sha256: control.graph_run.plan_sha256.clone(),
        last_event_seq: 1,
        last_event_sha256: head,
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan: control.plan.clone(),
        manifest: control.manifest.clone(),
        snapshot_sha256: control.contract.control_snapshot_sha256.clone(),
    };
    control
        .contract
        .validate_against_control(&snapshot)
        .map_err(|error| invalid(&error.message))
}

fn validate_release_digest(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    if control.expected_sha256()? != control.snapshot_sha256 {
        return Err(invalid("release-control snapshot digest disagrees"));
    }
    let bytes = control.canonical_json()?.len();
    (1..=MAX_GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_BYTES)
        .contains(&bytes)
        .then_some(())
        .ok_or_else(|| invalid("release-control snapshot exceeds its byte bound"))
}

pub(super) fn validate_authorization(
    authorization: &GroupAgentNodeDispatchAuthorization,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    validate_authorization_header(authorization)?;
    validate_authorization_policy(authorization)?;
    if authorization.expected_sha256()? != authorization.authorization_sha256
        || authorization.authorization_id
            != group_agent_node_dispatch_authorization_id(&authorization.authorization_sha256)
    {
        return Err(invalid(
            "dispatch authorization digest or identity disagrees",
        ));
    }
    let bytes = authorization.canonical_json()?.len();
    (1..=MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES)
        .contains(&bytes)
        .then_some(())
        .ok_or_else(|| invalid("dispatch authorization exceeds its byte bound"))
}

fn validate_authorization_header(
    value: &GroupAgentNodeDispatchAuthorization,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    let identifiers = [
        &value.graph_run_id,
        &value.graph_id,
        &value.group_run_id,
        &value.node_id,
        &value.project_id,
    ];
    let digests = [
        &value.source_snapshot_sha256,
        &value.graph_manifest_sha256,
        &value.core_plan_sha256,
        &value.release_control_snapshot_sha256,
        &value.expected_last_event_sha256,
        &value.contract_sha256,
        &value.dispatch_request_sha256,
        &value.logical_request_sha256,
        &value.request_body_sha256,
        &value.project_lane_sha256,
        &value.destination_sha256,
        &value.pricing_snapshot_sha256,
        &value.authorization_sha256,
    ];
    let valid = value.v == GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION
        && value.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && value.dispatch_authorization_protocol_version
            == GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION
        && identifiers.into_iter().all(|item| valid_identifier(item))
        && digests.into_iter().all(|item| is_digest(item))
        && value.contract_id == format!("node-contract-{}", value.contract_sha256)
        && value.dispatch_request_id
            == group_agent_node_dispatch_request_id(&value.dispatch_request_sha256)
        && value.expected_last_event_seq == 3
        && value.attempt == 1
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES).contains(&value.request_body_bytes)
        && valid_authorization_provider(value)
        && value.project_lane_sha256 == group_agent_project_lane_sha256(&value.project_id)
        && value.destination_sha256
            == group_agent_node_destination_sha256(
                value.provider_kind,
                &value.endpoint,
                &value.model,
            );
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Dispatch Authorization header"))
}

fn valid_authorization_provider(value: &GroupAgentNodeDispatchAuthorization) -> bool {
    super::super::validation::validate_provider(&GroupAgentNodeExecutionProvider {
        kind: value.provider_kind,
        endpoint: value.endpoint.clone(),
        model: value.model.clone(),
        store: false,
        stream: true,
    })
    .is_ok()
}

fn validate_authorization_policy(
    value: &GroupAgentNodeDispatchAuthorization,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    let requirements = &value.release_requirements;
    let valid = value.same_project_policy
        == GroupAgentNodeSameProjectPolicy::ExclusiveUntilTerminal
        && value.provider_kind == GroupAgentNodeProviderKind::OpenAiResponses
        && valid_budgets(&value.budgets)
        && value.pricing_snapshot_sha256 == value.budgets.pricing_snapshot_sha256
        && valid_failure(&value.failure)
        && requirements.consent == GroupAgentNodeDispatchConsentRequirement::FreshOffMachine
        && requirements.consent_contract_version
            == GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION
        && requirements.credential_preflight
            == GroupAgentNodeDispatchCredentialPreflight::HeaderSafeEnvironment
        && requirements.destination_preflight
            == GroupAgentNodeDispatchDestinationPreflight::ExactRegisteredDestination
        && requirements.pricing_preflight
            == GroupAgentNodeDispatchPricingPreflight::ExactSnapshotWithinMaxCost
        && requirements.project_lane_claim
            == GroupAgentNodeDispatchProjectLaneClaim::GlobalExclusiveUntilTerminal
        && requirements.provider_health_check
            == GroupAgentNodeDispatchProviderHealthCheck::Forbidden
        && value.execution_contract_present
        && value.dispatch_request_present
        && value.dispatch_authority_release_authorized
        && !value.dispatch_authority_released;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Dispatch Authorization release policy"))
}

fn valid_budgets(value: &super::GroupAgentNodeExecutionBudgets) -> bool {
    value.max_turns == 1
        && value.max_tool_calls == 0
        && (1..=MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS).contains(&value.max_output_tokens)
        && (1..=MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES).contains(&value.max_model_output_bytes)
        && (1..=MAX_GROUP_AGENT_NODE_MODEL_EVENTS).contains(&value.max_model_events)
        && (1..=MAX_GROUP_AGENT_NODE_TIMEOUT_MS).contains(&value.timeout_ms)
        && (1..=MAX_GROUP_AGENT_NODE_COST_USD_MICROS).contains(&value.max_cost_usd_micros)
        && is_digest(&value.pricing_snapshot_sha256)
}

fn valid_failure(value: &GroupAgentNodeExecutionFailurePolicy) -> bool {
    !value.automatic_retry
        && !value.lease_retry
        && value.post_claim_uncertainty == GroupAgentNodePostClaimUncertainty::DispatchUnknown
        && value.failure_propagation_owner == GroupAgentNodeFailurePropagationOwner::ForgeCore
}

pub(super) fn validate_authorization_against_release_control(
    authorization: &GroupAgentNodeDispatchAuthorization,
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    control.validate()?;
    authorization.validate()?;
    let event_head = control.journal_events[2]
        .expected_sha256()
        .map_err(|error| invalid(&error.message))?;
    let run = &control.graph_run;
    let contract = &control.contract;
    let request = &control.dispatch_request;
    let valid = authorization.graph_run_id == run.graph_run_id
        && authorization.graph_id == run.graph_id
        && authorization.group_run_id == control.manifest.source.group_run_id
        && authorization.source_snapshot_sha256 == run.source_snapshot_sha256
        && authorization.graph_manifest_sha256 == run.graph_manifest_sha256
        && authorization.core_plan_sha256 == run.plan_sha256
        && authorization.release_control_snapshot_sha256 == control.snapshot_sha256
        && authorization.expected_last_event_seq == run.last_event_seq
        && authorization.expected_last_event_sha256 == event_head
        && authorization.contract_id == contract.contract_id
        && authorization.contract_sha256 == contract.contract_sha256
        && authorization.dispatch_request_id == request.dispatch_request_id
        && authorization.dispatch_request_sha256 == request.dispatch_request_sha256
        && authorization.logical_request_sha256 == request.request_sha256
        && authorization.request_body_sha256 == request.provider_request_sha256
        && authorization.request_body_bytes == request.provider_request_bytes;
    if !valid {
        return Err(invalid("dispatch authorization source bindings disagree"));
    }
    validate_authorization_execution_bindings(authorization, control)
}

fn validate_authorization_execution_bindings(
    authorization: &GroupAgentNodeDispatchAuthorization,
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
    let contract = &control.contract;
    let request = &control.dispatch_request;
    let valid = authorization.node_id == contract.node.node_id
        && authorization.attempt == contract.node.attempt
        && authorization.project_id == contract.node.project_id
        && authorization.project_lane_sha256 == contract.node.project_lane_sha256
        && authorization.same_project_policy == contract.node.same_project_policy
        && authorization.provider_kind == contract.provider.kind
        && authorization.endpoint == contract.provider.endpoint
        && authorization.model == contract.provider.model
        && authorization.destination_sha256 == request.destination_sha256
        && authorization.pricing_snapshot_sha256 == request.pricing_snapshot_sha256
        && authorization.budgets == contract.budgets
        && authorization.failure == contract.failure
        && authorization.execution_contract_present == control.graph_run.execution_contract_present
        && authorization.dispatch_request_present == control.graph_run.dispatch_request_present
        && authorization.dispatch_authority_released
            == control.graph_run.dispatch_authority_released;
    valid
        .then_some(())
        .ok_or_else(|| invalid("dispatch authorization execution bindings disagree"))
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: &str) -> GroupAgentNodeDispatchReleaseValidationError {
    GroupAgentNodeDispatchReleaseValidationError {
        message: message.into(),
    }
}
