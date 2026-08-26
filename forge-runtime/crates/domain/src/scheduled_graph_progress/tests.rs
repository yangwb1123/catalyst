use super::*;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

#[test]
fn snapshot_round_trips_as_exact_canonical_json() {
    let snapshot = snapshot();
    let json = snapshot.canonical_json().expect("canonical snapshot");
    assert_eq!(
        ScheduledGraphProgressSnapshot::decode_exact(&json).expect("exact snapshot"),
        snapshot
    );
    assert!(ScheduledGraphProgressSnapshot::decode_exact(&format!("{json}\n")).is_err());
    assert!(ScheduledGraphProgressSnapshot::decode_exact(&format!(" {json}")).is_err());
}

#[test]
fn snapshot_rejects_unknown_duplicate_and_digest_drift() {
    let snapshot = snapshot();
    let json = snapshot.canonical_json().expect("canonical snapshot");
    let unknown = json.replacen("{\"v\":", "{\"unknown\":null,\"v\":", 1);
    let duplicate = json.replacen("{\"v\":1", "{\"v\":1,\"v\":1", 1);
    let drifted = json.replacen(&snapshot.snapshot_sha256, A, 1);
    assert!(ScheduledGraphProgressSnapshot::decode_exact(&unknown).is_err());
    assert!(ScheduledGraphProgressSnapshot::decode_exact(&duplicate).is_err());
    assert!(ScheduledGraphProgressSnapshot::decode_exact(&drifted).is_err());
}

#[test]
fn structural_validation_allows_noncontiguous_materialization_for_core() {
    let mut value = unsealed_snapshot();
    value.nodes[1] = candidate_node(1, "second", B);
    value.seal().expect("Core owns progress compatibility");
}

#[test]
fn snapshot_rejects_partial_or_substituted_stage_identities() {
    let mut partial = unsealed_snapshot();
    partial.nodes[0].candidate_id = Some(format!("scheduled-node-contract-{A}"));
    assert!(partial.seal().is_err());

    let mut substituted = unsealed_snapshot();
    substituted.nodes[0] = candidate_node(0, "first", A);
    substituted.nodes[0].provider_request_id =
        Some(crate::group_agent_scheduled_node_provider_request_id(B));
    substituted.nodes[0].prepared_request_sha256 = Some(C.into());
    assert!(substituted.seal().is_err());
}

#[test]
fn lifecycle_evidence_shape_is_strict() {
    let mut value = unsealed_snapshot();
    value.nodes[0] = provider_node(0, "first", A, B);
    value.nodes[0].lifecycle_status = Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized);
    assert!(value.clone().seal().is_err());

    value.nodes[0].terminal_outcome = Some(GroupAgentNodeTerminalOutcome::Completed);
    value.nodes[0].terminal_receipt_sha256 = Some(C.into());
    value.seal().expect("terminalized evidence pair");
}

#[test]
fn duplicate_terminal_receipt_identity_is_rejected() {
    let mut value = unsealed_snapshot();
    value.nodes[0] = terminal_node(0, "first", A, B, C);
    value.nodes[1] = terminal_node(1, "second", B, D, C);
    assert!(value.seal().is_err());
}

#[test]
fn decision_round_trips_and_binds_next_node() {
    let snapshot = snapshot();
    let decision = ready_decision(&snapshot, 0, "first");
    let json = decision.canonical_json().expect("canonical decision");
    let decoded = ScheduledGraphReconcileDecision::decode_exact(&json).expect("exact decision");
    decoded
        .validate_against_snapshot(&snapshot)
        .expect("bound decision");
    assert!(ScheduledGraphReconcileDecision::decode_exact(&format!("{json}\n")).is_err());

    let wrong = ready_decision(&snapshot, 0, "second");
    assert!(wrong.validate_against_snapshot(&snapshot).is_err());
}

#[test]
fn non_ready_decisions_cannot_smuggle_a_next_node() {
    let snapshot = snapshot();
    let mut decision = unsealed_decision(&snapshot, ScheduledGraphReconcileDisposition::Completed);
    decision.next_execution_ordinal = Some(0);
    decision.next_node_id = Some("first".into());
    assert!(decision.seal().is_err());
}

