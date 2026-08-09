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
    bind_successor_node(&mut successor, &backend, candidate);
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

fn bind_successor_node(
    successor: &mut forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
    backend: &forge_runtime_domain::GroupAgentGraphExecutionScheduleNode,
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
) {
    successor.node.execution_ordinal = 1;
    successor.node.node_id.clone_from(&backend.node_id);
    successor.node.authored_node_index = backend.authored_node_index;
    successor.node.topology_wave_index = backend.topology_wave_index;
    let source = manifest_node(candidate, &backend.node_id);
    successor.node.project_id.clone_from(&source.project_id);
    successor.node.member_role.clone_from(&source.member_role);
    successor
        .node
        .agent_profile
        .clone_from(&source.agent_profile);
    successor
        .node
        .project_lane_sha256
        .clone_from(&backend.project_lane_sha256);
    successor.request.execution_ordinal = 1;
    successor.request.node_id.clone_from(&backend.node_id);
    successor
        .request
        .required_predecessor_node_ids
        .clone_from(&backend.direct_predecessor_node_ids);
}

fn manifest_node(
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    node_id: &str,
) -> forge_runtime_domain::GroupAgentGraphNode {
    candidate
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .expect("manifest node")
        .clone()
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
        group_agent_prompt_sha256, group_agent_scheduled_node_user_prompt,
    };
    successor.request.user_prompt =
        group_agent_scheduled_node_user_prompt(&backend.node_id, &task.0, &task.1)
            .expect("canonical successor user Prompt");
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

/// Builds the ordinal-2 (sso) successor candidate admission for the
/// serial-three fixture: rebinds node/request/receipts to the sso node and
/// re-signs every digest.
fn sso_successor_admit(
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    admit_backend: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
) -> forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate {
    use forge_runtime_domain::GroupAgentScheduledNodeContractScope;
    let mut successor = candidate.candidate.clone();
    successor.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    let sso = candidate
        .schedule
        .nodes
        .iter()
        .find(|node| node.execution_ordinal == 2)
        .expect("ordinal-2 schedule node")
        .clone();
    bind_sso_node(&mut successor, &sso, candidate);
    resign_successor_fields(
        &mut successor,
        &candidate.schedule.initial_node,
        &manifest_task(candidate, &sso.node_id),
        &sso,
    );
    rebind_sso_receipt(&mut successor);
    resign_sso_digests(&mut successor);
    let successor_json = successor.canonical_json().expect("sso successor JSON");
    let mut admit_sso = admit_backend.clone();
    admit_sso.idempotency_key = "scheduled-wave-admit-sso".into();
    admit_sso.admitted_at_ms = 90;
    admit_sso.candidate = successor;
    admit_sso.candidate_json = successor_json;
    admit_sso
}

fn bind_sso_node(
    successor: &mut forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
    sso: &forge_runtime_domain::GroupAgentGraphExecutionScheduleNode,
    candidate: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
) {
    successor.node.execution_ordinal = 2;
    successor.node.node_id.clone_from(&sso.node_id);
    successor.node.authored_node_index = sso.authored_node_index;
    successor.node.topology_wave_index = sso.topology_wave_index;
    let source = manifest_node(candidate, &sso.node_id);
    successor
        .node
        .project_id
        .clone_from(&source.project_id);
    successor.node.member_role.clone_from(&source.member_role);
    successor
        .node
        .agent_profile
        .clone_from(&source.agent_profile);
    successor
        .node
        .project_lane_sha256
        .clone_from(&sso.project_lane_sha256);
    successor.request.execution_ordinal = 2;
    successor.request.node_id.clone_from(&sso.node_id);
    successor.request.required_predecessor_node_ids = vec!["backend".to_owned()];
}

