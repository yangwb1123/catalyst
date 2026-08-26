use crate::runtime_domain::{
    MAX_GROUP_AGENT_GRAPH_NODES, MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES,
    ScheduledGraphProgressSnapshot, ScheduledGraphProgressStore,
};

use super::bounds_support::{node_id, prepared_fixture};

#[test]
fn snapshot_projects_the_full_node_bound_in_exact_schedule_order() {
    let fixture = prepared_fixture();
    let snapshot = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("snapshot full-bound progress");

    assert_eq!(snapshot.node_count, MAX_GROUP_AGENT_GRAPH_NODES);
    assert_eq!(snapshot.nodes.len(), MAX_GROUP_AGENT_GRAPH_NODES);
    for (ordinal, node) in snapshot.nodes.iter().enumerate() {
        assert_eq!(node.execution_ordinal, ordinal);
        assert_eq!(node.node_id, node_id(ordinal));
        assert_eq!(node.attempt, 1);
        assert!(node.candidate_id.is_none());
    }

    let canonical = snapshot.canonical_json().expect("canonical bound snapshot");
    assert!(canonical.len() <= MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES);
    assert!(!canonical.contains('\n'));
    assert_eq!(
        ScheduledGraphProgressSnapshot::decode_exact(&canonical).expect("decode bound snapshot"),
        snapshot
    );
}
