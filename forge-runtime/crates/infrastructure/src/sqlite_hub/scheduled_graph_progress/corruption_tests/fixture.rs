use crate::runtime_domain::{
    GroupAgentScheduledNodeContractStore, GroupAgentScheduledNodeProviderRequestStore,
    HubStoreError, ScheduledGraphProgressStore,
};

use super::super::{
    sqlite_group_agent_graph_run_support::Fixture,
    sqlite_group_agent_scheduled_node_contract_support as contract_support,
    sqlite_group_agent_scheduled_node_provider_request_support as provider_support,
};

pub(super) type SqlMutation = (&'static str, &'static str);

pub(super) fn initial_fixture() -> Fixture {
    let (fixture, request) = contract_support::prepared_fixture();
    fixture
        .store
        .admit_group_agent_scheduled_node_contract(&request)
        .expect("admit initial scheduled candidate");
    fixture
}

pub(super) fn successor_fixture() -> Fixture {
    let fixture = Fixture::diamond();
    provider_support::diamond_run_with_two_contracts(&fixture);
    fixture
}

pub(super) fn provider_fixture() -> Fixture {
    let (fixture, request) = provider_support::prepared_fixture();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("prepare scheduled provider request");
    fixture
}

pub(super) fn assert_cases(make: fn() -> Fixture, cases: &[SqlMutation]) {
    for (name, sql) in cases {
        let fixture = make();
        apply_corruption(&fixture, sql);
        assert_snapshot_corrupt(&fixture, name);
    }
}

pub(super) fn apply_corruption(fixture: &Fixture, sql: &str) {
    fixture
        .connection()
        .execute_batch(&format!(
            "PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON; {sql}"
        ))
        .expect("apply stored corruption");
}

pub(super) fn assert_snapshot_corrupt(fixture: &Fixture, case: &str) {
    let error = fixture
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect_err("corrupt durable state must reject");
    assert!(
        matches!(error, HubStoreError::Corrupt { .. }),
        "{case}: expected Corrupt, got {error:?}"
    );
}