fn rebind_sso_receipt(
    successor: &mut forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
) {
    use forge_runtime_domain::{
        GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
    };
    successor.request.predecessor_terminal_receipts =
        vec![GroupAgentScheduledNodePredecessorReceipt {
            predecessor_node_id: "backend".to_owned(),
            predecessor_attempt: 1,
            terminal_event_seq: 0,
            terminal_event_sha256: String::new(),
            terminal_receipt_id: format!("scheduled-node-terminal-receipt-{}", "b".repeat(64)),
            terminal_receipt_sha256: "b".repeat(64),
            node_outcome: GroupAgentScheduledNodePredecessorOutcome::Completed,
            provider_request_id: "scheduled-node-provider-request-backend".into(),
            dispatch_id: "dispatch-backend".into(),
        }];
}

fn resign_sso_digests(
    successor: &mut forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
) {
    successor.request.user_prompt_bytes = successor.request.user_prompt.len();
    successor.request.user_prompt_sha256 =
        forge_runtime_domain::group_agent_prompt_sha256(&successor.request.user_prompt);
    let request_digest = successor.request.expected_sha256().expect("sso request digest");
    successor.request.request_id = format!("scheduled-node-request-{request_digest}");
    successor.request.request_sha256 = request_digest;
    let contract_digest = successor.expected_sha256().expect("sso contract digest");
    successor.contract_id = format!("scheduled-node-contract-{contract_digest}");
    successor.contract_sha256 = contract_digest;
}

#[test]
fn same_run_admits_a_second_candidate_for_a_different_node_v21() {
    use forge_runtime_domain::{
        GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore,
    };
    use sqlite_group_agent_graph_run_support as run_support;
    use sqlite_group_agent_graph_execution_schedule_support as schedule_support;
    use sqlite_group_agent_scheduled_node_contract_support as contract_support;
    let fixture = run_support::Fixture::serial_three();
    let _run = fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-1", "run-key", 30))
        .expect("seed serial-three Graph Run");
    let schedule = schedule_support::request(&fixture, "schedule-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit schedule");
    let candidate = contract_support::admission(schedule, "scheduled-contract-key", 50);
    let (admit_backend, _) = successor_admit_request(&candidate);
    fixture
        .store
        .admit_group_agent_scheduled_node_successor(&admit_backend)
        .expect("admit backend successor candidate");

    let admit_sso = sso_successor_admit(&candidate, &admit_backend);
    let admitted = fixture
        .store
        .admit_group_agent_scheduled_node_successor(&admit_sso)
        .expect("second wave sibling (sso) must admit in the same run");
    assert_eq!(admitted.inspection.record.execution_ordinal, 2);
    assert_eq!(admitted.inspection.record.node_id, "sso");
}

#[test]
fn zero_receipt_wave_sibling_persists_through_v21() {
    use sqlite_group_agent_graph_run_support as run_support;
    use sqlite_group_agent_graph_execution_schedule_support as schedule_support;
    use sqlite_group_agent_scheduled_node_contract_support as contract_support;
    use forge_runtime_domain::{
        GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore,
    };
    let fixture = run_support::Fixture::diamond();
    let _run = fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-1", "run-key", 30))
        .expect("seed diamond Graph Run");
    let schedule = schedule_support::request(&fixture, "schedule-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit schedule");
    let candidate = contract_support::admission(schedule, "scheduled-contract-key", 50);
    // backend is a same-wave sibling of frontend with an empty
    // direct-predecessor set: its successor candidate carries zero
    // receipts (ADR-0035). The v21 CHECK allows 0..=31.
    let (admit_backend, _) = successor_admit_request(&candidate);
    let mut empty = admit_backend.clone();
    empty.candidate.request.predecessor_terminal_receipts.clear();
    empty.candidate.request.required_predecessor_node_ids.clear();
    resign_sso_digests(&mut empty.candidate);
    empty.candidate_json = empty
        .candidate
        .canonical_json()
        .expect("zero-receipt canonical JSON");
    empty.idempotency_key = "scheduled-wave-sibling-zero-receipt".into();
    let admitted = fixture
        .store
        .admit_group_agent_scheduled_node_successor(&empty)
        .expect("zero-receipt wave sibling must admit through v21");
    assert_eq!(admitted.inspection.record.execution_ordinal, 1);
}
