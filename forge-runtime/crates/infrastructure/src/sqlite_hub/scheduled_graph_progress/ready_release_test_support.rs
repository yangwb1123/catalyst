use crate::runtime_domain::{
    AdmitGroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractScope,
    GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestStore,
    GroupAgentScheduledNodeSuccessorStore, GroupAgentScheduledNodeTerminalArtifact,
    GroupAgentScheduledNodeTerminalReceipt, group_agent_prompt_sha256,
    group_agent_scheduled_node_user_prompt, group_agent_scheduled_node_user_prompt_with_output,
};

use super::super::{
    read::atomicity_fixture::{
        ClaimedFixture, claim_prepared, claim_ready, diamond_ready_fixture, terminalize_prepared,
    },
    sqlite_group_agent_scheduled_node_provider_request_support as provider_support,
};

pub(super) struct DiamondClosureFixture {
    pub(super) claimed: ClaimedFixture,
    pub(super) receipts: Vec<GroupAgentScheduledNodeTerminalReceipt>,
    pub(super) artifacts: Vec<GroupAgentScheduledNodeTerminalArtifact>,
}

pub(super) fn terminalized_diamond() -> DiamondClosureFixture {
    let claimed = claim_ready(diamond_ready_fixture());
    let first = claimed.terminalize().expect("terminalize frontend");
    let first_receipt = receipt(&first.inspection);
    let first_artifact = artifact(&first.inspection);
    let backend = prepare_successor(&claimed, 1, &[], None, "backend");
    let (release, claim) = claim_prepared(
        &claimed.graph,
        &claimed.initial_admission,
        &backend,
        &claimed.pricing,
    )
    .expect("claim backend successor");
    let second = terminalize_prepared(&claimed.graph, &release, &claim)
        .expect("terminalize backend successor");
    DiamondClosureFixture {
        claimed,
        receipts: vec![first_receipt, receipt(&second.inspection)],
        artifacts: vec![first_artifact, artifact(&second.inspection)],
    }
}

pub(super) fn prepare_sso_with_content(
    fixture: &DiamondClosureFixture,
) -> GroupAgentScheduledNodeProviderRequestInspection {
    prepare_successor(
        &fixture.claimed,
        2,
        &fixture.receipts,
        fixture.artifacts.first(),
        "sso",
    )
}

fn prepare_successor(
    fixture: &ClaimedFixture,
    ordinal: usize,
    receipts: &[GroupAgentScheduledNodeTerminalReceipt],
    content: Option<&GroupAgentScheduledNodeTerminalArtifact>,
    key: &str,
) -> GroupAgentScheduledNodeProviderRequestInspection {
    let admission = successor_admission(fixture, ordinal, receipts, content, key);
    let stored = fixture
        .graph
        .store
        .admit_group_agent_scheduled_node_successor(&admission)
        .expect("admit exact successor")
        .inspection;
    let request = provider_support::request(&stored, &format!("{key}-provider-key"), 100);
    fixture
        .graph
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("prepare successor provider request")
        .inspection
}

fn successor_admission(
    fixture: &ClaimedFixture,
    ordinal: usize,
    receipts: &[GroupAgentScheduledNodeTerminalReceipt],
    content: Option<&GroupAgentScheduledNodeTerminalArtifact>,
    key: &str,
) -> AdmitGroupAgentScheduledNodeContractCandidate {
    let source = &fixture.initial_admission;
    let scheduled = source
        .schedule
        .nodes
        .get(ordinal)
        .expect("successor schedule node");
    let manifest = source
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == scheduled.node_id)
        .expect("successor manifest node");
    let mut candidate = source.candidate.clone();
    candidate.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    bind_node(&mut candidate, scheduled, manifest);
    bind_request(&mut candidate, scheduled, manifest, receipts, content);
    provider_support::resign_candidate_digests(&mut candidate);
    candidate
        .validate_against_control_and_schedule(&source.control_snapshot, &source.schedule)
        .expect("valid exact successor candidate");
    let mut admission = source.clone();
    admission.candidate_json = candidate
        .canonical_json()
        .expect("successor candidate JSON");
    admission.candidate = candidate;
    admission.idempotency_key = format!("{key}-successor-key");
    admission.admitted_at_ms = 90 + ordinal as u64;
    admission.validate().expect("valid successor admission");
    admission
}

fn bind_node(
    candidate: &mut crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
    scheduled: &crate::runtime_domain::GroupAgentGraphExecutionScheduleNode,
    manifest: &crate::runtime_domain::GroupAgentGraphNode,
) {
    candidate.node.execution_ordinal = scheduled.execution_ordinal;
    candidate.node.node_id.clone_from(&scheduled.node_id);
    candidate.node.authored_node_index = scheduled.authored_node_index;
    candidate.node.topology_wave_index = scheduled.topology_wave_index;
    candidate.node.attempt = scheduled.attempt;
    candidate.node.project_id.clone_from(&manifest.project_id);
    candidate.node.member_role.clone_from(&manifest.member_role);
    candidate
        .node
        .agent_profile
        .clone_from(&manifest.agent_profile);
    candidate
        .node
        .project_lane_sha256
        .clone_from(&scheduled.project_lane_sha256);
}

fn bind_request(
    candidate: &mut crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
    scheduled: &crate::runtime_domain::GroupAgentGraphExecutionScheduleNode,
    manifest: &crate::runtime_domain::GroupAgentGraphNode,
    receipts: &[GroupAgentScheduledNodeTerminalReceipt],
    content: Option<&GroupAgentScheduledNodeTerminalArtifact>,
) {
    candidate.request.execution_ordinal = scheduled.execution_ordinal;
    candidate.request.node_id.clone_from(&scheduled.node_id);
    candidate
        .request
        .required_predecessor_node_ids
        .clone_from(&scheduled.direct_predecessor_node_ids);
    candidate.request.predecessor_terminal_receipts =
        receipts.iter().map(compact_receipt).collect();
    candidate.request.predecessor_content_included = content.is_some();
    candidate.request.user_prompt = successor_prompt(manifest, content);
    candidate.request.user_prompt_bytes = candidate.request.user_prompt.len();
    candidate.request.user_prompt_sha256 =
        group_agent_prompt_sha256(&candidate.request.user_prompt);
}

fn successor_prompt(
    manifest: &crate::runtime_domain::GroupAgentGraphNode,
    content: Option<&GroupAgentScheduledNodeTerminalArtifact>,
) -> String {
    match content {
        Some(artifact) => group_agent_scheduled_node_user_prompt_with_output(
            &manifest.node_id,
            &manifest.task,
            &manifest.acceptance,
            &artifact.output_text,
        ),
        None => group_agent_scheduled_node_user_prompt(
            &manifest.node_id,
            &manifest.task,
            &manifest.acceptance,
        ),
    }
    .expect("successor user Prompt")
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

fn receipt(
    inspection: &crate::runtime_domain::GroupAgentScheduledNodeLifecycleInspection,
) -> GroupAgentScheduledNodeTerminalReceipt {
    inspection
        .terminal_receipt
        .clone()
        .expect("terminal lifecycle receipt")
}

fn artifact(
    inspection: &crate::runtime_domain::GroupAgentScheduledNodeLifecycleInspection,
) -> GroupAgentScheduledNodeTerminalArtifact {
    inspection
        .artifact
        .clone()
        .expect("terminal lifecycle artifact")
}
