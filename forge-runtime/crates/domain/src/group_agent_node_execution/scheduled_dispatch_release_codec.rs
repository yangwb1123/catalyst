use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeDispatchReleaseValidationError,
};

#[derive(Serialize)]
struct ReleaseControlPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    release_control_protocol_version: u16,
    graph_run: &'a crate::GroupAgentGraphRunRecord,
    journal_events: &'a [crate::GroupAgentGraphRunEvent],
    control_snapshot: &'a crate::GroupAgentGraphControlSnapshot,
    schedule_record: &'a crate::GroupAgentGraphExecutionScheduleRecord,
    schedule: &'a crate::GroupAgentGraphExecutionSchedule,
    scheduled_contract_record: &'a crate::GroupAgentScheduledNodeContractRecord,
    scheduled_contract: &'a crate::GroupAgentScheduledNodeContractCandidate,
    provider_request: &'a crate::GroupAgentScheduledNodeProviderRequestRecord,
    provider_request_json: &'a str,
}

#[derive(Serialize)]
struct AuthorizationSource<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    dispatch_authorization_protocol_version: u16,
    graph_run_id: &'a str,
    graph_id: &'a str,
    group_run_id: &'a str,
    group_id: &'a str,
    source_snapshot_sha256: &'a str,
    graph_manifest_sha256: &'a str,
    core_plan_sha256: &'a str,
    control_snapshot_sha256: &'a str,
    release_control_snapshot_sha256: &'a str,
    schedule_id: &'a str,
    schedule_sha256: &'a str,
    scheduled_contract_id: &'a str,
    scheduled_contract_sha256: &'a str,
    scheduled_provider_request_id: &'a str,
    scheduled_provider_request_sha256: &'a str,
    logical_request_id: &'a str,
    logical_request_sha256: &'a str,
    request_body_sha256: &'a str,
    request_body_bytes: usize,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
}

#[derive(Serialize)]
struct AuthorizationExecution<'a> {
    execution_ordinal: usize,
    node_id: &'a str,
    attempt: u16,
    project_id: &'a str,
    project_lane_sha256: &'a str,
    same_project_policy: crate::GroupAgentNodeSameProjectPolicy,
    provider_kind: crate::GroupAgentNodeProviderKind,
    endpoint: &'a str,
    model: &'a str,
    destination_sha256: &'a str,
    pricing_snapshot_sha256: &'a str,
    budgets: &'a crate::GroupAgentNodeExecutionBudgets,
    release_requirements: &'a super::GroupAgentScheduledNodeDispatchReleaseRequirements,
    failure: &'a crate::GroupAgentNodeExecutionFailurePolicy,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct AuthorizationState {
    lifecycle_contract_admission_authorized: bool,
    execution_authority_release_authorized: bool,
    dispatch_authority_release_authorized: bool,
    scheduled_contract_candidate_present: bool,
    provider_request_prepared: bool,
    lifecycle_contract_admitted: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    project_lane_claimed: bool,
    provider_request_sent: bool,
    progress_observed: bool,
    terminal_receipt_recorded: bool,
    successor_advance_authorized: bool,
}

#[derive(Serialize)]
struct AuthorizationPayload<'a> {
    #[serde(flatten)]
    source: AuthorizationSource<'a>,
    #[serde(flatten)]
    execution: AuthorizationExecution<'a>,
    #[serde(flatten)]
    state: AuthorizationState,
}

impl<'a> From<&'a GroupAgentScheduledNodeDispatchReleaseControl> for ReleaseControlPayload<'a> {
    fn from(value: &'a GroupAgentScheduledNodeDispatchReleaseControl) -> Self {
        Self {
            v: value.v,
            scheduler_protocol_version: value.scheduler_protocol_version,
            release_control_protocol_version: value.release_control_protocol_version,
            graph_run: &value.graph_run,
            journal_events: &value.journal_events,
            control_snapshot: &value.control_snapshot,
            schedule_record: &value.schedule_record,
            schedule: &value.schedule,
            scheduled_contract_record: &value.scheduled_contract_record,
            scheduled_contract: &value.scheduled_contract,
            provider_request: &value.provider_request,
            provider_request_json: &value.provider_request_json,
        }
    }
}