#[test]
fn decision_rejects_source_substitution() {
    let snapshot = snapshot();
    let mut substituted = snapshot.clone();
    substituted.graph_run_id = "graph-run-other".into();
    substituted.snapshot_sha256.clear();
    let substituted = substituted.seal().expect("other source snapshot");
    assert!(
        ready_decision(&snapshot, 0, "first")
            .validate_against_snapshot(&substituted)
            .is_err()
    );
}

fn snapshot() -> ScheduledGraphProgressSnapshot {
    unsealed_snapshot().seal().expect("fixture snapshot")
}

fn unsealed_snapshot() -> ScheduledGraphProgressSnapshot {
    ScheduledGraphProgressSnapshot {
        v: SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: "graph-run-fixture".into(),
        graph_id: "graph-fixture".into(),
        schedule_id: format!("graph-execution-schedule-{A}"),
        schedule_sha256: A.into(),
        node_count: 2,
        execution_mode: GroupAgentGraphExecutionMode::Serial,
        max_in_flight_nodes: 1,
        progression_policy: GroupAgentGraphExecutionProgressionPolicy::CompletedContiguousPrefix,
        attempt_policy: GroupAgentGraphExecutionAttemptPolicy::ExactlyOne,
        failure_policy: GroupAgentGraphExecutionFailurePolicy::FailFastNoRetry,
        nodes: vec![empty_node(0, "first"), empty_node(1, "second")],
        snapshot_sha256: String::new(),
    }
}

fn empty_node(execution_ordinal: usize, node_id: &str) -> ScheduledGraphProgressNode {
    ScheduledGraphProgressNode {
        execution_ordinal,
        node_id: node_id.into(),
        attempt: 1,
        candidate_id: None,
        candidate_sha256: None,
        provider_request_id: None,
        prepared_request_sha256: None,
        lifecycle_status: None,
        terminal_outcome: None,
        terminal_receipt_sha256: None,
    }
}

fn candidate_node(ordinal: usize, node_id: &str, digest: &str) -> ScheduledGraphProgressNode {
    let mut node = empty_node(ordinal, node_id);
    node.candidate_id = Some(format!("scheduled-node-contract-{digest}"));
    node.candidate_sha256 = Some(digest.into());
    node
}

fn provider_node(
    ordinal: usize,
    node_id: &str,
    candidate_digest: &str,
    prepared_digest: &str,
) -> ScheduledGraphProgressNode {
    let mut node = candidate_node(ordinal, node_id, candidate_digest);
    node.provider_request_id = Some(crate::group_agent_scheduled_node_provider_request_id(
        prepared_digest,
    ));
    node.prepared_request_sha256 = Some(prepared_digest.into());
    node
}

fn terminal_node(
    ordinal: usize,
    node_id: &str,
    candidate_digest: &str,
    prepared_digest: &str,
    receipt_digest: &str,
) -> ScheduledGraphProgressNode {
    let mut node = provider_node(ordinal, node_id, candidate_digest, prepared_digest);
    node.lifecycle_status = Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized);
    node.terminal_outcome = Some(GroupAgentNodeTerminalOutcome::Completed);
    node.terminal_receipt_sha256 = Some(receipt_digest.into());
    node
}

fn ready_decision(
    snapshot: &ScheduledGraphProgressSnapshot,
    ordinal: usize,
    node_id: &str,
) -> ScheduledGraphReconcileDecision {
    let mut value = unsealed_decision(snapshot, ScheduledGraphReconcileDisposition::Ready);
    value.next_execution_ordinal = Some(ordinal);
    value.next_node_id = Some(node_id.into());
    value.seal().expect("fixture decision")
}

fn unsealed_decision(
    snapshot: &ScheduledGraphProgressSnapshot,
    disposition: ScheduledGraphReconcileDisposition,
) -> ScheduledGraphReconcileDecision {
    ScheduledGraphReconcileDecision {
        v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        schedule_id: snapshot.schedule_id.clone(),
        schedule_sha256: snapshot.schedule_sha256.clone(),
        snapshot_sha256: snapshot.snapshot_sha256.clone(),
        disposition,
        next_execution_ordinal: None,
        next_node_id: None,
        decision_sha256: String::new(),
    }
}
