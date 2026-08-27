use serde_json::Value;

use super::*;
use crate::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentGraphExecutionScheduleNode,
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeContractScope, GroupAgentScheduledNodeLifecycleStatus,
    GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
    GroupAgentScheduledNodeTerminalArtifactKind, ScheduledGraphProgressNode,
    ScheduledGraphReconcileDisposition, group_agent_node_provider_request_sha256,
    group_agent_prompt_sha256, group_agent_scheduled_node_provider_request_id,
    group_agent_scheduled_node_terminal_artifact_id,
    group_agent_scheduled_node_terminal_output_sha256,
    group_agent_scheduled_node_terminal_receipt_id, group_agent_scheduled_node_user_prompt,
    group_agent_scheduled_node_user_prompt_with_output,
};

const BODY: &str = "{}";

pub(super) fn initial_control() -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let legacy = legacy_control();
    let progress = progress_snapshot(&legacy, 0, &[]);
    ready_control(legacy, progress, 0, Vec::new(), None)
}

pub(super) fn successor_control(
    content: Option<&str>,
) -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let legacy = legacy_control();
    let target = 2;
    let mut artifacts = Vec::new();
    let receipts = legacy.schedule.nodes[..target]
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let provider = synthetic_provider_id(index);
            let artifact = (index == 0)
                .then(|| content.map(|text| content_artifact(node, &provider, text)))
                .flatten();
            let receipt = terminal_receipt(node, &provider, artifact.as_ref(), index);
            artifacts.push(artifact);
            receipt
        })
        .collect::<Vec<_>>();
    let candidate = successor_candidate(&legacy, target, &receipts, content);
    let provider = provider_request(&legacy, &candidate);
    let mut source = legacy;
    source.scheduled_contract_record = contract_record(&candidate);
    source.scheduled_contract = candidate;
    source.provider_request = provider;
    source.provider_request_json = BODY.into();
    let progress = progress_snapshot(&source, target, &receipts);
    let artifact = content.and_then(|_| artifacts.into_iter().next().flatten());
    ready_control(source, progress, target, receipts, artifact)
}

fn legacy_control() -> crate::GroupAgentScheduledNodeDispatchReleaseControl {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json"
    )))
    .expect("legacy release fixture");
    crate::GroupAgentScheduledNodeDispatchReleaseControl::decode_exact(
        fixture["canonical_release_control_json"]
            .as_str()
            .expect("legacy release JSON"),
    )
    .expect("valid legacy release fixture")
}

fn ready_control(
    legacy: crate::GroupAgentScheduledNodeDispatchReleaseControl,
    progress: ScheduledGraphProgressSnapshot,
    target: usize,
    receipts: Vec<GroupAgentScheduledNodeTerminalReceipt>,
    artifact: Option<GroupAgentScheduledNodeTerminalArtifact>,
) -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let decision = ScheduledGraphReconcileDecision {
        v: crate::SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
        progress_protocol_version: crate::SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: progress.graph_run_id.clone(),
        schedule_id: progress.schedule_id.clone(),
        schedule_sha256: progress.schedule_sha256.clone(),
        snapshot_sha256: progress.snapshot_sha256.clone(),
        disposition: ScheduledGraphReconcileDisposition::Ready,
        next_execution_ordinal: Some(target),
        next_node_id: Some(progress.nodes[target].node_id.clone()),
        decision_sha256: String::new(),
    }
    .seal()
    .expect("seal ready decision");
    GroupAgentScheduledReadyNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: legacy.graph_run,
        journal_events: legacy.journal_events,
        control_snapshot: legacy.control_snapshot,
        schedule_record: legacy.schedule_record,
        schedule: legacy.schedule,
        progress_snapshot: progress,
        reconcile_decision: decision,
        scheduled_contract_record: legacy.scheduled_contract_record,
        scheduled_contract: legacy.scheduled_contract,
        direct_predecessor_receipts: receipts,
        predecessor_content_artifact: artifact,
        provider_request: legacy.provider_request,
        provider_request_json: legacy.provider_request_json,
        snapshot_sha256: String::new(),
    }
    .seal()
    .expect("seal ready release control")
}

