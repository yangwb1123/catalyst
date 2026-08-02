#![allow(dead_code)]

mod sqlite_group_agent_graph_run_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_dispatch_request_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;
mod sqlite_group_agent_node_lifecycle_support;

use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    ClaimGroupAgentNodeDispatchResult, GroupAgentGraphRunStatus, GroupAgentGraphRunStore,
    GroupAgentNodeDispatchRequestStore, GroupAgentNodeExecutionContractStore,
    GroupAgentNodeLifecycleStore, HubStoreError,
};

use sqlite_group_agent_node_lifecycle_support::{
    claim_fixture, prepare_claim_request, terminal_request,
};

#[test]
fn claim_returns_authority_once_and_persists_exact_v4_quarantine() {
    let source = claim_fixture();
    let expected_body = source.release.provider_request_json.as_bytes();
    let first = source
        .fixture
        .store
        .claim_group_agent_node_dispatch(&source.request)
        .expect("claim dispatch");
    let ClaimGroupAgentNodeDispatchResult::Claimed { authority } = first else {
        panic!("first claimant must own authority");
    };
    let (claim, body) = authority.into_parts();
    assert_eq!(claim, source.request.claim);
    assert_eq!(body, expected_body);

    let inspection = source
        .fixture
        .store
        .inspect_group_agent_node_lifecycle(&claim.graph_run_id)
        .expect("inspect v4 lifecycle");
    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::DispatchUnknown
    );
    assert_eq!(
        inspection.active_lane.as_ref(),
        Some(&source.request.active_lane)
    );
    assert!(inspection.artifact.is_none());

    let second = source
        .fixture
        .store
        .claim_group_agent_node_dispatch(&source.request)
        .expect("repeat claim returns inspection");
    assert!(matches!(
        second,
        ClaimGroupAgentNodeDispatchResult::AlreadyClaimed { .. }
    ));
    assert_counts(&source.fixture, [1, 1, 0, 0], 4, 1);
}

#[test]
fn terminalization_persists_evidence_and_releases_lane_in_one_transition() {
    let source = claim_fixture();
    source
        .fixture
        .store
        .claim_group_agent_node_dispatch(&source.request)
        .expect("claim dispatch");
    let claimed = source
        .fixture
        .store
        .inspect_group_agent_node_lifecycle(&source.request.claim.graph_run_id)
        .expect("inspect claim");
    let terminal = terminal_request(&source, &claimed);
    let result = source
        .fixture
        .store
        .terminalize_group_agent_node_dispatch(&terminal)
        .expect("terminalize dispatch");

    assert_eq!(
        result.inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::FailedUncertain
    );
    assert!(result.inspection.active_lane.is_none());
    assert_eq!(
        result.inspection.artifact.as_ref(),
        Some(&terminal.control.artifact)
    );
    assert_eq!(
        result.inspection.terminal_receipt.as_ref(),
        Some(&terminal.receipt)
    );
    assert!(matches!(
        source
            .fixture
            .store
            .terminalize_group_agent_node_dispatch(&terminal),
        Err(HubStoreError::Conflict { .. })
    ));
    assert_counts(&source.fixture, [1, 0, 1, 1], 5, 1);
    assert_immutable_sources_readable(&source);
}

fn assert_immutable_sources_readable(
    source: &sqlite_group_agent_node_lifecycle_support::ClaimFixture,
) {
    source
        .fixture
        .store
        .inspect_group_agent_node_execution_contract(&source.release.contract_record.contract_id)
        .expect("immutable contract remains readable after v5");
    source
        .fixture
        .store
        .inspect_group_agent_node_dispatch_request(
            &source.release.dispatch_request.dispatch_request_id,
        )
        .expect("immutable dispatch request remains readable after v5");
}

#[test]
fn concurrent_claimants_have_one_authority_and_one_non_replay_result() {
    let source = claim_fixture();
    let store = source.fixture.store.clone();
    let request = Arc::new(source.request.clone());
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let store = store.clone();
        let request = request.clone();
        let barrier = barrier.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.claim_group_agent_node_dispatch(&request)
        }));
    }
    barrier.wait();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("claim worker").expect("claim result"))
        .collect::<Vec<_>>();
    let authorities = results
        .iter()
        .filter(|result| matches!(result, ClaimGroupAgentNodeDispatchResult::Claimed { .. }))
        .count();
    let repeats = results.len() - authorities;
    assert_eq!((authorities, repeats), (1, 1));
    assert_counts(&source.fixture, [1, 1, 0, 0], 4, 1);
}

