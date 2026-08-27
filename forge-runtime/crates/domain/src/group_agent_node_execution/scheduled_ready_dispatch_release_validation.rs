use super::{
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
    GroupAgentScheduledReadyNodeDispatchAtomicTransitionRequirement,
    GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeDispatchReleaseControl,
    GroupAgentScheduledReadyNodeDispatchReleaseValidationError,
    GroupAgentScheduledReadyNodeSuccessorRequirement,
    MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
    group_agent_scheduled_ready_node_dispatch_authorization_id,
};
use crate::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentNodeSameProjectPolicy,
    GroupAgentScheduledNodeDispatchConsentRequirement,
    GroupAgentScheduledNodeDispatchCredentialPreflight,
    GroupAgentScheduledNodeDispatchDestinationPreflight,
    GroupAgentScheduledNodeDispatchPricingPreflight,
    GroupAgentScheduledNodeDispatchProjectLaneClaim,
    GroupAgentScheduledNodeDispatchProviderHealthCheck,
    MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES, group_agent_node_destination_sha256,
    group_agent_project_lane_sha256,
};

#[path = "scheduled_ready_dispatch_release_source_validation.rs"]
mod source_validation;

pub(super) fn decode_release_control(
    bytes: &[u8],
) -> Result<
    GroupAgentScheduledReadyNodeDispatchReleaseControl,
    GroupAgentScheduledReadyNodeDispatchReleaseValidationError,
> {
    bounded(
        bytes.len(),
        MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
        "ready release control input is outside its byte bound",
    )?;
    let value: GroupAgentScheduledReadyNodeDispatchReleaseControl =
        serde_json::from_slice(bytes)
            .map_err(|_| invalid("ready release control is invalid JSON"))?;
    value.validate()?;
    if value.canonical_json()?.as_bytes() != bytes {
        return Err(invalid("ready release control is not exact canonical JSON"));
    }
    Ok(value)
}

pub(super) fn decode_authorization(
    bytes: &[u8],
) -> Result<
    GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeDispatchReleaseValidationError,
> {
    bounded(
        bytes.len(),
        MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES,
        "ready authorization input is outside its byte bound",
    )?;
    let value: GroupAgentScheduledReadyNodeDispatchAuthorization = serde_json::from_slice(bytes)
        .map_err(|_| invalid("ready authorization is invalid JSON"))?;
    value.validate()?;
    if value.canonical_json()?.as_bytes() != bytes {
        return Err(invalid("ready authorization is not exact canonical JSON"));
    }
    Ok(value)
}

pub(super) fn validate_release_control(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let header = control.v == GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION
        && control.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && control.release_control_protocol_version
            == GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION
        && digest(&control.snapshot_sha256);
    if !header {
        return Err(invalid("invalid ready release-control header"));
    }
    source_validation::validate_sources(control)?;
    if control.expected_sha256()? != control.snapshot_sha256 {
        return Err(invalid("ready release-control digest disagrees"));
    }
    bounded(
        control.canonical_json()?.len(),
        MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES,
        "ready release control exceeds its byte bound",
    )
}

pub(super) fn validate_authorization(
    value: &GroupAgentScheduledReadyNodeDispatchAuthorization,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    validate_authorization_header(value)?;
    validate_authorization_policy(value)?;
    if value.expected_sha256()? != value.authorization_sha256
        || value.authorization_id
            != group_agent_scheduled_ready_node_dispatch_authorization_id(
                &value.authorization_sha256,
            )
    {
        return Err(invalid("ready authorization identity disagrees"));
    }
    bounded(
        value.canonical_json()?.len(),
        MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES,
        "ready authorization exceeds its byte bound",
    )
}

fn validate_authorization_header(
    value: &GroupAgentScheduledReadyNodeDispatchAuthorization,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let identifiers = [
        &value.graph_run_id,
        &value.graph_id,
        &value.group_run_id,
        &value.group_id,
        &value.node_id,
        &value.project_id,
    ];
    let valid = value.v == GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_VERSION
        && value.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && value.dispatch_authorization_protocol_version
            == GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION
        && identifiers.into_iter().all(|item| valid_identifier(item))
        && authorization_digests(value).into_iter().all(digest)
        && valid_authorization_content_ids(value)
        && value.expected_last_event_seq == 1
        && value.execution_ordinal <= 31
        && value.attempt == 1
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES).contains(&value.request_body_bytes)
        && value.project_lane_sha256 == group_agent_project_lane_sha256(&value.project_id)
        && value.destination_sha256
            == group_agent_node_destination_sha256(
                value.provider_kind,
                &value.endpoint,
                &value.model,
            );
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid ready authorization header"))?;
    validate_shared_execution_policy(value)
}

