use super::{
    GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractScope, GroupAgentScheduledNodeContractValidationError,
    GroupAgentScheduledNodeExecutionNode, GroupAgentScheduledNodeRequest,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES,
};
use crate::{
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentNodeSameProjectPolicy,
    MAX_GROUP_AGENT_GRAPH_MANAGER_INSTRUCTION_BYTES, MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES, MAX_GROUP_AGENT_GRAPH_NODES,
    group_agent_project_lane_sha256, group_agent_prompt_sha256,
};

const CONTRACT_ID_PREFIX: &str = "scheduled-node-contract-";
const REQUEST_ID_PREFIX: &str = "scheduled-node-request-";
const PROMPT_ENVELOPE_BYTES: usize = 1_024;
const MAX_SYSTEM_PROMPT_BYTES: usize =
    MAX_GROUP_AGENT_GRAPH_MANAGER_INSTRUCTION_BYTES + PROMPT_ENVELOPE_BYTES;
const MAX_USER_PROMPT_BYTES: usize = MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES
    + MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES
    + PROMPT_ENVELOPE_BYTES;

pub(super) fn validate_candidate(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    validate_header(candidate)?;
    validate_node(&candidate.node, candidate.contract_scope)?;
    validate_request(&candidate.request, candidate.contract_scope)?;
    validate_request_binding(candidate)?;
    validate_shared_policy(candidate)?;
    let digest = candidate.expected_sha256()?;
    if candidate.contract_sha256 != digest
        || candidate.contract_id != format!("{CONTRACT_ID_PREFIX}{digest}")
    {
        return Err(invalid("scheduled contract digest or identity disagrees"));
    }
    let bytes = candidate.canonical_json()?.len();
    if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES).contains(&bytes) {
        return Err(invalid("scheduled contract exceeds its byte bound"));
    }
    Ok(())
}

fn validate_header(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let valid = candidate.v == GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION
        && candidate.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && candidate.node_execution_protocol_version
            == GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION
        && candidate.execution_schedule_protocol_version
            == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION
        && matches!(
            candidate.contract_scope,
            GroupAgentScheduledNodeContractScope::ScheduleInitialNodeOnly
                | GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly
        )
        && super::super::validation::valid_identifier(&candidate.graph_run_id)
        && super::super::validation::valid_identifier(&candidate.graph_id)
        && digest(&candidate.source_snapshot_sha256)
        && digest(&candidate.graph_manifest_sha256)
        && digest(&candidate.core_plan_sha256)
        && digest(&candidate.control_snapshot_sha256)
        && valid_content_id(
            &candidate.schedule_id,
            "graph-execution-schedule-",
            &candidate.schedule_sha256,
        )
        && candidate.expected_last_event_seq == 1
        && digest(&candidate.expected_last_event_sha256)
        && !candidate.lifecycle_contract_admitted
        && !candidate.provider_request_present
        && !candidate.execution_authority_released
        && !candidate.dispatch_authority_released
        && !candidate.progress_observed
        && !candidate.successor_advance_authorized
        && digest(&candidate.contract_sha256);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid passive scheduled contract header"))
}

fn validate_node(
    node: &GroupAgentScheduledNodeExecutionNode,
    scope: GroupAgentScheduledNodeContractScope,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let (ordinal_valid, wave_valid) = match scope {
        GroupAgentScheduledNodeContractScope::ScheduleInitialNodeOnly => {
            (node.execution_ordinal == 0, node.topology_wave_index == 0)
        }
        GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly => (
            node.execution_ordinal >= 1 && node.execution_ordinal < MAX_GROUP_AGENT_GRAPH_NODES,
            node.topology_wave_index < MAX_GROUP_AGENT_GRAPH_NODES,
        ),
    };
    let valid = ordinal_valid
        && super::super::validation::valid_identifier(&node.node_id)
        && node.authored_node_index < MAX_GROUP_AGENT_GRAPH_NODES
        && wave_valid
        && node.attempt == 1
        && super::super::validation::valid_identifier(&node.project_id)
        && super::super::validation::valid_text(&node.member_role, 64)
        && super::super::validation::valid_text(&node.agent_profile, 128)
        && node.project_lane_sha256 == group_agent_project_lane_sha256(&node.project_id)
        && node.same_project_policy == GroupAgentNodeSameProjectPolicy::ExclusiveUntilTerminal;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid scheduled initial node"))
}

