#![allow(dead_code)]

#[path = "sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
mod sqlite_group_agent_graph_execution_schedule_support;
#[path = "sqlite_group_agent_graph_run_support/mod.rs"]
mod sqlite_group_agent_graph_run_support;
#[path = "sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_contract_support;
#[path = "sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_provider_request_support;

use forge_runtime_domain::{
    GroupAgentScheduledNodeProviderRequestStore, GroupAgentScheduledNodeSuccessorStore,
    HubStoreError,
};

fn successor_admit_request(
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
) -> (
    forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
) {
    use forge_runtime_domain::GroupAgentScheduledNodeContractScope;
    let mut successor = candidate.candidate.clone();
    successor.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    let backend = schedule_backend_node(candidate);
    successor.node.execution_ordinal = 1;
    successor.node.node_id.clone_from(&backend.node_id);
    successor.node.authored_node_index = backend.authored_node_index;
    successor.node.topology_wave_index = backend.topology_wave_index;
    successor
        .node
        .project_id
        .clone_from(&manifest_project_id(candidate, &backend.node_id));
    successor
        .node
        .project_lane_sha256
        .clone_from(&backend.project_lane_sha256);
    successor.request.execution_ordinal = 1;
    successor.request.node_id.clone_from(&backend.node_id);
    resign_successor_fields(
        &mut successor,
        &candidate.schedule.initial_node,
        &manifest_task(candidate, &backend.node_id),
        &backend,
    );
    let successor_json = successor.canonical_json().expect("successor JSON");
    let admit = forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate {
        v: candidate.v,
        graph_run_id: candidate.graph_run_id.clone(),
        control_snapshot: candidate.control_snapshot.clone(),
        control_snapshot_json: candidate.control_snapshot_json.clone(),
        schedule: candidate.schedule.clone(),
        schedule_json: candidate.schedule_json.clone(),
        candidate: successor.clone(),
        candidate_json: successor_json,
        idempotency_key: "successor-admit-key".into(),
        admitted_at_ms: 70,
    };
    (admit, successor)
}

/// Resolves the ordinal-1 schedule node for the successor fixture.
fn manifest_project_id(
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    node_id: &str,
) -> String {
    candidate
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .map(|node| node.project_id.clone())
        .expect("manifest project")
}

fn manifest_task(
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    node_id: &str,
) -> (String, String) {
    candidate
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .map(|node| (node.task.clone(), node.acceptance.clone()))
        .expect("manifest task")
}

/// Resolves the ordinal-1 schedule node for the successor fixture.
fn schedule_backend_node(
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
) -> forge_runtime_domain::GroupAgentGraphExecutionScheduleNode {
    candidate
        .schedule
        .nodes
        .iter()
        .find(|node| node.execution_ordinal == 1)
        .expect("ordinal-1 schedule node")
        .clone()
}

/// Builds the backend user Prompt and re-signs every digest after the
/// successor mutation.
fn resign_successor_fields(
    successor: &mut forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
    initial_node: &str,
    task: &(String, String),
    backend: &forge_runtime_domain::GroupAgentGraphExecutionScheduleNode,
) {
    use forge_runtime_domain::{
        GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
        group_agent_prompt_sha256,
    };
    successor.request.user_prompt = format!(
        "{{\"v\":2,\"node_id\":\"{}\",\"task\":\"{}\",\"acceptance\":\"{}\"}}",
        backend.node_id, task.0, task.1
    );
    successor.request.predecessor_terminal_receipts =
        vec![GroupAgentScheduledNodePredecessorReceipt {
            predecessor_node_id: initial_node.to_owned(),
            predecessor_attempt: 1,
            terminal_event_seq: 0,
            terminal_event_sha256: String::new(),
            terminal_receipt_id: format!("scheduled-node-terminal-receipt-{}", "a".repeat(64)),
            terminal_receipt_sha256: "a".repeat(64),
            node_outcome: GroupAgentScheduledNodePredecessorOutcome::Completed,
            provider_request_id: "scheduled-node-provider-request-frontend".into(),
            dispatch_id: "dispatch-frontend".into(),
        }];
    successor.request.user_prompt_bytes = successor.request.user_prompt.len();
    successor.request.user_prompt_sha256 =
        group_agent_prompt_sha256(&successor.request.user_prompt);
    let request_digest = successor.request.expected_sha256().expect("request digest");
    successor.request.request_id = format!("scheduled-node-request-{request_digest}");
    successor.request.request_sha256 = request_digest;
    let contract_digest = successor.expected_sha256().expect("contract digest");
    successor.contract_id = format!("scheduled-node-contract-{contract_digest}");
    successor.contract_sha256 = contract_digest;
}