fn valid_authorization_content_ids(
    value: &GroupAgentScheduledReadyNodeDispatchAuthorization,
) -> bool {
    content_id(
        &value.schedule_id,
        "graph-execution-schedule-",
        &value.schedule_sha256,
    ) && content_id(
        &value.scheduled_contract_id,
        "scheduled-node-contract-",
        &value.scheduled_contract_sha256,
    ) && content_id(
        &value.scheduled_provider_request_id,
        "scheduled-node-provider-request-",
        &value.scheduled_provider_request_sha256,
    ) && content_id(
        &value.logical_request_id,
        "scheduled-node-request-",
        &value.logical_request_sha256,
    )
}

fn authorization_digests(value: &GroupAgentScheduledReadyNodeDispatchAuthorization) -> [&str; 17] {
    [
        &value.source_snapshot_sha256,
        &value.graph_manifest_sha256,
        &value.core_plan_sha256,
        &value.control_snapshot_sha256,
        &value.release_control_snapshot_sha256,
        &value.progress_snapshot_sha256,
        &value.reconcile_decision_sha256,
        &value.schedule_sha256,
        &value.scheduled_contract_sha256,
        &value.scheduled_provider_request_sha256,
        &value.logical_request_sha256,
        &value.request_body_sha256,
        &value.expected_last_event_sha256,
        &value.project_lane_sha256,
        &value.destination_sha256,
        &value.pricing_snapshot_sha256,
        &value.authorization_sha256,
    ]
}

fn validate_shared_execution_policy(
    value: &GroupAgentScheduledReadyNodeDispatchAuthorization,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let provider = crate::GroupAgentNodeExecutionProvider {
        kind: value.provider_kind,
        endpoint: value.endpoint.clone(),
        model: value.model.clone(),
        store: false,
        stream: true,
    };
    super::super::validation::validate_provider(&provider)
        .map_err(|_| invalid("ready authorization provider policy is invalid"))?;
    super::super::validation::validate_budgets(&value.budgets)
        .map_err(|_| invalid("ready authorization budgets are invalid"))?;
    super::super::validation::validate_failure(&value.failure)
        .map_err(|_| invalid("ready authorization failure policy is invalid"))?;
    let valid = value.same_project_policy
        == GroupAgentNodeSameProjectPolicy::ExclusiveUntilTerminal
        && value.pricing_snapshot_sha256 == value.budgets.pricing_snapshot_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("ready authorization execution policy disagrees"))
}

fn validate_authorization_policy(
    value: &GroupAgentScheduledReadyNodeDispatchAuthorization,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let requirement = &value.release_requirements;
    let valid = requirement.consent == GroupAgentScheduledNodeDispatchConsentRequirement::FreshOffMachine
        && requirement.consent_contract_version
            == GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_CONSENT_CONTRACT_VERSION
        && requirement.credential_preflight
            == GroupAgentScheduledNodeDispatchCredentialPreflight::HeaderSafeEnvironment
        && requirement.destination_preflight
            == GroupAgentScheduledNodeDispatchDestinationPreflight::ExactRegisteredDestination
        && requirement.pricing_preflight
            == GroupAgentScheduledNodeDispatchPricingPreflight::ExactSnapshotWithinMaxCost
        && requirement.project_lane_claim
            == GroupAgentScheduledNodeDispatchProjectLaneClaim::GlobalExclusiveUntilTerminal
        && requirement.provider_health_check
            == GroupAgentScheduledNodeDispatchProviderHealthCheck::Forbidden
        && requirement.atomic_transition
            == GroupAgentScheduledReadyNodeDispatchAtomicTransitionRequirement::ExactProgressSnapshotSelectedNodeAdmissionReleaseAndLaneClaim
        && requirement.successor
            == GroupAgentScheduledReadyNodeSuccessorRequirement::ExactOrderedDirectPredecessorTerminalReceiptsBeforeSuccessor
        && value.maximum_future_node_releases == 1
        && authorized_flags(value)
        && current_flags_false(value);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid passive ready release policy"))
}

