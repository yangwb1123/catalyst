use crate::runtime_domain::{
    GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledNodeTerminalReceipt,
    ScheduledGraphProgressStore, group_agent_scheduled_node_terminal_receipt_id,
};
use rusqlite::params;

use super::{
    fixture::{apply_corruption, assert_snapshot_corrupt},
    lifecycle_fixture::terminalized_fixture,
};

const TERMINAL_JSON_CASES: &[(&str, &str)] = &[
    (
        "terminal artifact canonical JSON",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET artifact_json=CAST(CAST(artifact_json AS TEXT)||' ' AS BLOB),
             artifact_json_bytes=artifact_json_bytes+1",
    ),
    (
        "terminal control canonical JSON",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET terminal_control_json=CAST(CAST(terminal_control_json AS TEXT)||' ' AS BLOB),
             terminal_control_json_bytes=terminal_control_json_bytes+1",
    ),
    (
        "terminal receipt canonical JSON",
        "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
         SET terminal_receipt_json=CAST(CAST(terminal_receipt_json AS TEXT)||' ' AS BLOB),
             terminal_receipt_json_bytes=terminal_receipt_json_bytes+1",
    ),
];

#[test]
fn terminalized_fixture_is_valid_before_corruption() {
    let fixture = terminalized_fixture();
    let snapshot = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("valid terminalized progress snapshot");
    assert_eq!(
        snapshot.nodes[0].lifecycle_status,
        Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
    );
    assert!(snapshot.nodes[0].terminal_receipt_sha256.is_some());
}

#[test]
fn terminal_json_corruption_matrix_fails_closed() {
    for (name, sql) in TERMINAL_JSON_CASES {
        let fixture = terminalized_fixture();
        apply_corruption(&fixture, sql);
        assert_snapshot_corrupt(&fixture, name);
    }
}

#[test]
fn terminal_receipt_digest_drift_fails_closed() {
    let fixture = terminalized_fixture();
    rewrite_receipt(&fixture, |receipt| {
        receipt.receipt_sha256 = "0".repeat(64);
        receipt.receipt_id =
            group_agent_scheduled_node_terminal_receipt_id(&receipt.receipt_sha256);
    });
    assert_snapshot_corrupt(&fixture, "terminal receipt digest drift");
}

#[test]
fn intrinsically_valid_terminal_receipt_source_drift_fails_closed() {
    let fixture = terminalized_fixture();
    rewrite_receipt(&fixture, |receipt| {
        receipt.node_id = "backend".into();
        let digest = receipt.expected_sha256().expect("drifted receipt digest");
        receipt.receipt_id = group_agent_scheduled_node_terminal_receipt_id(&digest);
        receipt.receipt_sha256 = digest;
        receipt
            .validate()
            .expect("intrinsically valid drifted receipt");
    });
    assert_snapshot_corrupt(&fixture, "terminal receipt source drift");
}

fn rewrite_receipt(
    fixture: &super::super::sqlite_group_agent_graph_run_support::Fixture,
    mutate: impl FnOnce(&mut GroupAgentScheduledNodeTerminalReceipt),
) {
    let connection = fixture.connection();
    let bytes: Vec<u8> = connection
        .query_row(
            "SELECT terminal_receipt_json
             FROM group_agent_graph_scheduled_node_dispatch_lifecycles",
            [],
            |row| row.get(0),
        )
        .expect("stored terminal receipt");
    let mut receipt = GroupAgentScheduledNodeTerminalReceipt::decode_exact(&bytes)
        .expect("decode terminal receipt");
    mutate(&mut receipt);
    let json = receipt.canonical_json().expect("rewritten receipt JSON");
    connection
        .execute(
            "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
             SET terminal_receipt_json=?1,terminal_receipt_json_bytes=?2",
            params![json.as_bytes(), i64::try_from(json.len()).unwrap()],
        )
        .expect("rewrite terminal receipt");
}
