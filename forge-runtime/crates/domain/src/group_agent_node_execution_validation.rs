use std::net::Ipv4Addr;

use super::{
    GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION, GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
    GROUP_AGENT_NODE_EXECUTION_PROTOCOL_VERSION, GroupAgentGraphControlSnapshot,
    GroupAgentNodeArtifactKind, GroupAgentNodeDataflowPolicy, GroupAgentNodeEffectApproval,
    GroupAgentNodeExecutionApproval, GroupAgentNodeExecutionBudgets,
    GroupAgentNodeExecutionContract, GroupAgentNodeExecutionFailurePolicy,
    GroupAgentNodeExecutionNode, GroupAgentNodeExecutionProvider, GroupAgentNodeExecutionRequest,
    GroupAgentNodeExecutionResultPolicy, GroupAgentNodeExecutionValidationError,
    GroupAgentNodeExecutionWorkspace, GroupAgentNodeFailurePropagationOwner,
    GroupAgentNodePostClaimUncertainty, GroupAgentNodeProviderApproval, GroupAgentNodeProviderKind,
    GroupAgentNodeSameProjectPolicy, GroupAgentNodeWorkspaceMode, GroupAgentNodeWritebackPolicy,
    MAX_GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES, MAX_GROUP_AGENT_NODE_COST_USD_MICROS,
    MAX_GROUP_AGENT_NODE_MODEL_BYTES, MAX_GROUP_AGENT_NODE_MODEL_EVENTS,
    MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES, MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS,
    MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES, MAX_GROUP_AGENT_NODE_RESULT_BYTES,
    MAX_GROUP_AGENT_NODE_TIMEOUT_MS, group_agent_project_lane_sha256, group_agent_prompt_sha256,
};
use url::Url;

const HTTPS_PREFIX: &str = "https://";
use crate::{
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_GRAPH_NODES,
};

pub(super) fn validate_control_snapshot(
    snapshot: &GroupAgentGraphControlSnapshot,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    snapshot
        .plan
        .validate()
        .map_err(|_| invalid("control snapshot has an invalid Core Plan"))?;
    snapshot
        .manifest
        .validate()
        .map_err(|_| invalid("control snapshot has an invalid graph manifest"))?;
    if !valid_control_header(snapshot) || !valid_control_bindings(snapshot) {
        return Err(invalid("invalid Group Agent Graph control snapshot"));
    }
    if snapshot.expected_sha256()? != snapshot.snapshot_sha256 {
        return Err(invalid("control snapshot digest disagrees"));
    }
    let bytes = snapshot.canonical_json()?.len();
    if !(1..=MAX_GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_BYTES).contains(&bytes) {
        return Err(invalid("control snapshot exceeds its byte limit"));
    }
    Ok(())
}

fn valid_control_header(snapshot: &GroupAgentGraphControlSnapshot) -> bool {
    snapshot.v == GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION
        && snapshot.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && snapshot.graph_run_version == GROUP_AGENT_GRAPH_RUN_VERSION
        && valid_identifier(&snapshot.graph_run_id)
        && valid_identifier(&snapshot.graph_id)
        && is_digest(&snapshot.source_snapshot_sha256)
        && is_digest(&snapshot.graph_manifest_sha256)
        && is_digest(&snapshot.core_plan_sha256)
        && snapshot.last_event_seq == 1
        && is_digest(&snapshot.last_event_sha256)
        && !snapshot.execution_contract_present
        && !snapshot.dispatch_authority_released
        && is_digest(&snapshot.snapshot_sha256)
}

fn valid_control_bindings(snapshot: &GroupAgentGraphControlSnapshot) -> bool {
    let authored = snapshot
        .manifest
        .nodes
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<Vec<_>>();
    let planned = snapshot
        .plan
        .authored_node_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    snapshot.graph_id == snapshot.plan.graph_id
        && snapshot.graph_manifest_sha256 == snapshot.plan.graph_manifest_sha256
        && snapshot
            .manifest
            .expected_sha256()
            .is_ok_and(|digest| digest == snapshot.graph_manifest_sha256)
        && snapshot.source_snapshot_sha256 == snapshot.manifest.source.snapshot_sha256
        && snapshot.core_plan_sha256 == snapshot.plan.plan_sha256
        && authored == planned
        && snapshot.manifest.edges == snapshot.plan.edges
        && snapshot.manifest.waves == snapshot.plan.waves
}