fn successor_candidate(
    legacy: &crate::GroupAgentScheduledNodeDispatchReleaseControl,
    target: usize,
    receipts: &[GroupAgentScheduledNodeTerminalReceipt],
    content: Option<&str>,
) -> GroupAgentScheduledNodeContractCandidate {
    let mut value = legacy.scheduled_contract.clone();
    let scheduled = &legacy.schedule.nodes[target];
    let source = legacy
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == scheduled.node_id)
        .expect("target source node");
    value.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    value.node.execution_ordinal = target;
    value.node.node_id.clone_from(&scheduled.node_id);
    value.node.authored_node_index = scheduled.authored_node_index;
    value.node.topology_wave_index = scheduled.topology_wave_index;
    value.node.project_id.clone_from(&source.project_id);
    value.node.member_role.clone_from(&source.member_role);
    value.node.agent_profile.clone_from(&source.agent_profile);
    value
        .node
        .project_lane_sha256
        .clone_from(&scheduled.project_lane_sha256);
    value.request.execution_ordinal = target;
    value.request.node_id.clone_from(&scheduled.node_id);
    value
        .request
        .required_predecessor_node_ids
        .clone_from(&scheduled.direct_predecessor_node_ids);
    value.request.predecessor_terminal_receipts = receipts.iter().map(compact_receipt).collect();
    value.request.predecessor_content_included = content.is_some();
    value.request.user_prompt = successor_prompt(source, content);
    value.request.user_prompt_bytes = value.request.user_prompt.len();
    value.request.user_prompt_sha256 = group_agent_prompt_sha256(&value.request.user_prompt);
    resign_candidate(&mut value);
    value
        .validate_against_control_and_schedule(&legacy.control_snapshot, &legacy.schedule)
        .expect("source-bound successor candidate");
    value
}

fn successor_prompt(source: &crate::GroupAgentGraphNode, content: Option<&str>) -> String {
    match content {
        Some(output) => group_agent_scheduled_node_user_prompt_with_output(
            &source.node_id,
            &source.task,
            &source.acceptance,
            output,
        ),
        None => group_agent_scheduled_node_user_prompt(
            &source.node_id,
            &source.task,
            &source.acceptance,
        ),
    }
    .expect("successor user Prompt")
}

fn resign_candidate(value: &mut GroupAgentScheduledNodeContractCandidate) {
    value.request.request_id.clear();
    value.request.request_sha256.clear();
    value.request.request_sha256 = value.request.expected_sha256().expect("request digest");
    value.request.request_id = format!("scheduled-node-request-{}", value.request.request_sha256);
    value.contract_id.clear();
    value.contract_sha256.clear();
    value.contract_sha256 = value.expected_sha256().expect("candidate digest");
    value.contract_id = format!("scheduled-node-contract-{}", value.contract_sha256);
}