fn authorized_flags(value: &GroupAgentScheduledReadyNodeDispatchAuthorization) -> bool {
    value.lifecycle_contract_admission_authorized
        && value.execution_authority_release_authorized
        && value.dispatch_authority_release_authorized
        && value.scheduled_contract_candidate_present
        && value.provider_request_prepared
}

fn current_flags_false(value: &GroupAgentScheduledReadyNodeDispatchAuthorization) -> bool {
    !value.lifecycle_contract_admitted
        && !value.execution_authority_released
        && !value.dispatch_authority_released
        && !value.project_lane_claimed
        && !value.provider_request_sent
        && !value.progress_observed
        && !value.terminal_receipt_recorded
        && !value.successor_advance_authorized
}

pub(super) fn validate_authorization_against_release_control(
    authorization: &GroupAgentScheduledReadyNodeDispatchAuthorization,
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    control.validate()?;
    authorization.validate()?;
    validate_authorization_source_bindings(authorization, control)?;
    validate_authorization_execution_bindings(authorization, control)
}

fn validate_authorization_source_bindings(
    value: &GroupAgentScheduledReadyNodeDispatchAuthorization,
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let source = &control.control_snapshot;
    let request = &control.provider_request;
    let valid = value.graph_run_id == source.graph_run_id
        && value.graph_id == source.graph_id
        && value.group_run_id == source.manifest.source.group_run_id
        && value.group_id == source.manifest.source.group_id
        && value.source_snapshot_sha256 == source.source_snapshot_sha256
        && value.graph_manifest_sha256 == source.graph_manifest_sha256
        && value.core_plan_sha256 == source.core_plan_sha256
        && value.control_snapshot_sha256 == source.snapshot_sha256
        && value.release_control_snapshot_sha256 == control.snapshot_sha256
        && value.progress_snapshot_sha256 == control.progress_snapshot.snapshot_sha256
        && value.reconcile_decision_sha256 == control.reconcile_decision.decision_sha256
        && value.schedule_id == control.schedule.schedule_id
        && value.schedule_sha256 == control.schedule.schedule_sha256
        && value.scheduled_contract_id == control.scheduled_contract.contract_id
        && value.scheduled_contract_sha256 == control.scheduled_contract.contract_sha256
        && value.scheduled_provider_request_id == request.provider_request_id
        && value.scheduled_provider_request_sha256 == request.prepared_request_sha256
        && value.logical_request_id == request.logical_request_id
        && value.logical_request_sha256 == request.logical_request_sha256
        && value.request_body_sha256 == request.provider_request_sha256
        && value.request_body_bytes == request.provider_request_bytes
        && value.expected_last_event_seq == source.last_event_seq
        && value.expected_last_event_sha256 == source.last_event_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("ready authorization source bindings disagree"))
}

fn validate_authorization_execution_bindings(
    value: &GroupAgentScheduledReadyNodeDispatchAuthorization,
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let contract = &control.scheduled_contract;
    let request = &control.provider_request;
    let node = &contract.node;
    let valid = value.execution_ordinal == node.execution_ordinal
        && value.node_id == node.node_id
        && value.attempt == node.attempt
        && value.project_id == node.project_id
        && value.project_lane_sha256 == node.project_lane_sha256
        && value.same_project_policy == node.same_project_policy
        && value.provider_kind == contract.provider.kind
        && value.endpoint == contract.provider.endpoint
        && value.model == contract.provider.model
        && value.destination_sha256 == request.destination_sha256
        && value.pricing_snapshot_sha256 == request.pricing_snapshot_sha256
        && value.budgets == contract.budgets
        && value.failure == contract.failure;
    valid
        .then_some(())
        .ok_or_else(|| invalid("ready authorization execution bindings disagree"))
}

fn content_id(value: &str, prefix: &str, sha256: &str) -> bool {
    digest(sha256) && valid_identifier(value) && value == format!("{prefix}{sha256}")
}

fn valid_identifier(value: &str) -> bool {
    super::super::validation::valid_identifier(value)
}

fn digest(value: &str) -> bool {
    super::super::validation::is_digest(value)
}

fn bounded(
    bytes: usize,
    maximum: usize,
    message: &str,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    (1..=maximum)
        .contains(&bytes)
        .then_some(())
        .ok_or_else(|| invalid(message))
}

pub(super) fn invalid(message: &str) -> GroupAgentScheduledReadyNodeDispatchReleaseValidationError {
    GroupAgentScheduledReadyNodeDispatchReleaseValidationError {
        message: message.into(),
    }
}
