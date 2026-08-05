use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_SCHEDULED_NODE_CONTRACT_DIGEST_DOMAIN,
    GROUP_AGENT_SCHEDULED_NODE_REQUEST_DIGEST_DOMAIN, GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractScope,
    GroupAgentScheduledNodeContractValidationError, GroupAgentScheduledNodeExecutionNode,
    GroupAgentScheduledNodePredecessorReceipt, GroupAgentScheduledNodeRequest,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
};
use crate::{
    GroupAgentNodeExecutionApproval, GroupAgentNodeExecutionBudgets,
    GroupAgentNodeExecutionFailurePolicy, GroupAgentNodeExecutionProvider,
    GroupAgentNodeExecutionResultPolicy, GroupAgentNodeExecutionWorkspace,
};

#[derive(Serialize)]
struct RequestPayload<'a> {
    v: u16,
    graph_run_id: &'a str,
    schedule_id: &'a str,
    schedule_sha256: &'a str,
    execution_ordinal: usize,
    node_id: &'a str,
    attempt: u16,
    system_prompt: &'a str,
    system_prompt_bytes: usize,
    system_prompt_sha256: &'a str,
    user_prompt: &'a str,
    user_prompt_bytes: usize,
    user_prompt_sha256: &'a str,
    required_predecessor_node_ids: &'a [String],
    predecessor_terminal_receipts: &'a [GroupAgentScheduledNodePredecessorReceipt],
    predecessor_content_included: bool,
    tools: &'a [String],
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct ContractPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    node_execution_protocol_version: u16,
    execution_schedule_protocol_version: u16,
    contract_scope: GroupAgentScheduledNodeContractScope,
    graph_run_id: &'a str,
    graph_id: &'a str,
    source_snapshot_sha256: &'a str,
    graph_manifest_sha256: &'a str,
    core_plan_sha256: &'a str,
    control_snapshot_sha256: &'a str,
    schedule_id: &'a str,
    schedule_sha256: &'a str,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
    node: &'a GroupAgentScheduledNodeExecutionNode,
    request: &'a GroupAgentScheduledNodeRequest,
    workspace: &'a GroupAgentNodeExecutionWorkspace,
    provider: &'a GroupAgentNodeExecutionProvider,
    budgets: &'a GroupAgentNodeExecutionBudgets,
    approval: &'a GroupAgentNodeExecutionApproval,
    result: &'a GroupAgentNodeExecutionResultPolicy,
    failure: &'a GroupAgentNodeExecutionFailurePolicy,
    lifecycle_contract_admitted: bool,
    provider_request_present: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    progress_observed: bool,
    successor_advance_authorized: bool,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UserPrompt {
    pub(super) v: u16,
    pub(super) node_id: String,
    pub(super) task: String,
    pub(super) acceptance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) predecessor_output: Option<String>,
}

pub(super) fn decode_exact(
    bytes: &[u8],
) -> Result<GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractValidationError>
{
    if bytes.is_empty() || bytes.len() > MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES {
        return Err(invalid(
            "scheduled contract input is outside its byte bound",
        ));
    }
    let candidate: GroupAgentScheduledNodeContractCandidate = serde_json::from_slice(bytes)
        .map_err(|_| invalid("scheduled contract input is invalid JSON"))?;
    candidate.validate()?;
    if candidate.canonical_json()?.as_bytes() != bytes {
        return Err(invalid(
            "scheduled contract input is not exact canonical JSON",
        ));
    }
    Ok(candidate)
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("scheduled contract cannot be encoded"))
}

pub(super) fn request_digest(
    request: &GroupAgentScheduledNodeRequest,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    digest_json(
        GROUP_AGENT_SCHEDULED_NODE_REQUEST_DIGEST_DOMAIN,
        &request_payload(request),
    )
}

pub(super) fn contract_digest(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    digest_json(
        GROUP_AGENT_SCHEDULED_NODE_CONTRACT_DIGEST_DOMAIN,
        &contract_payload(candidate),
    )
}