pub(super) fn validate_contract(
    contract: &GroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    if !valid_contract_header(contract) {
        return Err(invalid("invalid Node Execution Contract header"));
    }
    validate_node(&contract.node)?;
    validate_workspace(&contract.workspace)?;
    validate_provider(&contract.provider)?;
    validate_request(&contract.request)?;
    validate_budgets(&contract.budgets)?;
    validate_approval(&contract.approval)?;
    validate_result(&contract.result)?;
    validate_failure(&contract.failure)?;
    let expected = contract.expected_sha256()?;
    if contract.contract_sha256 != expected
        || contract.contract_id != format!("node-contract-{expected}")
    {
        return Err(invalid(
            "Node Execution Contract digest or identity disagrees",
        ));
    }
    let bytes = contract.canonical_json()?.len();
    if !(1..=MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES).contains(&bytes) {
        return Err(invalid("Node Execution Contract exceeds its byte limit"));
    }
    Ok(())
}

fn valid_contract_header(contract: &GroupAgentNodeExecutionContract) -> bool {
    contract.v == GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION
        && contract.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && contract.node_execution_protocol_version == GROUP_AGENT_NODE_EXECUTION_PROTOCOL_VERSION
        && valid_identifier(&contract.graph_run_id)
        && valid_identifier(&contract.graph_id)
        && is_digest(&contract.source_snapshot_sha256)
        && is_digest(&contract.graph_manifest_sha256)
        && is_digest(&contract.core_plan_sha256)
        && is_digest(&contract.control_snapshot_sha256)
        && contract.expected_last_event_seq == 1
        && is_digest(&contract.expected_last_event_sha256)
        && contract.execution_contract_present
        && !contract.dispatch_authority_released
        && is_digest(&contract.contract_sha256)
}

fn validate_node(
    node: &GroupAgentNodeExecutionNode,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = valid_identifier(&node.node_id)
        && node.authored_node_index < MAX_GROUP_AGENT_GRAPH_NODES
        && node.topology_wave_index == 0
        && node.attempt == 1
        && valid_identifier(&node.project_id)
        && valid_text(&node.member_role, 64)
        && valid_text(&node.agent_profile, 128)
        && node.project_lane_sha256 == group_agent_project_lane_sha256(&node.project_id)
        && node.same_project_policy == GroupAgentNodeSameProjectPolicy::ExclusiveUntilTerminal;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Execution Contract node"))
}

fn validate_workspace(
    workspace: &GroupAgentNodeExecutionWorkspace,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = workspace.mode == GroupAgentNodeWorkspaceMode::None
        && workspace.root_identity.is_none()
        && workspace.isolation_id.is_none()
        && workspace.allowed_read_paths.is_empty();
    valid
        .then_some(())
        .ok_or_else(|| invalid("Node Execution Contract workspace must be none"))
}

fn validate_provider(
    provider: &GroupAgentNodeExecutionProvider,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = provider.kind == GroupAgentNodeProviderKind::OpenAiResponses
        && valid_text(
            &provider.endpoint,
            MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES,
        )
        && valid_https_endpoint(&provider.endpoint)
        && valid_text(&provider.model, MAX_GROUP_AGENT_NODE_MODEL_BYTES)
        && !provider.store
        && provider.stream;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Execution Contract provider"))
}

fn validate_request(
    request: &GroupAgentNodeExecutionRequest,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = valid_prose(&request.system_prompt)
        && request.system_prompt_bytes == request.system_prompt.len()
        && request.system_prompt_sha256 == group_agent_prompt_sha256(&request.system_prompt)
        && valid_prose(&request.user_prompt)
        && request.user_prompt_bytes == request.user_prompt.len()
        && request.user_prompt_sha256 == group_agent_prompt_sha256(&request.user_prompt)
        && request.predecessor_result_receipts.is_empty()
        && request.tools.is_empty()
        && is_digest(&request.request_sha256);
    if !valid || request.expected_sha256()? != request.request_sha256 {
        return Err(invalid("invalid Node Execution Contract request"));
    }
    Ok(())
}

fn validate_budgets(
    budgets: &GroupAgentNodeExecutionBudgets,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = budgets.max_turns == 1
        && budgets.max_tool_calls == 0
        && (1..=MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS).contains(&budgets.max_output_tokens)
        && (1..=MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES).contains(&budgets.max_model_output_bytes)
        && (1..=MAX_GROUP_AGENT_NODE_MODEL_EVENTS).contains(&budgets.max_model_events)
        && (1..=MAX_GROUP_AGENT_NODE_TIMEOUT_MS).contains(&budgets.timeout_ms)
        && (1..=MAX_GROUP_AGENT_NODE_COST_USD_MICROS).contains(&budgets.max_cost_usd_micros)
        && is_digest(&budgets.pricing_snapshot_sha256);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Execution Contract budgets"))
}