#[test]
fn ordinal_one_successor_request_persists_through_the_v18_table() {
    use sqlite_group_agent_scheduled_node_contract_support::prepared_fixture as contract_fixture;
    let (fixture, candidate) = contract_fixture();
    let backend_node_id = candidate
        .schedule
        .nodes
        .iter()
        .find(|node| node.execution_ordinal == 1)
        .map(|node| node.node_id.clone())
        .expect("backend node id");
    let (admit, _successor) = successor_admit_request(&candidate);
    let admitted = fixture
        .store
        .admit_group_agent_scheduled_node_successor(&admit)
        .expect("admit successor candidate");
    assert_eq!(admitted.inspection.record.execution_ordinal, 1);

    // ordinal-1 provider request 落库(v18 表)
    let request = sqlite_group_agent_scheduled_node_provider_request_support::request(
        &admitted.inspection,
        "successor-provider-request-key",
        80,
    );
    // request() 已按 successor source 重签名(request 层 reidentify)
    request
        .validate()
        .expect("ordinal-1 provider request validates");
    let stored = fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("ordinal-1 provider request persists")
        .inspection;
    assert_eq!(stored.record.execution_ordinal, 1);
    assert_eq!(stored.record.node_id, backend_node_id);
}

#[test]
fn same_run_rejects_a_second_candidate_for_the_same_node_v20() {
    use sqlite_group_agent_scheduled_node_contract_support::prepared_fixture as contract_fixture;
    let (fixture, candidate) = contract_fixture();
    let (admit_backend, _) = successor_admit_request(&candidate);
    fixture
        .store
        .admit_group_agent_scheduled_node_successor(&admit_backend)
        .expect("admit backend successor candidate");

    // 同一 run 同一 node 的第二个候选(不同 idempotency key)必须冲突:
    // v20 的 UNIQUE(graph_run_id, node_id, attempt) 是 per-node 唯一性。
    let mut duplicate = admit_backend.clone();
    duplicate.idempotency_key = "successor-admit-key-backend-2".into();
    let mut duplicate_candidate = duplicate.candidate;
    duplicate_candidate.request.user_prompt =
        "{\"v\":2,\"node_id\":\"backend\",\"task\":\"backend task 2\",\"acceptance\":\"acceptance 2\"}"
            .to_owned();
    duplicate_candidate.request.user_prompt_bytes = duplicate_candidate.request.user_prompt.len();
    duplicate_candidate.request.user_prompt_sha256 =
        forge_runtime_domain::group_agent_prompt_sha256(&duplicate_candidate.request.user_prompt);
    let request_digest = duplicate_candidate
        .request
        .expected_sha256()
        .expect("duplicate request digest");
    duplicate_candidate.request.request_id = format!("scheduled-node-request-{request_digest}");
    duplicate_candidate.request.request_sha256 = request_digest;
    let contract_digest = duplicate_candidate
        .expected_sha256()
        .expect("duplicate contract digest");
    duplicate_candidate.contract_id = format!("scheduled-node-contract-{contract_digest}");
    duplicate_candidate.contract_sha256 = contract_digest;
    duplicate.candidate = duplicate_candidate;
    duplicate.candidate_json = duplicate
        .candidate
        .canonical_json()
        .expect("duplicate canonical JSON");
    let error = fixture
        .store
        .admit_group_agent_scheduled_node_successor(&duplicate)
        .expect_err("second candidate for the same node must conflict");
    assert!(matches!(error, HubStoreError::Conflict { .. }));
}
