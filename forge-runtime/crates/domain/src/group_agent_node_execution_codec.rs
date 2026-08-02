use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_DIGEST_DOMAIN,
    GROUP_AGENT_NODE_EXECUTION_CONTRACT_DIGEST_DOMAIN, GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
    GROUP_AGENT_NODE_REQUEST_DIGEST_DOMAIN, GroupAgentGraphControlSnapshot,
    GroupAgentNodeExecutionApproval, GroupAgentNodeExecutionBudgets,
    GroupAgentNodeExecutionContract, GroupAgentNodeExecutionFailurePolicy,
    GroupAgentNodeExecutionNode, GroupAgentNodeExecutionProvider, GroupAgentNodeExecutionRequest,
    GroupAgentNodeExecutionResultPolicy, GroupAgentNodeExecutionValidationError,
    GroupAgentNodeExecutionWorkspace,
};

#[derive(Serialize)]
struct ControlSnapshotPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    graph_run_version: u16,
    graph_run_id: &'a str,
    graph_id: &'a str,
    source_snapshot_sha256: &'a str,
    graph_manifest_sha256: &'a str,
    core_plan_sha256: &'a str,
    last_event_seq: u64,
    last_event_sha256: &'a str,
    execution_contract_present: bool,
    dispatch_authority_released: bool,
    plan: &'a crate::GroupAgentGraphCorePlan,
    manifest: &'a crate::GroupAgentGraphManifest,
}

#[derive(Serialize)]
struct RequestPayload<'a> {
    system_prompt: &'a str,
    system_prompt_bytes: usize,
    system_prompt_sha256: &'a str,
    user_prompt: &'a str,
    user_prompt_bytes: usize,
    user_prompt_sha256: &'a str,
    predecessor_result_receipts: &'a [String],
    tools: &'a [String],
}

#[derive(Serialize)]
struct ContractPayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    node_execution_protocol_version: u16,
    graph_run_id: &'a str,
    graph_id: &'a str,
    source_snapshot_sha256: &'a str,
    graph_manifest_sha256: &'a str,
    core_plan_sha256: &'a str,
    control_snapshot_sha256: &'a str,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
    node: &'a GroupAgentNodeExecutionNode,
    workspace: &'a GroupAgentNodeExecutionWorkspace,
    provider: &'a GroupAgentNodeExecutionProvider,
    request: &'a GroupAgentNodeExecutionRequest,
    budgets: &'a GroupAgentNodeExecutionBudgets,
    approval: &'a GroupAgentNodeExecutionApproval,
    result: &'a GroupAgentNodeExecutionResultPolicy,
    failure: &'a GroupAgentNodeExecutionFailurePolicy,
    execution_contract_present: bool,
    dispatch_authority_released: bool,
}

#[derive(Serialize)]
struct UserPrompt<'a> {
    v: u16,
    node_id: &'a str,
    task: &'a str,
    acceptance: &'a str,
    predecessor_result_receipts: &'a [String],
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentNodeExecutionValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("value cannot be encoded as JSON"))
}

pub(super) fn control_snapshot_digest(
    snapshot: &GroupAgentGraphControlSnapshot,
) -> Result<String, GroupAgentNodeExecutionValidationError> {
    let payload = ControlSnapshotPayload {
        v: snapshot.v,
        scheduler_protocol_version: snapshot.scheduler_protocol_version,
        graph_run_version: snapshot.graph_run_version,
        graph_run_id: &snapshot.graph_run_id,
        graph_id: &snapshot.graph_id,
        source_snapshot_sha256: &snapshot.source_snapshot_sha256,
        graph_manifest_sha256: &snapshot.graph_manifest_sha256,
        core_plan_sha256: &snapshot.core_plan_sha256,
        last_event_seq: snapshot.last_event_seq,
        last_event_sha256: &snapshot.last_event_sha256,
        execution_contract_present: snapshot.execution_contract_present,
        dispatch_authority_released: snapshot.dispatch_authority_released,
        plan: &snapshot.plan,
        manifest: &snapshot.manifest,
    };
    digest_json(GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_DIGEST_DOMAIN, &payload)
}