fn validate_approval(
    approval: &GroupAgentNodeExecutionApproval,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = approval.provider_dispatch
        == GroupAgentNodeProviderApproval::FreshOffMachineConsent
        && approval.workspace == GroupAgentNodeEffectApproval::Forbidden
        && approval.tools == GroupAgentNodeEffectApproval::Forbidden
        && approval.writeback == GroupAgentNodeEffectApproval::Forbidden;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Execution Contract approval policy"))
}

fn validate_result(
    result: &GroupAgentNodeExecutionResultPolicy,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = result.artifact_kind == GroupAgentNodeArtifactKind::LocalGraphNodeArtifact
        && (1..=MAX_GROUP_AGENT_NODE_RESULT_BYTES).contains(&result.max_result_bytes)
        && result.predecessor_dataflow == GroupAgentNodeDataflowPolicy::None
        && result.conversation_writeback == GroupAgentNodeWritebackPolicy::None
        && result.prompt_writeback == GroupAgentNodeWritebackPolicy::None
        && result.memory_writeback == GroupAgentNodeWritebackPolicy::None;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Execution Contract result policy"))
}

fn validate_failure(
    failure: &GroupAgentNodeExecutionFailurePolicy,
) -> Result<(), GroupAgentNodeExecutionValidationError> {
    let valid = !failure.automatic_retry
        && !failure.lease_retry
        && failure.post_claim_uncertainty == GroupAgentNodePostClaimUncertainty::DispatchUnknown
        && failure.failure_propagation_owner == GroupAgentNodeFailurePropagationOwner::ForgeCore;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Node Execution Contract failure policy"))
}

fn valid_https_endpoint(value: &str) -> bool {
    let Some(rest) = value.strip_prefix(HTTPS_PREFIX) else {
        return false;
    };
    let (authority, path) = authority_and_path(rest);
    let Some((host, port)) = split_authority(authority) else {
        return false;
    };
    if !valid_host(host) || port.is_some_and(|value| !valid_port(value)) || !valid_path(path) {
        return false;
    }
    exact_url(value, host, port, path)
}

fn authority_and_path(value: &str) -> (&str, &str) {
    value.split_once('/').map_or((value, ""), |(authority, _)| {
        (authority, &value[authority.len()..])
    })
}

fn split_authority(authority: &str) -> Option<(&str, Option<&str>)> {
    if authority.is_empty() || authority.matches(':').count() > 1 {
        return None;
    }
    let Some((host, port)) = authority.split_once(':') else {
        return Some((authority, None));
    };
    (!host.is_empty() && !port.is_empty()).then_some((host, Some(port)))
}

fn valid_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 {
        return false;
    }
    if host
        .bytes()
        .all(|byte| byte == b'.' || byte.is_ascii_digit())
    {
        return host
            .parse::<Ipv4Addr>()
            .is_ok_and(|address| address.to_string() == host);
    }
    let labels = host.split('.').collect::<Vec<_>>();
    labels.last().is_some_and(|label| {
        label.bytes().any(|byte| byte.is_ascii_lowercase())
            && labels.iter().all(|label| valid_dns_label(label))
    })
}

fn valid_dns_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 63
        || !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit()
        || !bytes[bytes.len() - 1].is_ascii_lowercase() && !bytes[bytes.len() - 1].is_ascii_digit()
    {
        return false;
    }
    bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn valid_port(port: &str) -> bool {
    port != "443"
        && !port.starts_with('0')
        && port.bytes().all(|byte| byte.is_ascii_digit())
        && port.parse::<u16>().is_ok_and(|value| value > 0)
}

fn valid_path(path: &str) -> bool {
    (path.is_empty() || path.starts_with('/'))
        && path.split('/').all(|segment| {
            segment != "." && segment != ".." && segment.bytes().all(valid_path_byte)
        })
}

fn valid_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn exact_url(value: &str, host: &str, port: Option<&str>, path: &str) -> bool {
    let Ok(parsed) = Url::parse(value) else {
        return false;
    };
    let exact = parsed.as_str() == value
        || (path.is_empty() && parsed.as_str().strip_suffix('/') == Some(value));
    parsed.scheme() == "https"
        && parsed.host_str() == Some(host)
        && parsed.port().map(|value| value.to_string()).as_deref() == port
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && exact
}

pub(super) fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

pub(super) fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(unsupported_character)
}

fn valid_prose(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.chars().any(|character| {
            (character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
                || is_bidi_control(character)
        })
}

fn unsupported_character(value: char) -> bool {
    value.is_control() || is_bidi_control(value)
}

fn is_bidi_control(value: char) -> bool {
    matches!(
        value,
        '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn invalid(message: &str) -> GroupAgentNodeExecutionValidationError {
    GroupAgentNodeExecutionValidationError {
        message: message.into(),
    }
}