pub(super) fn user_prompt(
    node_id: &str,
    task: &str,
    acceptance: &str,
    predecessor_output: Option<&str>,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    canonical_json(&UserPrompt {
        v: GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION,
        node_id: node_id.into(),
        task: task.into(),
        acceptance: acceptance.into(),
        predecessor_output: predecessor_output.map(str::to_owned),
    })
}

pub(super) fn decode_user_prompt_exact(
    value: &str,
) -> Result<UserPrompt, GroupAgentScheduledNodeContractValidationError> {
    let prompt: UserPrompt = serde_json::from_str(value)
        .map_err(|_| invalid("scheduled user Prompt is invalid JSON"))?;
    if canonical_json(&prompt)?.as_str() != value {
        return Err(invalid("scheduled user Prompt is not exact canonical JSON"));
    }
    Ok(prompt)
}

#[cfg(test)]
pub(super) fn request_payload_json(
    request: &GroupAgentScheduledNodeRequest,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    canonical_json(&request_payload(request))
}

#[cfg(test)]
pub(super) fn contract_payload_json(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    canonical_json(&contract_payload(candidate))
}

fn request_payload(request: &GroupAgentScheduledNodeRequest) -> RequestPayload<'_> {
    RequestPayload {
        v: request.v,
        graph_run_id: &request.graph_run_id,
        schedule_id: &request.schedule_id,
        schedule_sha256: &request.schedule_sha256,
        execution_ordinal: request.execution_ordinal,
        node_id: &request.node_id,
        attempt: request.attempt,
        system_prompt: &request.system_prompt,
        system_prompt_bytes: request.system_prompt_bytes,
        system_prompt_sha256: &request.system_prompt_sha256,
        user_prompt: &request.user_prompt,
        user_prompt_bytes: request.user_prompt_bytes,
        user_prompt_sha256: &request.user_prompt_sha256,
        required_predecessor_node_ids: &request.required_predecessor_node_ids,
        predecessor_terminal_receipts: &request.predecessor_terminal_receipts,
        predecessor_content_included: request.predecessor_content_included,
        tools: &request.tools,
    }
}

fn contract_payload(candidate: &GroupAgentScheduledNodeContractCandidate) -> ContractPayload<'_> {
    ContractPayload {
        v: candidate.v,
        scheduler_protocol_version: candidate.scheduler_protocol_version,
        node_execution_protocol_version: candidate.node_execution_protocol_version,
        execution_schedule_protocol_version: candidate.execution_schedule_protocol_version,
        contract_scope: candidate.contract_scope,
        graph_run_id: &candidate.graph_run_id,
        graph_id: &candidate.graph_id,
        source_snapshot_sha256: &candidate.source_snapshot_sha256,
        graph_manifest_sha256: &candidate.graph_manifest_sha256,
        core_plan_sha256: &candidate.core_plan_sha256,
        control_snapshot_sha256: &candidate.control_snapshot_sha256,
        schedule_id: &candidate.schedule_id,
        schedule_sha256: &candidate.schedule_sha256,
        expected_last_event_seq: candidate.expected_last_event_seq,
        expected_last_event_sha256: &candidate.expected_last_event_sha256,
        node: &candidate.node,
        request: &candidate.request,
        workspace: &candidate.workspace,
        provider: &candidate.provider,
        budgets: &candidate.budgets,
        approval: &candidate.approval,
        result: &candidate.result,
        failure: &candidate.failure,
        lifecycle_contract_admitted: candidate.lifecycle_contract_admitted,
        provider_request_present: candidate.provider_request_present,
        execution_authority_released: candidate.execution_authority_released,
        dispatch_authority_released: candidate.dispatch_authority_released,
        progress_observed: candidate.progress_observed,
        successor_advance_authorized: candidate.successor_advance_authorized,
    }
}

fn digest_json(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| invalid("digest payload cannot be encoded"))?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn invalid(message: &str) -> GroupAgentScheduledNodeContractValidationError {
    GroupAgentScheduledNodeContractValidationError {
        message: message.into(),
    }
}