pub(super) fn request_digest(
    request: &GroupAgentNodeExecutionRequest,
) -> Result<String, GroupAgentNodeExecutionValidationError> {
    let payload = RequestPayload {
        system_prompt: &request.system_prompt,
        system_prompt_bytes: request.system_prompt_bytes,
        system_prompt_sha256: &request.system_prompt_sha256,
        user_prompt: &request.user_prompt,
        user_prompt_bytes: request.user_prompt_bytes,
        user_prompt_sha256: &request.user_prompt_sha256,
        predecessor_result_receipts: &request.predecessor_result_receipts,
        tools: &request.tools,
    };
    digest_json(GROUP_AGENT_NODE_REQUEST_DIGEST_DOMAIN, &payload)
}

pub(super) fn contract_digest(
    contract: &GroupAgentNodeExecutionContract,
) -> Result<String, GroupAgentNodeExecutionValidationError> {
    let payload = ContractPayload {
        v: contract.v,
        scheduler_protocol_version: contract.scheduler_protocol_version,
        node_execution_protocol_version: contract.node_execution_protocol_version,
        graph_run_id: &contract.graph_run_id,
        graph_id: &contract.graph_id,
        source_snapshot_sha256: &contract.source_snapshot_sha256,
        graph_manifest_sha256: &contract.graph_manifest_sha256,
        core_plan_sha256: &contract.core_plan_sha256,
        control_snapshot_sha256: &contract.control_snapshot_sha256,
        expected_last_event_seq: contract.expected_last_event_seq,
        expected_last_event_sha256: &contract.expected_last_event_sha256,
        node: &contract.node,
        workspace: &contract.workspace,
        provider: &contract.provider,
        request: &contract.request,
        budgets: &contract.budgets,
        approval: &contract.approval,
        result: &contract.result,
        failure: &contract.failure,
        execution_contract_present: contract.execution_contract_present,
        dispatch_authority_released: contract.dispatch_authority_released,
    };
    digest_json(GROUP_AGENT_NODE_EXECUTION_CONTRACT_DIGEST_DOMAIN, &payload)
}

pub(super) fn user_prompt(
    node_id: &str,
    task: &str,
    acceptance: &str,
) -> Result<String, GroupAgentNodeExecutionValidationError> {
    canonical_json(&UserPrompt {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        node_id,
        task,
        acceptance,
        predecessor_result_receipts: &[],
    })
}

/// Computes the unkeyed SHA-256 identity of exact UTF-8 Prompt bytes.
#[must_use]
pub fn group_agent_prompt_sha256(prompt: &str) -> String {
    digest_hex(&[], prompt.as_bytes())
}

/// Builds the exact fixed system Prompt around the frozen manager instruction.
#[must_use]
pub fn group_agent_node_system_prompt(manager_instruction: &str) -> String {
    format!(
        "Execute exactly one frozen Group Agent Graph node. Follow the manager \
instruction, complete only the assigned task, and return a text result that can \
be checked against the acceptance criteria. Tools, network, workspace access, \
memory, and writeback are unavailable.\n\nManager instruction:\n{manager_instruction}"
    )
}

/// Builds the exact canonical user Prompt for one node.
///
/// # Errors
///
/// Returns an error when canonical encoding fails.
pub fn group_agent_node_user_prompt(
    node_id: &str,
    task: &str,
    acceptance: &str,
) -> Result<String, GroupAgentNodeExecutionValidationError> {
    user_prompt(node_id, task, acceptance)
}

pub(super) fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn digest_json(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<String, GroupAgentNodeExecutionValidationError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| invalid("value cannot be encoded as JSON"))?;
    Ok(digest_hex(domain, &bytes))
}

fn invalid(message: &str) -> GroupAgentNodeExecutionValidationError {
    GroupAgentNodeExecutionValidationError {
        message: message.into(),
    }
}
