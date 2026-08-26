use super::fixture::{SqlMutation, assert_cases, initial_fixture, successor_fixture};

const INITIAL: &[SqlMutation] = &[
    (
        "initial candidate cross-run binding",
        "UPDATE group_agent_graph_scheduled_node_contract_candidates
         SET graph_run_id='graph-run-cross-source'",
    ),
    (
        "initial candidate schedule binding",
        "UPDATE group_agent_graph_scheduled_node_contract_candidates
         SET schedule_id='schedule-cross-source'",
    ),
    (
        "initial candidate ordinal binding",
        "UPDATE group_agent_graph_scheduled_node_contract_candidates SET execution_ordinal=31",
    ),
    (
        "initial candidate node binding",
        "UPDATE group_agent_graph_scheduled_node_contract_candidates SET node_id='backend'",
    ),
    (
        "initial candidate attempt binding",
        "UPDATE group_agent_graph_scheduled_node_contract_candidates SET attempt=2",
    ),
    (
        "initial candidate digest binding",
        "UPDATE group_agent_graph_scheduled_node_contract_candidates
         SET contract_sha256=zeroblob(32)",
    ),
    (
        "initial candidate canonical body",
        "UPDATE group_agent_graph_scheduled_node_contract_candidates
         SET contract_blob=CAST('not-json' AS BLOB),contract_bytes=8",
    ),
];

const SUCCESSOR: &[SqlMutation] = &[
    (
        "successor cross-run binding",
        "UPDATE group_agent_graph_scheduled_node_successor_candidates
         SET graph_run_id='graph-run-cross-source'",
    ),
    (
        "successor schedule binding",
        "UPDATE group_agent_graph_scheduled_node_successor_candidates
         SET schedule_id='schedule-cross-source'",
    ),
    (
        "successor ordinal binding",
        "UPDATE group_agent_graph_scheduled_node_successor_candidates SET execution_ordinal=31",
    ),
    (
        "successor node binding",
        "UPDATE group_agent_graph_scheduled_node_successor_candidates SET node_id='sso'",
    ),
    (
        "successor attempt binding",
        "UPDATE group_agent_graph_scheduled_node_successor_candidates SET attempt=2",
    ),
    (
        "successor digest binding",
        "UPDATE group_agent_graph_scheduled_node_successor_candidates
         SET request_sha256=zeroblob(32)",
    ),
    (
        "successor canonical body",
        "UPDATE group_agent_graph_scheduled_node_successor_candidates
         SET contract_blob=CAST('not-json' AS BLOB),contract_bytes=8",
    ),
];

#[test]
fn initial_candidate_corruption_matrix_fails_closed() {
    assert_cases(initial_fixture, INITIAL);
}

#[test]
fn successor_candidate_corruption_matrix_fails_closed() {
    assert_cases(successor_fixture, SUCCESSOR);
}