impl<'a> From<&'a GroupAgentScheduledNodeDispatchAuthorization> for AuthorizationSource<'a> {
    fn from(value: &'a GroupAgentScheduledNodeDispatchAuthorization) -> Self {
        Self {
            v: value.v,
            scheduler_protocol_version: value.scheduler_protocol_version,
            dispatch_authorization_protocol_version: value.dispatch_authorization_protocol_version,
            graph_run_id: &value.graph_run_id,
            graph_id: &value.graph_id,
            group_run_id: &value.group_run_id,
            group_id: &value.group_id,
            source_snapshot_sha256: &value.source_snapshot_sha256,
            graph_manifest_sha256: &value.graph_manifest_sha256,
            core_plan_sha256: &value.core_plan_sha256,
            control_snapshot_sha256: &value.control_snapshot_sha256,
            release_control_snapshot_sha256: &value.release_control_snapshot_sha256,
            schedule_id: &value.schedule_id,
            schedule_sha256: &value.schedule_sha256,
            scheduled_contract_id: &value.scheduled_contract_id,
            scheduled_contract_sha256: &value.scheduled_contract_sha256,
            scheduled_provider_request_id: &value.scheduled_provider_request_id,
            scheduled_provider_request_sha256: &value.scheduled_provider_request_sha256,
            logical_request_id: &value.logical_request_id,
            logical_request_sha256: &value.logical_request_sha256,
            request_body_sha256: &value.request_body_sha256,
            request_body_bytes: value.request_body_bytes,
            expected_last_event_seq: value.expected_last_event_seq,
            expected_last_event_sha256: &value.expected_last_event_sha256,
        }
    }
}

impl<'a> From<&'a GroupAgentScheduledNodeDispatchAuthorization> for AuthorizationExecution<'a> {
    fn from(value: &'a GroupAgentScheduledNodeDispatchAuthorization) -> Self {
        Self {
            execution_ordinal: value.execution_ordinal,
            node_id: &value.node_id,
            attempt: value.attempt,
            project_id: &value.project_id,
            project_lane_sha256: &value.project_lane_sha256,
            same_project_policy: value.same_project_policy,
            provider_kind: value.provider_kind,
            endpoint: &value.endpoint,
            model: &value.model,
            destination_sha256: &value.destination_sha256,
            pricing_snapshot_sha256: &value.pricing_snapshot_sha256,
            budgets: &value.budgets,
            release_requirements: &value.release_requirements,
            failure: &value.failure,
        }
    }
}

impl From<&GroupAgentScheduledNodeDispatchAuthorization> for AuthorizationState {
    fn from(value: &GroupAgentScheduledNodeDispatchAuthorization) -> Self {
        Self {
            lifecycle_contract_admission_authorized: value.lifecycle_contract_admission_authorized,
            execution_authority_release_authorized: value.execution_authority_release_authorized,
            dispatch_authority_release_authorized: value.dispatch_authority_release_authorized,
            scheduled_contract_candidate_present: value.scheduled_contract_candidate_present,
            provider_request_prepared: value.provider_request_prepared,
            lifecycle_contract_admitted: value.lifecycle_contract_admitted,
            execution_authority_released: value.execution_authority_released,
            dispatch_authority_released: value.dispatch_authority_released,
            project_lane_claimed: value.project_lane_claimed,
            provider_request_sent: value.provider_request_sent,
            progress_observed: value.progress_observed,
            terminal_receipt_recorded: value.terminal_receipt_recorded,
            successor_advance_authorized: value.successor_advance_authorized,
        }
    }
}

impl<'a> From<&'a GroupAgentScheduledNodeDispatchAuthorization> for AuthorizationPayload<'a> {
    fn from(value: &'a GroupAgentScheduledNodeDispatchAuthorization) -> Self {
        Self {
            source: value.into(),
            execution: value.into(),
            state: value.into(),
        }
    }
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("value cannot be canonically encoded"))
}

pub(super) fn release_control_digest(
    value: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
    digest(
        GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN,
        &ReleaseControlPayload::from(value),
    )
}

pub(super) fn authorization_payload_json(
    value: &GroupAgentScheduledNodeDispatchAuthorization,
) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
    canonical_json(&AuthorizationPayload::from(value))
}

pub(super) fn authorization_digest(
    value: &GroupAgentScheduledNodeDispatchAuthorization,
) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
    digest(
        GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN,
        &AuthorizationPayload::from(value),
    )
}

fn digest(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| invalid("digest payload cannot be encoded"))?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn invalid(message: &str) -> GroupAgentScheduledNodeDispatchReleaseValidationError {
    GroupAgentScheduledNodeDispatchReleaseValidationError {
        message: message.into(),
    }
}