fn validate_request(
    request: &GroupAgentScheduledNodeRequest,
    scope: GroupAgentScheduledNodeContractScope,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let predecessors_valid = predecessors_valid(request, scope);
    let ordinal_valid = if scope == GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly {
        request.execution_ordinal >= 1
    } else {
        request.execution_ordinal == 0
    };
    let valid = request.v == GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION
        && super::super::validation::valid_identifier(&request.graph_run_id)
        && valid_content_id(
            &request.schedule_id,
            "graph-execution-schedule-",
            &request.schedule_sha256,
        )
        && ordinal_valid
        && super::super::validation::valid_identifier(&request.node_id)
        && request.attempt == 1
        && valid_prompt(
            &request.system_prompt,
            request.system_prompt_bytes,
            &request.system_prompt_sha256,
            MAX_SYSTEM_PROMPT_BYTES,
        )
        && valid_prompt(
            &request.user_prompt,
            request.user_prompt_bytes,
            &request.user_prompt_sha256,
            MAX_USER_PROMPT_BYTES,
        )
        && predecessors_valid
        && !request.predecessor_content_included
        && request.tools.is_empty()
        && digest(&request.request_sha256);
    if !valid || request.expected_sha256()? != request.request_sha256 {
        return Err(invalid("invalid scheduled node request"));
    }
    validate_user_prompt(request)?;
    if request.request_id != format!("{REQUEST_ID_PREFIX}{}", request.request_sha256) {
        return Err(invalid("scheduled request identity disagrees"));
    }
    Ok(())
}

/// Predecessor evidence rules: the initial candidate requires an empty set,
/// while a successor candidate requires at least one consumed receipt and
/// coverage of every required topological predecessor.
fn predecessors_valid(
    request: &GroupAgentScheduledNodeRequest,
    scope: GroupAgentScheduledNodeContractScope,
) -> bool {
    if scope != GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly {
        return request.required_predecessor_node_ids.is_empty()
            && request.predecessor_terminal_receipts.is_empty();
    }
    !request.predecessor_terminal_receipts.is_empty()
        && request
            .required_predecessor_node_ids
            .iter()
            .all(|required| {
                request
                    .predecessor_terminal_receipts
                    .iter()
                    .any(|receipt| &receipt.predecessor_node_id == required)
            })
        && request
            .predecessor_terminal_receipts
            .iter()
            .all(valid_receipt)
}

/// Every consumed predecessor receipt must carry exact identity evidence and
/// stay free of forbidden authority.
fn valid_receipt(receipt: &crate::GroupAgentScheduledNodePredecessorReceipt) -> bool {
    super::super::validation::valid_identifier(&receipt.predecessor_node_id)
        && receipt.predecessor_attempt == 1
        && valid_content_id(
            &receipt.terminal_receipt_id,
            "scheduled-node-terminal-receipt-",
            &receipt.terminal_receipt_sha256,
        )
        && super::super::validation::valid_identifier(&receipt.provider_request_id)
        && super::super::validation::valid_identifier(&receipt.dispatch_id)
}

fn validate_request_binding(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let request = &candidate.request;
    let node = &candidate.node;
    let valid = request.graph_run_id == candidate.graph_run_id
        && request.schedule_id == candidate.schedule_id
        && request.schedule_sha256 == candidate.schedule_sha256
        && request.execution_ordinal == node.execution_ordinal
        && request.node_id == node.node_id
        && request.attempt == node.attempt;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled request and contract bindings disagree"))
}

fn validate_shared_policy(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let checks = [
        super::super::validation::validate_workspace(&candidate.workspace),
        super::super::validation::validate_provider(&candidate.provider),
        super::super::validation::validate_budgets(&candidate.budgets),
        super::super::validation::validate_approval(&candidate.approval),
        super::super::validation::validate_result(&candidate.result),
        super::super::validation::validate_failure(&candidate.failure),
    ];
    checks
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map(|_| ())
        .map_err(|error| invalid(&error.to_string()))
}

fn valid_prompt(value: &str, bytes: usize, sha256: &str, maximum: usize) -> bool {
    super::super::validation::valid_prose(value)
        && value.len() <= maximum
        && bytes == value.len()
        && sha256 == group_agent_prompt_sha256(value)
}

fn validate_user_prompt(
    request: &GroupAgentScheduledNodeRequest,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let prompt = super::codec::decode_user_prompt_exact(&request.user_prompt)?;
    let content_present = prompt.predecessor_output.is_some();
    let content_valid = match &prompt.predecessor_output {
        Some(output) => {
            !output.is_empty()
                && output.len() <= MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES
                && super::super::validation::valid_prose(output)
        }
        None => true,
    };
    let valid = prompt.v == GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION
        && prompt.node_id == request.node_id
        && super::super::validation::valid_identifier(&prompt.node_id)
        && prompt.task.len() <= MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES
        && super::super::validation::valid_prose(&prompt.task)
        && prompt.acceptance.len() <= MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES
        && super::super::validation::valid_prose(&prompt.acceptance)
        && request.predecessor_content_included == content_present
        && content_valid;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid scheduled user Prompt"))
}

fn valid_content_id(value: &str, prefix: &str, sha256: &str) -> bool {
    digest(sha256)
        && super::super::validation::valid_identifier(value)
        && value == format!("{prefix}{sha256}")
}

pub(super) fn digest(value: &str) -> bool {
    super::super::validation::is_digest(value)
}

pub(super) fn invalid(message: &str) -> GroupAgentScheduledNodeContractValidationError {
    GroupAgentScheduledNodeContractValidationError {
        message: message.into(),
    }
}
