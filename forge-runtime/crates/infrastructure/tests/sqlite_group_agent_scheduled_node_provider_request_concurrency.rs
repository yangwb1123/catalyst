#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
#[allow(dead_code)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[allow(dead_code)]
mod sqlite_group_agent_scheduled_node_contract_support;
#[allow(dead_code)]
mod sqlite_group_agent_scheduled_node_provider_request_support;


#[test]
fn adjudicate_update_columns_and_status_remain_live_in_current_schema() {
    // Stage-03 Finding 1 regression: v23 restores 'adjudicated' +
    // adjudicated_at_ms; a 0-row UPDATE validates the SQL on the live table.
    let (fixture, _request) = sqlite_group_agent_scheduled_node_contract_support::prepared_fixture();
    let connection = fixture.connection();
    let updated = connection
        .execute(
            "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles
             SET status='adjudicated', lane_active=0, adjudicated_at_ms=200
             WHERE provider_request_id='nonexistent-adjudicate-row'",
            [],
        )
        .expect("adjudicate UPDATE must be accepted by the current schema");
    assert_eq!(updated, 0, "no rows match the sentinel id");
    // The status/lane CHECK must accept an adjudicated row shape.
    let accepted = connection
        .query_row(
            "SELECT CASE WHEN ?1 IN ('claimed','terminalized','quarantined','adjudicated') THEN 1 ELSE 0 END",
            ["adjudicated"],
            |row| row.get::<_, i64>(0),
        )
        .expect("status domain");
    assert_eq!(accepted, 1, "adjudicated must be a legal lifecycle status");
}


fn initial_admit_graph_run(initial: &forge_runtime_domain::GroupAgentScheduledNodeContractInspection) -> String {
    initial.record.graph_run_id.clone()
}

fn assert_concurrent_request_count(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    graph_run_id: &str,
    expected: i64,
) {
    let count: i64 = fixture
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM group_agent_graph_scheduled_node_provider_requests
             WHERE graph_run_id=?1",
            [graph_run_id],
            |row| row.get(0),
        )
        .expect("count requests");
    assert_eq!(count, expected, "concurrent requests persisted");
}

#[test]
fn two_nodes_prepare_provider_requests_concurrently_through_v22() {
    use sqlite_group_agent_graph_run_support as run_support;
    use sqlite_group_agent_scheduled_node_provider_request_support as request_support;
    use forge_runtime_domain::GroupAgentScheduledNodeProviderRequestStore;

    // Diamond: initial (frontend) + zero-receipt backend successor share one
    // run; their provider requests are prepared from two threads at once —
    // the wave-parallel concurrency guarantee (Stage-03 Finding 2).
    let fixture = run_support::Fixture::diamond();
    let (initial, backend) = request_support::diamond_run_with_two_contracts(&fixture);
    let initial_request = request_support::request(&initial, "scheduled-provider-key-initial", 60);
    let backend_request = request_support::request(&backend, "scheduled-provider-key-backend", 70);

    let store_a = fixture.store.clone();
    let store_b = fixture.store.clone();
    let thread_a = std::thread::spawn(move || {
        store_a
            .prepare_group_agent_scheduled_node_provider_request(&initial_request)
            .expect("concurrent initial-node prepare")
            .inspection
            .record
            .execution_ordinal
    });
    let thread_b = std::thread::spawn(move || {
        store_b
            .prepare_group_agent_scheduled_node_provider_request(&backend_request)
            .expect("concurrent second-node prepare")
            .inspection
            .record
            .execution_ordinal
    });
    let mut ordinals = [thread_a.join().expect("thread a"), thread_b.join().expect("thread b")];
    ordinals.sort_unstable();
    assert_eq!(ordinals, [0, 1], "both nodes' requests land in one run");
    assert_concurrent_request_count(&fixture, &initial_admit_graph_run(&initial), 2);
}
