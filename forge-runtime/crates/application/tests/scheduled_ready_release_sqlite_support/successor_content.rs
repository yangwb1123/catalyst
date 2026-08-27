use std::sync::Arc;

use forge_runtime_domain::{
    ClaimGroupAgentScheduledNodeDispatchResult, GroupAgentGraphExecutionScheduleStore,
    GroupAgentScheduledNodeContractScope, GroupAgentScheduledNodeContractStore,
    GroupAgentScheduledNodeLifecycleStore, GroupAgentScheduledNodePredecessorOutcome,
    GroupAgentScheduledNodePredecessorReceipt, GroupAgentScheduledNodeProviderRequestStore,
    GroupAgentScheduledNodeSuccessorStore, GroupAgentScheduledNodeTerminalArtifact,
    GroupAgentScheduledNodeTerminalReceipt, group_agent_scheduled_node_user_prompt_with_output,
};
use forge_runtime_infrastructure::SqliteHubStore;

use super::{
    Fixture, GraphFixture, claim_request, legacy_authorization_support, predecessor_terminal,
    pricing_snapshot, provider_support, release_control, schedule_support,
};

pub(super) fn fixture() -> Fixture {
    let graph = schedule_support::prepared_fixture();
    let schedule_request = schedule_support::request(&graph, "schedule-key", 40);
    let schedule = graph
        .store
        .admit_group_agent_graph_execution_schedule(&schedule_request)
        .expect("admit predecessor-content schedule")
        .inspection;
    let mut admission = super::contract_support::admission(schedule_request, "contract-key", 50);
    let pricing = pricing_snapshot(&admission);
    admission
        .candidate
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&pricing.pricing_snapshot_sha256);
    provider_support::resign_candidate_digests(&mut admission.candidate);
    admission.candidate_json = admission
        .candidate
        .canonical_json()
        .expect("repriced predecessor candidate JSON");
    let initial = graph
        .store
        .admit_group_agent_scheduled_node_contract(&admission)
        .expect("admit predecessor contract")
        .inspection;
    let provider = prepare_provider(&graph, &initial, "predecessor-provider-key", 60);
    let release = release_control(&graph, &admission, schedule, &provider);
    let authorization = legacy_authorization_support::authorization(&release, &pricing);
    let claim = claim_request(release.clone(), authorization, pricing.clone(), &provider);
    let (receipt, artifact) = terminalize_predecessor(&graph, &release, &claim);
    prepare_content_successor(&graph, &admission, &receipt, &artifact);
    let reader = Arc::new(
        SqliteHubStore::open_existing_current_live_read_only(&graph.database)
            .expect("open exact-current predecessor-content reader"),
    );
    Fixture {
        graph,
        reader,
        claim,
        pricing_json: pricing.canonical_json().expect("content pricing JSON"),
    }
}

fn prepare_provider(
    graph: &GraphFixture,
    contract: &forge_runtime_domain::GroupAgentScheduledNodeContractInspection,
    key: &str,
    prepared_at_ms: u64,
) -> forge_runtime_domain::GroupAgentScheduledNodeProviderRequestInspection {
    let request = provider_support::request(contract, key, prepared_at_ms);
    graph
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("prepare predecessor provider request")
        .inspection
}

fn terminalize_predecessor(
    graph: &GraphFixture,
    release: &forge_runtime_domain::GroupAgentScheduledNodeDispatchReleaseControl,
    request: &forge_runtime_domain::ClaimGroupAgentScheduledNodeDispatch,
) -> (
    GroupAgentScheduledNodeTerminalReceipt,
    GroupAgentScheduledNodeTerminalArtifact,
) {
    let claimed = graph
        .store
        .claim_group_agent_scheduled_node_dispatch(request)
        .expect("claim predecessor dispatch");
    let ClaimGroupAgentScheduledNodeDispatchResult::Claimed { authority } = claimed else {
        panic!("predecessor dispatch must be newly claimed");
    };
    let (claim, _) = authority.into_parts();
    let terminal = predecessor_terminal::terminal_request(release, &claim);
    let inspection = graph
        .store
        .terminalize_group_agent_scheduled_node_dispatch(&terminal)
        .expect("terminalize predecessor dispatch")
        .inspection;
    (
        inspection.terminal_receipt.expect("predecessor receipt"),
        inspection.artifact.expect("predecessor artifact"),
    )
}

fn prepare_content_successor(
    graph: &GraphFixture,
    source: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
) {
    let node = provider_support::schedule_node(source, 1);
    let mut candidate = source.candidate.clone();
    candidate.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    provider_support::bind_backend_node(&mut candidate, source, &node);
    candidate.request.required_predecessor_node_ids = node.direct_predecessor_node_ids;
    candidate.request.predecessor_terminal_receipts = vec![compact_receipt(receipt)];
    candidate.request.predecessor_content_included = true;
    candidate.request.user_prompt = successor_prompt(source, &node.node_id, artifact);
    provider_support::resign_candidate_digests(&mut candidate);
    let mut admission = source.clone();
    admission.candidate_json = candidate.canonical_json().expect("content successor JSON");
    admission.candidate = candidate;
    admission.idempotency_key = "content-successor-key".into();
    admission.admitted_at_ms = 90;
    admission
        .validate()
        .expect("valid content successor admission");
    let successor = graph
        .store
        .admit_group_agent_scheduled_node_successor(&admission)
        .expect("admit content successor")
        .inspection;
    prepare_provider(graph, &successor, "content-provider-key", 100);
}

fn successor_prompt(
    source: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    node_id: &str,
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
) -> String {
    let manifest = source
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .expect("successor manifest node");
    group_agent_scheduled_node_user_prompt_with_output(
        &manifest.node_id,
        &manifest.task,
        &manifest.acceptance,
        &artifact.output_text,
    )
    .expect("content successor user Prompt")
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
