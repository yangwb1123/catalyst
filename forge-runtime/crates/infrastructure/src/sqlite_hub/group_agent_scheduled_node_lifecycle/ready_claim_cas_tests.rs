use std::sync::{Arc, Barrier};

use crate::runtime_domain::*;
use crate::sqlite_hub::scheduled_graph_progress::sqlite_group_agent_graph_run_support::Fixture as GraphFixture;

use super::ready_claim_test_support::{
    FamilyFixture, competing_family_fixture, rebind_ephemeral_claim, simple_ready_request,
    stale_progress_fixture,
};

#[test]
fn stale_progress_after_control_generation_returns_no_authority_or_v2_row() {
    let fixture = stale_progress_fixture();
    assert!(matches!(
        fixture
            .graph
            .store
            .claim_group_agent_scheduled_node_dispatch(&fixture.progress_mutator)
            .expect("claim unrelated diamond sibling"),
        ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. }
    ));

    let error = fixture
        .graph
        .store
        .claim_group_agent_scheduled_ready_node_dispatch(&fixture.ready)
        .expect_err("stale ready control must not receive authority");
    assert!(matches!(error, HubStoreError::Conflict { .. }));
    assert_eq!(
        lifecycle_rows_for(&fixture.graph, &fixture.ready.claim.provider_request_id),
        0
    );
    assert_eq!(lifecycle_rows(&fixture.graph), 1);
}

#[test]
fn concurrent_ready_claims_return_one_authority_and_one_durable_lane() {
    let fixture = crate::sqlite_hub::scheduled_graph_progress::atomicity_fixture::ready_fixture();
    let first = simple_ready_request(&fixture, "ready-racer-one", 100);
    let second = rebind_ephemeral_claim(&first, "ready-racer-two", 101);
    let (left, right) = race_ready(&fixture.graph.store, first, second);

    assert_eq!(
        usize::from(is_new_ready(&left)) + usize::from(is_new_ready(&right)),
        1
    );
    assert_eq!(
        usize::from(is_replayed_ready(&left)) + usize::from(is_replayed_ready(&right)),
        1
    );
    assert_eq!(lifecycle_rows(&fixture.graph), 1);
    assert_eq!(active_lane_rows(&fixture.graph), 1);
}

#[test]
fn exact_replay_ignores_fresh_owner_and_time_but_validates_first() {
    let fixture = crate::sqlite_hub::scheduled_graph_progress::atomicity_fixture::ready_fixture();
    let initial = simple_ready_request(&fixture, "original-owner", 100);
    claim_ready(&fixture.graph, &initial);
    let replay = rebind_ephemeral_claim(&initial, "fresh-contender", 200);
    let result = fixture
        .graph
        .store
        .claim_group_agent_scheduled_ready_node_dispatch(&replay)
        .expect("valid immutable replay");
    let ClaimGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed { inspection } = result
    else {
        panic!("fresh ephemeral fields must replay without new authority");
    };
    assert_eq!(inspection.claim.lane_ownership_id, "original-owner");
    assert_eq!(inspection.claim.released_at_ms, 100);

    let mut malformed = replay;
    malformed.claim_json.push(' ');
    assert_conflict(&fixture.graph, &malformed);
    assert_eq!(lifecycle_rows(&fixture.graph), 1);
}

#[test]
fn replay_with_immutable_pricing_or_body_difference_conflicts() {
    let fixture = crate::sqlite_hub::scheduled_graph_progress::atomicity_fixture::ready_fixture();
    let initial = simple_ready_request(&fixture, "immutable-owner", 100);
    claim_ready(&fixture.graph, &initial);

    let mut pricing = rebind_ephemeral_claim(&initial, "pricing-contender", 200);
    pricing.pricing.input_usd_micros_per_token_unit += 1;
    pricing.pricing.pricing_snapshot_sha256 = pricing.pricing.expected_sha256().expect("pricing");
    pricing.pricing_json = pricing.pricing.canonical_json().expect("pricing JSON");
    assert_conflict(&fixture.graph, &pricing);

    let mut body = rebind_ephemeral_claim(&initial, "body-contender", 201);
    body.provider_request_body.push(b' ');
    assert_conflict(&fixture.graph, &body);
    assert_eq!(lifecycle_rows(&fixture.graph), 1);
}

#[test]
fn ready_v2_owner_blocks_distinct_legacy_v1_provider_on_same_lane() {
    let family = competing_family_fixture();
    assert_distinct_provider_same_lane(&family);
    claim_ready(&family.fixture.graph, &family.ready);

    let error = family
        .fixture
        .graph
        .store
        .claim_group_agent_scheduled_node_dispatch(&family.legacy)
        .expect_err("ready v2 owner blocks legacy v1 family");
    assert!(matches!(error, HubStoreError::Conflict { .. }));
    assert_family_single_owner(&family);
}

#[test]
fn legacy_v1_owner_blocks_distinct_ready_v2_provider_on_same_lane() {
    let family = competing_family_fixture();
    assert_distinct_provider_same_lane(&family);
    assert!(matches!(
        family
            .fixture
            .graph
            .store
            .claim_group_agent_scheduled_node_dispatch(&family.legacy)
            .expect("claim legacy v1 owner"),
        ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. }
    ));

    let error = family
        .fixture
        .graph
        .store
        .claim_group_agent_scheduled_ready_node_dispatch(&family.ready)
        .expect_err("legacy v1 owner blocks ready v2 family");
    assert!(matches!(error, HubStoreError::Conflict { .. }));
    assert_family_single_owner(&family);
}

