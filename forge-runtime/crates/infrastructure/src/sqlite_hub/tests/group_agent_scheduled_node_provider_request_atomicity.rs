use crate::runtime_domain::HubStoreError;

use super::{sqlite_group_agent_scheduled_node_provider_request_support, write};

#[test]
fn late_reread_fault_rolls_back_request_without_mutating_source_or_journal() {
    let (fixture, request) =
        sqlite_group_agent_scheduled_node_provider_request_support::prepared_fixture();
    let mut connection = fixture.connection();
    let before_run = run_state(&connection);
    let before_events = fixture.row_count("group_agent_graph_run_events");
    let before_candidate =
        fixture.row_count("group_agent_graph_scheduled_node_contract_candidates");

    let error = write::prepare_with_before_reread(&mut connection, &request, || {
        Err(HubStoreError::Corrupt {
            message: "injected scheduled provider-request reread fault".into(),
        })
    })
    .expect_err("late fault aborts request preparation");

    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(
        fixture.row_count("group_agent_graph_scheduled_node_provider_requests"),
        0
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_scheduled_node_contract_candidates"),
        before_candidate
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_run_events"),
        before_events
    );
    assert_eq!(run_state(&connection), before_run);
    assert!(connection.is_autocommit());
}

fn run_state(connection: &rusqlite::Connection) -> (i64, String, i64, i64, i64) {
    connection
        .query_row(
            "SELECT run_version,status,execution_contract_present,
                    dispatch_request_present,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-1'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("Graph Run state")
}