#[test]
fn active_project_lane_blocks_another_run_until_terminal_release() {
    let source = claim_fixture();
    let competing = prepare_claim_request(
        &source.fixture,
        "graph-run-2",
        "run-key-2",
        "contract-key-2",
        "request-key-2",
        "dispatch-2",
        "lane-2",
    );
    assert_eq!(
        source.request.claim.project_lane_sha256,
        competing.claim.project_lane_sha256
    );
    source
        .fixture
        .store
        .claim_group_agent_node_dispatch(&source.request)
        .expect("claim first run");
    assert!(matches!(
        source
            .fixture
            .store
            .claim_group_agent_node_dispatch(&competing),
        Err(HubStoreError::Conflict { .. })
    ));
    let claimed = source
        .fixture
        .store
        .inspect_group_agent_node_lifecycle(&source.request.claim.graph_run_id)
        .expect("inspect first claim");
    source
        .fixture
        .store
        .terminalize_group_agent_node_dispatch(&terminal_request(&source, &claimed))
        .expect("terminalize first run");
    assert!(matches!(
        source
            .fixture
            .store
            .claim_group_agent_node_dispatch(&competing),
        Ok(ClaimGroupAgentNodeDispatchResult::Claimed { .. })
    ));
    assert_counts(&source.fixture, [2, 1, 1, 1], 9, 2);
}

#[test]
fn invalid_terminal_evidence_leaves_v4_and_lane_unchanged() {
    let source = claim_fixture();
    source
        .fixture
        .store
        .claim_group_agent_node_dispatch(&source.request)
        .expect("claim dispatch");
    let claimed = source
        .fixture
        .store
        .inspect_group_agent_node_lifecycle(&source.request.claim.graph_run_id)
        .expect("inspect claim");
    let mut terminal = terminal_request(&source, &claimed);
    terminal.event_json.push('\n');
    assert!(matches!(
        source
            .fixture
            .store
            .terminalize_group_agent_node_dispatch(&terminal),
        Err(HubStoreError::Conflict { .. })
    ));
    let unchanged = source
        .fixture
        .store
        .inspect_group_agent_graph_run(&source.request.claim.graph_run_id)
        .expect("inspect unchanged v4");
    assert_eq!(
        unchanged.run.status,
        GroupAgentGraphRunStatus::DispatchUnknown
    );
    assert_counts(&source.fixture, [1, 1, 0, 0], 4, 1);
}

#[test]
fn claim_metadata_drift_makes_lifecycle_and_graph_run_reads_fail_closed() {
    let source = claim_fixture();
    source
        .fixture
        .store
        .claim_group_agent_node_dispatch(&source.request)
        .expect("claim dispatch");
    source
        .fixture
        .connection()
        .execute(
            "UPDATE group_agent_graph_node_dispatch_claims
             SET released_at_ms=released_at_ms+1 WHERE graph_run_id=?1",
            [&source.request.claim.graph_run_id],
        )
        .expect("inject claim metadata drift");

    assert_corrupt_reads(&source);
}

#[test]
fn terminal_receipt_time_drift_makes_lifecycle_and_graph_run_reads_fail_closed() {
    let source = claim_fixture();
    source
        .fixture
        .store
        .claim_group_agent_node_dispatch(&source.request)
        .expect("claim dispatch");
    let claimed = source
        .fixture
        .store
        .inspect_group_agent_node_lifecycle(&source.request.claim.graph_run_id)
        .expect("inspect claim");
    source
        .fixture
        .store
        .terminalize_group_agent_node_dispatch(&terminal_request(&source, &claimed))
        .expect("terminalize dispatch");
    source
        .fixture
        .connection()
        .execute(
            "UPDATE group_agent_graph_node_terminal_receipts
             SET terminal_at_ms=terminal_at_ms+1 WHERE graph_run_id=?1",
            [&source.request.claim.graph_run_id],
        )
        .expect("inject receipt metadata drift");

    assert_corrupt_reads(&source);
}

fn assert_corrupt_reads(source: &sqlite_group_agent_node_lifecycle_support::ClaimFixture) {
    let run_id = &source.request.claim.graph_run_id;
    assert!(matches!(
        source
            .fixture
            .store
            .inspect_group_agent_node_lifecycle(run_id),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        source.fixture.store.inspect_group_agent_graph_run(run_id),
        Err(HubStoreError::Corrupt { .. })
    ));
}

fn assert_counts(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    expected: [i64; 4],
    events: i64,
    source_runs: i64,
) {
    let tables = [
        "group_agent_graph_node_dispatch_claims",
        "group_agent_project_lane_ownerships",
        "group_agent_graph_node_terminal_artifacts",
        "group_agent_graph_node_terminal_receipts",
    ];
    let actual = tables.map(|table| fixture.row_count(table));
    assert_eq!(actual, expected);
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), events);
    assert_eq!(
        fixture.row_count("group_agent_graph_node_execution_contracts"),
        source_runs
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        source_runs
    );
}
