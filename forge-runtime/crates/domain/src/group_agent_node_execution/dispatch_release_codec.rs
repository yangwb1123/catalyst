use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchReleaseControl, GroupAgentNodeDispatchReleaseValidationError,
};

#[derive(Serialize)]
struct ReleaseControlPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    release_control_protocol_version: u16,
    graph_run: &'a crate::GroupAgentGraphRunRecord,
    plan: &'a crate::GroupAgentGraphCorePlan,
    manifest: &'a crate::GroupAgentGraphManifest,
    journal_events: &'a [crate::GroupAgentGraphRunEvent],
    contract_record: &'a crate::GroupAgentNodeExecutionContractRecord,
    contract: &'a crate::GroupAgentNodeExecutionContract,
    dispatch_request: &'a crate::GroupAgentNodeDispatchRequestRecord,
    provider_request_json: &'a str,
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct AuthorizationPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    dispatch_authorization_protocol_version: u16,
    graph_run_id: &'a str,
    graph_id: &'a str,
    group_run_id: &'a str,
    source_snapshot_sha256: &'a str,
    graph_manifest_sha256: &'a str,
    core_plan_sha256: &'a str,
    release_control_snapshot_sha256: &'a str,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
    contract_id: &'a str,
    contract_sha256: &'a str,
    dispatch_request_id: &'a str,
    dispatch_request_sha256: &'a str,
    logical_request_sha256: &'a str,
    request_body_sha256: &'a str,
    request_body_bytes: usize,
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
    release_requirements: &'a super::GroupAgentNodeDispatchReleaseRequirements,
    failure: &'a crate::GroupAgentNodeExecutionFailurePolicy,
    execution_contract_present: bool,
    dispatch_request_present: bool,
    dispatch_authority_release_authorized: bool,
    dispatch_authority_released: bool,
}

impl<'a> From<&'a GroupAgentNodeDispatchReleaseControl> for ReleaseControlPayload<'a> {
    fn from(control: &'a GroupAgentNodeDispatchReleaseControl) -> Self {
        Self {
            v: control.v,
            scheduler_protocol_version: control.scheduler_protocol_version,
            release_control_protocol_version: control.release_control_protocol_version,
            graph_run: &control.graph_run,
            plan: &control.plan,
            manifest: &control.manifest,
            journal_events: &control.journal_events,
            contract_record: &control.contract_record,
            contract: &control.contract,
            dispatch_request: &control.dispatch_request,
            provider_request_json: &control.provider_request_json,
        }
    }
}

impl<'a> From<&'a GroupAgentNodeDispatchAuthorization> for AuthorizationPayload<'a> {
    fn from(value: &'a GroupAgentNodeDispatchAuthorization) -> Self {
        Self {
            v: value.v,
            scheduler_protocol_version: value.scheduler_protocol_version,
            dispatch_authorization_protocol_version: value.dispatch_authorization_protocol_version,
            graph_run_id: &value.graph_run_id,
            graph_id: &value.graph_id,
            group_run_id: &value.group_run_id,
            source_snapshot_sha256: &value.source_snapshot_sha256,
            graph_manifest_sha256: &value.graph_manifest_sha256,
            core_plan_sha256: &value.core_plan_sha256,
            release_control_snapshot_sha256: &value.release_control_snapshot_sha256,
            expected_last_event_seq: value.expected_last_event_seq,
            expected_last_event_sha256: &value.expected_last_event_sha256,
            contract_id: &value.contract_id,
            contract_sha256: &value.contract_sha256,
            dispatch_request_id: &value.dispatch_request_id,
            dispatch_request_sha256: &value.dispatch_request_sha256,
            logical_request_sha256: &value.logical_request_sha256,
            request_body_sha256: &value.request_body_sha256,
            request_body_bytes: value.request_body_bytes,
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
            execution_contract_present: value.execution_contract_present,
            dispatch_request_present: value.dispatch_request_present,
            dispatch_authority_release_authorized: value.dispatch_authority_release_authorized,
            dispatch_authority_released: value.dispatch_authority_released,
        }
    }
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("value cannot be encoded as canonical JSON"))
}

pub(super) fn release_control_digest(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
    digest_json(
        GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN,
        &ReleaseControlPayload::from(control),
    )
}

pub(super) fn authorization_payload_json(
    authorization: &GroupAgentNodeDispatchAuthorization,
) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
    canonical_json(&AuthorizationPayload::from(authorization))
}

pub(super) fn authorization_digest(
    authorization: &GroupAgentNodeDispatchAuthorization,
) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
    digest_json(
        GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN,
        &AuthorizationPayload::from(authorization),
    )
}

fn digest_json(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| invalid("digest payload cannot be encoded as canonical JSON"))?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn invalid(message: &str) -> GroupAgentNodeDispatchReleaseValidationError {
    GroupAgentNodeDispatchReleaseValidationError {
        message: message.into(),
    }
}
