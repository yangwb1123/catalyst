use super::{
    BeginGroupAgentGraphRun, GROUP_AGENT_GRAPH_CORE_PLAN_VERSION, GROUP_AGENT_GRAPH_RUN_VERSION,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentGraphCorePlan, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunStore, GroupAgentGraphStore, HubStoreError,
    read, write,
};
use crate::sqlite_hub::group_agent_graph;

#[test]
fn write_reread_failure_rolls_back_run_and_event_rows() {
    let fixture = group_agent_graph::atomicity_tests::fixture();
    let graph = fixture
        .store
        .prepare_group_agent_graph(&fixture.request)
        .expect("prepare graph")
        .inspection;
    let request = request(&graph);
    let mut connection = fixture.store.connect().expect("validated run connection");
    connection
        .execute_batch(
            "CREATE TRIGGER mutate_run_after_event
             AFTER INSERT ON group_agent_graph_run_events
             BEGIN
               UPDATE group_agent_graph_runs SET node_count=2 WHERE id=NEW.graph_run_id;
             END;",
        )
        .expect("install post-insert mutation");

    assert!(matches!(
        write::begin(&mut connection, &request),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(row_count(&connection, "group_agent_graph_runs"), 0);
    assert_eq!(row_count(&connection, "group_agent_graph_run_events"), 0);
    assert!(connection.is_autocommit());
}

#[test]
fn inspect_keeps_run_event_and_graph_in_one_deferred_snapshot() {
    let fixture = group_agent_graph::atomicity_tests::fixture();
    let graph = fixture
        .store
        .prepare_group_agent_graph(&fixture.request)
        .expect("prepare graph")
        .inspection;
    let request = request(&graph);
    let created = fixture
        .store
        .begin_group_agent_graph_run(&request)
        .expect("begin Graph Run")
        .inspection;
    let mut reader = fixture.store.connect().expect("validated run reader");
    let writer = fixture.store.connect().expect("validated run writer");

    let inspection = read::inspect_after_run(&mut reader, "graph-run-1", || {
        writer
            .execute(
                "UPDATE group_agent_graph_runs SET plan_sha256=zeroblob(32)
                 WHERE id='graph-run-1'",
                [],
            )
            .expect("commit run corruption after metadata read");
        Ok(())
    })
    .expect("reader retains its original snapshot");

    assert_eq!(inspection, created);
    assert!(matches!(
        fixture.store.inspect_group_agent_graph_run("graph-run-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

pub(in crate::sqlite_hub) fn request(
    graph: &super::GroupAgentGraphInspection,
) -> BeginGroupAgentGraphRun {
    let mut plan = GroupAgentGraphCorePlan {
        v: GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        graph_version: graph.graph.v,
        graph_id: graph.graph.graph_id.clone(),
        graph_manifest_sha256: graph.graph.manifest_sha256.clone(),
        authored_node_ids: graph
            .manifest
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect(),
        edges: graph.manifest.edges.clone(),
        waves: graph.manifest.waves.clone(),
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan_sha256: "0".repeat(64),
    };
    plan.plan_sha256 = plan.expected_sha256().expect("plan digest");
    begin_request(graph, plan)
}

fn begin_request(
    graph: &super::GroupAgentGraphInspection,
    plan: GroupAgentGraphCorePlan,
) -> BeginGroupAgentGraphRun {
    let event = GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: "graph-run-1".into(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: graph.graph.graph_id.clone(),
            graph_manifest_sha256: graph.graph.manifest_sha256.clone(),
            plan_sha256: plan.plan_sha256.clone(),
            scheduler_protocol_version: plan.scheduler_protocol_version,
            prepared_at_ms: 30,
        },
    };
    BeginGroupAgentGraphRun {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: "graph-run-1".into(),
        graph_id: graph.graph.graph_id.clone(),
        source_snapshot_sha256: graph.graph.source_snapshot_sha256.clone(),
        graph_manifest_sha256: graph.graph.manifest_sha256.clone(),
        plan_json: plan.canonical_json().expect("canonical plan"),
        plan,
        event_json: event.canonical_json().expect("canonical event"),
        event,
        idempotency_key: "graph-run-key".into(),
        created_at_ms: 30,
    }
}

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