fn contract_record(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> GroupAgentScheduledNodeContractRecord {
    GroupAgentScheduledNodeContractRecord {
        v: candidate.v,
        contract_id: candidate.contract_id.clone(),
        graph_run_id: candidate.graph_run_id.clone(),
        schedule_id: candidate.schedule_id.clone(),
        node_id: candidate.node.node_id.clone(),
        execution_ordinal: candidate.node.execution_ordinal,
        attempt: candidate.node.attempt,
        control_snapshot_sha256: candidate.control_snapshot_sha256.clone(),
        schedule_sha256: candidate.schedule_sha256.clone(),
        contract_sha256: candidate.contract_sha256.clone(),
        contract_bytes: candidate.canonical_json().expect("candidate JSON").len(),
        request_id: candidate.request.request_id.clone(),
        request_sha256: candidate.request.request_sha256.clone(),
        project_lane_sha256: candidate.node.project_lane_sha256.clone(),
        expected_last_event_seq: candidate.expected_last_event_seq,
        expected_last_event_sha256: candidate.expected_last_event_sha256.clone(),
        predecessor_receipt_count: candidate.request.predecessor_terminal_receipts.len(),
        lifecycle_contract_admitted: false,
        provider_request_present: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        progress_observed: false,
        successor_advance_authorized: false,
        created_at_ms: 75,
    }
}

fn provider_request(
    legacy: &crate::GroupAgentScheduledNodeDispatchReleaseControl,
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> GroupAgentScheduledNodeProviderRequestRecord {
    let mut value = legacy.provider_request.clone();
    value.provider_request_id.clear();
    value.schedule_id.clone_from(&candidate.schedule_id);
    value
        .scheduled_contract_id
        .clone_from(&candidate.contract_id);
    value.execution_ordinal = candidate.node.execution_ordinal;
    value.node_id.clone_from(&candidate.node.node_id);
    value
        .scheduled_contract_sha256
        .clone_from(&candidate.contract_sha256);
    value
        .logical_request_id
        .clone_from(&candidate.request.request_id);
    value
        .logical_request_sha256
        .clone_from(&candidate.request.request_sha256);
    value
        .project_lane_sha256
        .clone_from(&candidate.node.project_lane_sha256);
    value.provider_request_sha256 = group_agent_node_provider_request_sha256(BODY.as_bytes());
    value.provider_request_bytes = BODY.len();
    value.prepared_request_sha256.clear();
    value.prepared_request_sha256 = value.expected_sha256().expect("prepared request digest");
    value.provider_request_id =
        group_agent_scheduled_node_provider_request_id(&value.prepared_request_sha256);
    value.validate().expect("valid successor provider request");
    value
}

fn progress_snapshot(
    legacy: &crate::GroupAgentScheduledNodeDispatchReleaseControl,
    target: usize,
    completed: &[GroupAgentScheduledNodeTerminalReceipt],
) -> ScheduledGraphProgressSnapshot {
    let mut nodes = legacy
        .schedule
        .nodes
        .iter()
        .map(|node| ScheduledGraphProgressNode {
            execution_ordinal: node.execution_ordinal,
            node_id: node.node_id.clone(),
            attempt: node.attempt,
            candidate_id: None,
            candidate_sha256: None,
            provider_request_id: None,
            prepared_request_sha256: None,
            lifecycle_status: None,
            terminal_outcome: None,
            terminal_receipt_sha256: None,
        })
        .collect::<Vec<_>>();
    for (ordinal, receipt) in completed.iter().enumerate() {
        project_completed(&mut nodes[ordinal], receipt, ordinal);
    }
    project_selected(&mut nodes[target], legacy);
    seal_progress(legacy, nodes)
}

fn project_completed(
    node: &mut ScheduledGraphProgressNode,
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
    ordinal: usize,
) {
    let digest = indexed_digest(1_000 + ordinal);
    node.candidate_id = Some(format!("scheduled-node-contract-{digest}"));
    node.candidate_sha256 = Some(digest);
    node.provider_request_id = Some(receipt.provider_request_id.clone());
    node.prepared_request_sha256 = Some(
        receipt
            .provider_request_id
            .strip_prefix("scheduled-node-provider-request-")
            .expect("provider prefix")
            .into(),
    );
    node.lifecycle_status = Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized);
    node.terminal_outcome = Some(GroupAgentNodeTerminalOutcome::Completed);
    node.terminal_receipt_sha256 = Some(receipt.receipt_sha256.clone());
}

fn project_selected(
    node: &mut ScheduledGraphProgressNode,
    legacy: &crate::GroupAgentScheduledNodeDispatchReleaseControl,
) {
    let candidate = &legacy.scheduled_contract;
    let provider = &legacy.provider_request;
    node.candidate_id = Some(candidate.contract_id.clone());
    node.candidate_sha256 = Some(candidate.contract_sha256.clone());
    node.provider_request_id = Some(provider.provider_request_id.clone());
    node.prepared_request_sha256 = Some(provider.prepared_request_sha256.clone());
}

fn seal_progress(
    legacy: &crate::GroupAgentScheduledNodeDispatchReleaseControl,
    nodes: Vec<ScheduledGraphProgressNode>,
) -> ScheduledGraphProgressSnapshot {
    ScheduledGraphProgressSnapshot {
        v: crate::SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
        progress_protocol_version: crate::SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: legacy.graph_run.graph_run_id.clone(),
        graph_id: legacy.graph_run.graph_id.clone(),
        schedule_id: legacy.schedule.schedule_id.clone(),
        schedule_sha256: legacy.schedule.schedule_sha256.clone(),
        node_count: legacy.schedule.node_count,
        execution_mode: legacy.schedule.execution_mode,
        max_in_flight_nodes: legacy.schedule.max_in_flight_nodes,
        progression_policy: legacy.schedule.progression_policy,
        attempt_policy: legacy.schedule.attempt_policy,
        failure_policy: legacy.schedule.failure_policy,
        nodes,
        snapshot_sha256: String::new(),
    }
    .seal()
    .expect("seal progress")
}

fn terminal_receipt(
    node: &GroupAgentGraphExecutionScheduleNode,
    provider_id: &str,
    artifact: Option<&GroupAgentScheduledNodeTerminalArtifact>,
    index: usize,
) -> GroupAgentScheduledNodeTerminalReceipt {
    let artifact_sha = artifact.map_or_else(
        || indexed_digest(3_000 + index),
        |v| v.artifact_sha256.clone(),
    );
    let mut value = GroupAgentScheduledNodeTerminalReceipt {
        v: crate::GROUP_AGENT_SCHEDULED_NODE_TERMINAL_RECEIPT_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        terminal_receipt_protocol_version:
            crate::GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        terminal_control_sha256: indexed_digest(4_000 + index),
        graph_run_id: "graph-run-fixture-v1".into(),
        graph_id: "graph-fixture-v1".into(),
        node_id: node.node_id.clone(),
        attempt: node.attempt,
        dispatch_id: format!("dispatch-{}", node.node_id),
        provider_request_id: provider_id.into(),
        project_lane_sha256: node.project_lane_sha256.clone(),
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Result,
        artifact_id: group_agent_scheduled_node_terminal_artifact_id(&artifact_sha),
        artifact_sha256: artifact_sha,
        node_outcome: GroupAgentNodeTerminalOutcome::Completed,
        retry_authorized: false,
        lane_release_authorized: true,
        successor_advance_authorized: false,
        receipt_id: String::new(),
        receipt_sha256: String::new(),
    };
    value.receipt_sha256 = value.expected_sha256().expect("receipt digest");
    value.receipt_id = group_agent_scheduled_node_terminal_receipt_id(&value.receipt_sha256);
    value.validate().expect("valid direct receipt");
    value
}

fn content_artifact(
    node: &GroupAgentGraphExecutionScheduleNode,
    provider_id: &str,
    output: &str,
) -> GroupAgentScheduledNodeTerminalArtifact {
    let mut value = GroupAgentScheduledNodeTerminalArtifact {
        v: crate::GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION,
        terminal_artifact_protocol_version:
            crate::GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Result,
        graph_run_id: "graph-run-fixture-v1".into(),
        node_id: node.node_id.clone(),
        attempt: node.attempt,
        dispatch_id: format!("dispatch-{}", node.node_id),
        provider_request_id: provider_id.into(),
        claim_event_sha256: indexed_digest(5_000),
        authorization_sha256: indexed_digest(5_001),
        provider_request_sha256: indexed_digest(5_002),
        request_body_sha256: indexed_digest(5_003),
        pricing_snapshot_sha256: indexed_digest(5_004),
        lane_ownership_id: "lane-owner-fixture".into(),
        project_lane_sha256: node.project_lane_sha256.clone(),
        provider_poll_started: true,
        terminal_seen: true,
        stream_eof_seen: true,
        classification: GroupAgentNodeTerminalClassification::Completed,
        output_text: output.into(),
        output_bytes: output.len(),
        output_sha256: group_agent_scheduled_node_terminal_output_sha256(output),
        usage_observed: true,
        input_tokens: 1,
        output_tokens: 1,
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms: 77,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    };
    seal_artifact(&mut value);
    value
}

fn seal_artifact(value: &mut GroupAgentScheduledNodeTerminalArtifact) {
    value.artifact_sha256 = value.expected_sha256().expect("artifact digest");
    value.artifact_id = group_agent_scheduled_node_terminal_artifact_id(&value.artifact_sha256);
    loop {
        let bytes = value.canonical_json().expect("artifact JSON").len();
        if value.artifact_bytes == bytes {
            break;
        }
        value.artifact_bytes = bytes;
    }
    value.validate().expect("valid content artifact");
}

fn compact_receipt(
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
) -> GroupAgentScheduledNodePredecessorReceipt {
    GroupAgentScheduledNodePredecessorReceipt {
        predecessor_node_id: receipt.node_id.clone(),
        predecessor_attempt: receipt.attempt,
        terminal_event_seq: 0,
        terminal_event_sha256: String::new(),
        terminal_receipt_id: receipt.receipt_id.clone(),
        terminal_receipt_sha256: receipt.receipt_sha256.clone(),
        node_outcome: GroupAgentScheduledNodePredecessorOutcome::Completed,
        provider_request_id: receipt.provider_request_id.clone(),
        dispatch_id: receipt.dispatch_id.clone(),
    }
}

fn synthetic_provider_id(index: usize) -> String {
    group_agent_scheduled_node_provider_request_id(&indexed_digest(2_000 + index))
}

fn indexed_digest(index: usize) -> String {
    format!("{index:064x}")
}
