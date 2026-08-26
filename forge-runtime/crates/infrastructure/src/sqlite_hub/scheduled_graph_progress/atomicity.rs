use crate::runtime_domain::{
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodeLifecycleStatus,
    ScheduledGraphProgressStore,
};

use super::{atomicity_fixture::claimed_fixture, snapshot_after_source};

#[test]
fn snapshot_keeps_complete_pre_terminal_state_while_terminalization_commits() {
    let fixture = claimed_fixture();
    let before = fixture
        .graph
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("claimed progress snapshot");
    assert_claimed_snapshot(&before);
    let mut reader = fixture.graph.connection();

    let during = snapshot_after_source(&mut reader, "graph-run-1", || {
        let result = fixture.terminalize()?;
        assert_eq!(
            result.inspection.status,
            GroupAgentScheduledNodeLifecycleStatus::Terminalized
        );
        Ok(())
    })
    .expect("reader retains pinned claimed snapshot");

    assert_eq!(during, before);
    let after = fixture
        .graph
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("terminalized progress snapshot");
    assert_terminalized_snapshot(&after, &before);
}

fn assert_claimed_snapshot(value: &crate::runtime_domain::ScheduledGraphProgressSnapshot) {
    value.validate().expect("valid claimed snapshot");
    let first = &value.nodes[0];
    assert_eq!(
        first.lifecycle_status,
        Some(GroupAgentScheduledNodeLifecycleStatus::Claimed)
    );
    assert!(first.candidate_id.is_some());
    assert!(first.provider_request_id.is_some());
    assert!(first.terminal_outcome.is_none());
    assert!(first.terminal_receipt_sha256.is_none());
    assert!(
        value.nodes[1..]
            .iter()
            .all(|node| node.lifecycle_status.is_none())
    );
}

fn assert_terminalized_snapshot(
    value: &crate::runtime_domain::ScheduledGraphProgressSnapshot,
    before: &crate::runtime_domain::ScheduledGraphProgressSnapshot,
) {
    value.validate().expect("valid terminalized snapshot");
    let first = &value.nodes[0];
    assert_eq!(first.candidate_id, before.nodes[0].candidate_id);
    assert_eq!(
        first.provider_request_id,
        before.nodes[0].provider_request_id
    );
    assert_eq!(
        first.lifecycle_status,
        Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
    );
    assert_eq!(
        first.terminal_outcome,
        Some(GroupAgentNodeTerminalOutcome::Completed)
    );
    assert!(first.terminal_receipt_sha256.is_some());
    assert_ne!(value.snapshot_sha256, before.snapshot_sha256);
    assert!(
        value.nodes[1..]
            .iter()
            .all(|node| node.lifecycle_status.is_none())
    );
}
