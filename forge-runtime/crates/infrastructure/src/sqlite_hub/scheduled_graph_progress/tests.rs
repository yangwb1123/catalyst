use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleStore, GroupAgentScheduledNodeProviderRequestStore,
    ScheduledGraphProgressStore,
};

use super::{
    sqlite_group_agent_graph_execution_schedule_support as schedule_support,
    sqlite_group_agent_graph_run_support::Fixture,
    sqlite_group_agent_scheduled_node_provider_request_support as provider_support,
};

#[test]
fn schedule_only_snapshot_is_complete_and_content_free() {
    let fixture = schedule_support::prepared_fixture();
    let schedule = schedule_support::request(&fixture, "schedule-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit schedule");

    let snapshot = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("snapshot schedule progress");

    assert_eq!(snapshot.nodes.len(), 2);
    assert!(
        snapshot
            .nodes
            .iter()
            .all(|node| node.candidate_id.is_none())
    );
    snapshot.validate().expect("valid progress snapshot");
    assert!(
        !snapshot
            .canonical_json()
            .expect("canonical snapshot")
            .contains('\n')
    );
}

#[test]
fn snapshot_projects_candidate_and_prepared_request_identity() {
    let (fixture, request) = provider_support::prepared_fixture();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("prepare provider request");

    let snapshot = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("snapshot prepared request");
    let first = &snapshot.nodes[0];

    assert_eq!(
        first.candidate_id.as_deref(),
        Some(request.scheduled_contract_id.as_str())
    );
    assert_eq!(
        first.prepared_request_sha256.as_deref(),
        Some(request.prepared_request_sha256.as_str())
    );
    assert!(first.lifecycle_status.is_none());
    assert!(snapshot.nodes[1].provider_request_id.is_none());
}

#[test]
fn snapshot_preserves_noncontiguous_candidate_evidence_for_core() {
    let fixture = Fixture::diamond();
    provider_support::diamond_run_with_two_contracts(&fixture);

    let snapshot = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("snapshot noncontiguous candidates");

    assert!(snapshot.nodes[0].candidate_id.is_some());
    assert!(snapshot.nodes[1].candidate_id.is_some());
    assert!(
        snapshot
            .nodes
            .iter()
            .all(|node| node.lifecycle_status.is_none())
    );
}

#[test]
fn reconcile_snapshot_requires_an_admitted_schedule() {
    let fixture = schedule_support::prepared_fixture();
    let error = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect_err("missing schedule must reject");
    assert!(matches!(
        error,
        crate::runtime_domain::HubStoreError::NotFound { .. }
    ));
}

#[test]
fn snapshot_rejects_a_candidate_row_outside_the_schedule_ordinals() {
    let (fixture, _) = provider_support::prepared_fixture();
    fixture
        .connection()
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON;
             UPDATE group_agent_graph_scheduled_node_contract_candidates
             SET execution_ordinal=31
             WHERE graph_run_id='graph-run-1';",
        )
        .expect("move candidate outside the exact schedule");

    let error = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect_err("out-of-schedule durable evidence must reject");

    assert!(matches!(
        error,
        crate::runtime_domain::HubStoreError::Corrupt { .. }
    ));
}
