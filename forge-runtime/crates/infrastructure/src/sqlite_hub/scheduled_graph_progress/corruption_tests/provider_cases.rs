use super::fixture::{SqlMutation, assert_cases, provider_fixture};

const PROVIDER: &[SqlMutation] = &[
    (
        "provider cross-run binding",
        "UPDATE group_agent_graph_scheduled_node_provider_requests
         SET graph_run_id='graph-run-cross-source'",
    ),
    (
        "provider schedule binding",
        "UPDATE group_agent_graph_scheduled_node_provider_requests
         SET schedule_id='schedule-cross-source'",
    ),
    (
        "provider ordinal binding",
        "UPDATE group_agent_graph_scheduled_node_provider_requests SET execution_ordinal=1",
    ),
    (
        "provider node binding",
        "UPDATE group_agent_graph_scheduled_node_provider_requests SET node_id='backend'",
    ),
    (
        "provider attempt binding",
        "UPDATE group_agent_graph_scheduled_node_provider_requests SET attempt=2",
    ),
    (
        "provider candidate presence chain",
        "UPDATE group_agent_graph_scheduled_node_provider_requests
         SET scheduled_contract_id='scheduled-contract-orphan'",
    ),
    (
        "provider prepared digest",
        "UPDATE group_agent_graph_scheduled_node_provider_requests
         SET prepared_request_sha256=zeroblob(32)",
    ),
    (
        "provider body digest",
        "UPDATE group_agent_graph_scheduled_node_provider_requests
         SET provider_request_blob=CAST('not-a-provider-request' AS BLOB),
             provider_request_bytes=22",
    ),
    (
        "provider byte metadata",
        "UPDATE group_agent_graph_scheduled_node_provider_requests
         SET provider_request_bytes=provider_request_bytes+1",
    ),
];

#[test]
fn provider_request_corruption_matrix_fails_closed() {
    assert_cases(provider_fixture, PROVIDER);
}
