use crate::runtime_domain::{GroupAgentNodeLifecycleStore, HubStoreError};

use super::{
    claim::{self, ClaimWriteStage},
    sqlite_group_agent_node_lifecycle_support::{claim_fixture, terminal_request},
    terminalize::{self, TerminalWriteStage},
};

#[test]
fn every_claim_write_fault_rolls_back_claim_lane_event_and_v4_transition() {
    for stage in [
        ClaimWriteStage::BeforeClaim,
        ClaimWriteStage::ClaimInserted,
        ClaimWriteStage::LaneInserted,
        ClaimWriteStage::EventInserted,
        ClaimWriteStage::RunTransitioned,
    ] {
        assert_claim_fault_rolls_back(stage);
    }
}

fn assert_claim_fault_rolls_back(stage: ClaimWriteStage) {
    let source = claim_fixture();
    let mut connection = source
        .fixture
        .store
        .connect()
        .expect("validated connection");

    let error = claim::claim_with_write_fault(&mut connection, &source.request, |current| {
        if current == stage {
            return Err(injected_fault("claim"));
        }
        Ok(())
    })
    .expect_err("write fault aborts claim");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_lifecycle_counts(&connection, [0, 0, 0, 0], 3);
    assert_run_state(&connection, (3, "awaiting_dispatch_authorization", 0, 3));
    assert!(connection.is_autocommit());
}

#[test]
fn every_terminal_write_fault_rolls_back_evidence_event_v5_and_lane_release() {
    for stage in [
        TerminalWriteStage::BeforeArtifact,
        TerminalWriteStage::ArtifactInserted,
        TerminalWriteStage::ReceiptInserted,
        TerminalWriteStage::EventInserted,
        TerminalWriteStage::RunTransitioned,
        TerminalWriteStage::LaneDeleted,
    ] {
        assert_terminal_fault_rolls_back(stage);
    }
}

fn assert_terminal_fault_rolls_back(stage: TerminalWriteStage) {
    let source = claim_fixture();
    let mut connection = source.fixture.store.connect().expect("claim connection");
    claim::claim(&mut connection, &source.request).expect("seed durable claim");
    let claimed = source
        .fixture
        .store
        .inspect_group_agent_node_lifecycle(&source.request.claim.graph_run_id)
        .expect("inspect claim");
    let request = terminal_request(&source, &claimed);

    let error = terminalize::terminalize_with_write_fault(&mut connection, &request, |current| {
        if current == stage {
            return Err(injected_fault("terminal"));
        }
        Ok(())
    })
    .expect_err("write fault aborts terminalization");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_lifecycle_counts(&connection, [1, 1, 0, 0], 4);
    assert_run_state(&connection, (4, "dispatch_unknown", 1, 4));
    assert!(connection.is_autocommit());
}

fn injected_fault(subject: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: format!("injected {subject} write fault"),
    }
}

fn assert_lifecycle_counts(
    connection: &rusqlite::Connection,
    expected: [i64; 4],
    expected_events: i64,
) {
    let tables = [
        "group_agent_graph_node_dispatch_claims",
        "group_agent_project_lane_ownerships",
        "group_agent_graph_node_terminal_artifacts",
        "group_agent_graph_node_terminal_receipts",
    ];
    let actual = tables.map(|table| row_count(connection, table));
    assert_eq!(actual, expected);
    assert_eq!(
        row_count(connection, "group_agent_graph_run_events"),
        expected_events
    );
}

fn assert_run_state(connection: &rusqlite::Connection, expected: (i64, &str, i64, i64)) {
    let actual: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT run_version,status,dispatch_authority_released,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("Graph Run state");
    assert_eq!(
        actual,
        (expected.0, expected.1.into(), expected.2, expected.3)
    );
}

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