#[test]
fn concurrent_legacy_v1_and_ready_v2_claim_exactly_one_shared_lane() {
    let family = competing_family_fixture();
    assert_distinct_provider_same_lane(&family);
    let (ready, legacy) = race_families(&family);
    let ready_won = matches!(
        ready,
        Ok(ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { .. })
    );
    let legacy_won = matches!(
        legacy,
        Ok(ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. })
    );
    assert_eq!(usize::from(ready_won) + usize::from(legacy_won), 1);
    assert_eq!(
        usize::from(ready.is_err()) + usize::from(legacy.is_err()),
        1
    );
    assert!(ready.is_ok() || matches!(ready, Err(HubStoreError::Conflict { .. })));
    assert!(legacy.is_ok() || matches!(legacy, Err(HubStoreError::Conflict { .. })));
    assert_family_single_owner(&family);
}

type ReadyResult = Result<ClaimGroupAgentScheduledReadyNodeDispatchResult, HubStoreError>;
type LegacyResult = Result<ClaimGroupAgentScheduledNodeDispatchResult, HubStoreError>;

fn race_ready(
    store: &crate::sqlite_hub::SqliteHubStore,
    first: ClaimGroupAgentScheduledReadyNodeDispatch,
    second: ClaimGroupAgentScheduledReadyNodeDispatch,
) -> (ReadyResult, ReadyResult) {
    let barrier = Arc::new(Barrier::new(3));
    let left = ready_worker(store.clone(), first, barrier.clone());
    let right = ready_worker(store.clone(), second, barrier.clone());
    barrier.wait();
    (
        left.join().expect("first ready worker"),
        right.join().expect("second ready worker"),
    )
}

fn ready_worker(
    store: crate::sqlite_hub::SqliteHubStore,
    request: ClaimGroupAgentScheduledReadyNodeDispatch,
    barrier: Arc<Barrier>,
) -> std::thread::JoinHandle<ReadyResult> {
    std::thread::spawn(move || {
        barrier.wait();
        store.claim_group_agent_scheduled_ready_node_dispatch(&request)
    })
}

fn race_families(family: &FamilyFixture) -> (ReadyResult, LegacyResult) {
    let barrier = Arc::new(Barrier::new(3));
    let ready_store = family.fixture.graph.store.clone();
    let ready_request = family.ready.clone();
    let ready_barrier = barrier.clone();
    let ready = std::thread::spawn(move || {
        ready_barrier.wait();
        ready_store.claim_group_agent_scheduled_ready_node_dispatch(&ready_request)
    });
    let legacy_store = family.fixture.graph.store.clone();
    let legacy_request = family.legacy.clone();
    let legacy_barrier = barrier.clone();
    let legacy = std::thread::spawn(move || {
        legacy_barrier.wait();
        legacy_store.claim_group_agent_scheduled_node_dispatch(&legacy_request)
    });
    barrier.wait();
    (
        ready.join().expect("ready family worker"),
        legacy.join().expect("legacy family worker"),
    )
}

fn claim_ready(graph: &GraphFixture, request: &ClaimGroupAgentScheduledReadyNodeDispatch) {
    assert!(matches!(
        graph
            .store
            .claim_group_agent_scheduled_ready_node_dispatch(request)
            .expect("claim ready v2 lifecycle"),
        ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { .. }
    ));
}

fn assert_conflict(graph: &GraphFixture, request: &ClaimGroupAgentScheduledReadyNodeDispatch) {
    assert!(matches!(
        graph
            .store
            .claim_group_agent_scheduled_ready_node_dispatch(request),
        Err(HubStoreError::Conflict { .. })
    ));
}

fn assert_distinct_provider_same_lane(family: &FamilyFixture) {
    assert_ne!(
        family.ready.claim.provider_request_id,
        family.legacy.claim.provider_request_id
    );
    assert_eq!(
        family.ready.claim.project_lane_sha256,
        family.legacy.claim.project_lane_sha256
    );
}

fn assert_family_single_owner(family: &FamilyFixture) {
    assert_eq!(lifecycle_rows(&family.fixture.graph), 1);
    assert_eq!(active_lane_rows(&family.fixture.graph), 1);
    let durable_ids = family
        .fixture
        .graph
        .connection()
        .prepare(
            "SELECT provider_request_id FROM group_agent_graph_scheduled_node_dispatch_lifecycles",
        )
        .expect("prepare durable provider query")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query durable provider")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect durable provider IDs");
    assert!(
        durable_ids == vec![family.ready.claim.provider_request_id.clone()]
            || durable_ids == vec![family.legacy.claim.provider_request_id.clone()]
    );
}

fn lifecycle_rows(graph: &GraphFixture) -> i64 {
    graph.row_count("group_agent_graph_scheduled_node_dispatch_lifecycles")
}

fn lifecycle_rows_for(graph: &GraphFixture, provider_request_id: &str) -> i64 {
    graph
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM group_agent_graph_scheduled_node_dispatch_lifecycles WHERE provider_request_id=?1",
            [provider_request_id],
            |row| row.get(0),
        )
        .expect("count provider lifecycle rows")
}

fn active_lane_rows(graph: &GraphFixture) -> i64 {
    graph
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM group_agent_graph_scheduled_node_dispatch_lifecycles WHERE lane_active=1",
            [],
            |row| row.get(0),
        )
        .expect("count active scheduled lanes")
}

fn is_new_ready(result: &ReadyResult) -> bool {
    matches!(
        result,
        Ok(ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { .. })
    )
}

fn is_replayed_ready(result: &ReadyResult) -> bool {
    matches!(
        result,
        Ok(ClaimGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed { .. })
    )
}
