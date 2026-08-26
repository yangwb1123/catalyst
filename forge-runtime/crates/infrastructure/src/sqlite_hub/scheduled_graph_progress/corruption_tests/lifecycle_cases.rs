use crate::runtime_domain::{GroupAgentScheduledNodeLifecycleStatus, ScheduledGraphProgressStore};

use super::{
    fixture::{SqlMutation, apply_corruption, assert_snapshot_corrupt},
    lifecycle_fixture::claimed_fixture,
};

const LIFECYCLE: &[SqlMutation] = &[
    (
        "lifecycle cross-run binding",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET graph_run_id='graph-run-cross-source'",
    ),
    (
        "lifecycle provider presence chain",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET provider_request_id='scheduled-provider-orphan'",
    ),
    (
        "lifecycle node binding",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles SET node_id='backend'",
    ),
    (
        "lifecycle attempt binding",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles SET attempt=2",
    ),
    (
        "lifecycle authorization digest binding",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET authorization_sha256=zeroblob(32)",
    ),
    (
        "lifecycle provider body binding",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET request_body_blob=CAST('not-provider-body' AS BLOB),request_body_bytes=17",
    ),
    (
        "lifecycle canonical claim JSON",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET claim_json=claim_json||CAST(' ' AS BLOB),claim_json_bytes=claim_json_bytes+1",
    ),
    (
        "lifecycle release-control JSON",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET release_control_json=CAST('not-json' AS BLOB),release_control_json_bytes=8",
    ),
    (
        "lifecycle status evidence shape",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET status='terminalized',lane_active=0,terminalized_at_ms=80",
    ),
];

#[test]
fn claimed_fixture_is_valid_before_corruption() {
    let fixture = claimed_fixture();
    let snapshot = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("valid claimed progress snapshot");
    assert_eq!(
        snapshot.nodes[0].lifecycle_status,
        Some(GroupAgentScheduledNodeLifecycleStatus::Claimed)
    );
}

#[test]
fn lifecycle_corruption_matrix_fails_closed() {
    for (name, sql) in LIFECYCLE {
        let fixture = claimed_fixture();
        apply_corruption(&fixture, sql);
        assert_snapshot_corrupt(&fixture, name);
    }
}
