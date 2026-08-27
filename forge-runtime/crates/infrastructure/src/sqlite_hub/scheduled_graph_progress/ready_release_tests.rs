use crate::runtime_domain::{
    ClaimGroupAgentScheduledNodeDispatchResult, GroupAgentScheduledNodeLifecycleStatus,
    HubStoreError, ScheduledGraphProgressStore, ScheduledReadyNodeReleaseStore,
};

use super::{
    read::atomicity_fixture::{ReadyFixture, claimed_fixture, ready_fixture},
    ready_release,
};

#[path = "ready_release_test_support.rs"]
mod support;

#[test]
fn ready_bundle_keeps_one_snapshot_while_a_legal_claim_commits() {
    let fixture = ready_fixture();
    let before_progress = fixture
        .graph
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("ready progress snapshot");
    let selected = &before_progress.nodes[0];
    let before = fixture
        .graph
        .store
        .inspect_scheduled_ready_node_release(
            "graph-run-1",
            &before_progress.snapshot_sha256,
            0,
            &selected.node_id,
        )
        .expect("ready release source before claim");
    let mut reader = fixture.graph.connection();

    let during = ready_release::inspect_with_concurrent_writer(
        &mut reader,
        "graph-run-1",
        &before_progress.snapshot_sha256,
        0,
        &selected.node_id,
        || {
            let claimed = fixture.claim()?;
            assert!(matches!(
                claimed,
                ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. }
            ));
            Ok(())
        },
    )
    .expect("pinned reader returns complete pre-claim bundle");

    assert_eq!(during, before);
    assert_fresh_claimed(
        &fixture,
        &before_progress.snapshot_sha256,
        &selected.node_id,
    );
}

fn assert_fresh_claimed(fixture: &ReadyFixture, before_sha256: &str, node_id: &str) {
    let after = fixture
        .graph
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("claimed progress snapshot");
    assert_eq!(
        after.nodes[0].lifecycle_status,
        Some(GroupAgentScheduledNodeLifecycleStatus::Claimed)
    );
    assert_ne!(after.snapshot_sha256, before_sha256);
    assert!(matches!(
        fixture.graph.store.inspect_scheduled_ready_node_release(
            "graph-run-1",
            before_sha256,
            0,
            node_id,
        ),
        Err(HubStoreError::Conflict { .. })
    ));
}

#[test]
fn ready_bundle_rejects_the_pre_claim_snapshot_after_terminalization() {
    let fixture = claimed_fixture();
    let ready = &fixture.ready_progress;
    let selected = &ready.nodes[0];
    fixture
        .terminalize()
        .expect("terminalize claimed selected node");
    let terminal = fixture
        .graph
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("terminal progress snapshot");

    assert_eq!(
        terminal.nodes[0].lifecycle_status,
        Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
    );
    assert_ne!(terminal.snapshot_sha256, ready.snapshot_sha256);
    assert!(matches!(
        fixture.graph.store.inspect_scheduled_ready_node_release(
            "graph-run-1",
            &ready.snapshot_sha256,
            0,
            &selected.node_id,
        ),
        Err(HubStoreError::Conflict { .. })
    ));
}

#[test]
fn successor_bundle_loads_ordered_direct_receipts_and_first_content_artifact() {
    let fixture = support::terminalized_diamond();
    let selected_provider = support::prepare_sso_with_content(&fixture);
    let progress = fixture
        .claimed
        .graph
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("diamond ready progress");
    let source = fixture
        .claimed
        .graph
        .store
        .inspect_scheduled_ready_node_release("graph-run-1", &progress.snapshot_sha256, 2, "sso")
        .expect("inspect exact diamond successor bundle");

    assert_eq!(source.selected_provider_request, selected_provider);
    let schedule_order = &source.schedule.schedule.nodes[2].direct_predecessor_node_ids;
    let receipt_order = source
        .direct_predecessor_receipts
        .iter()
        .map(|receipt| receipt.node_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        receipt_order,
        schedule_order
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
    assert_eq!(source.direct_predecessor_receipts, fixture.receipts);
    assert_eq!(
        source.predecessor_content_artifact.as_ref(),
        fixture.artifacts.first()
    );
}
