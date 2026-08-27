use std::sync::{Arc, Barrier};

use crate::runtime_domain::{
    ClaimGroupAgentNodeDispatch, ClaimGroupAgentNodeDispatchResult,
    ClaimGroupAgentScheduledNodeDispatchResult, GroupAgentNodeLifecycleStore,
    GroupAgentScheduledNodeLifecycleStore, HubEntity, HubStoreError,
};

use super::{
    read::atomicity_fixture::{ReadyFixture, ready_fixture},
    sqlite_group_agent_node_lifecycle_support::prepare_claim_request,
};

fn competing_claims() -> (ReadyFixture, ClaimGroupAgentNodeDispatch) {
    let mut scheduled = ready_fixture();
    scheduled.graph.graph = scheduled.graph.prepare_single_node_sibling();
    let legacy = prepare_claim_request(
        &scheduled.graph,
        "graph-run-2",
        "legacy-run-key",
        "legacy-contract-key",
        "legacy-request-key",
        "legacy-dispatch",
        "legacy-lane-owner",
    );
    assert_eq!(
        scheduled.claim_request().claim.project_lane_sha256,
        legacy.claim.project_lane_sha256,
        "the test claims must target the same global Project lane"
    );
    (scheduled, legacy)
}

#[test]
fn scheduled_owner_blocks_legacy_claim_with_legacy_conflict_semantics() {
    let (scheduled, legacy) = competing_claims();
    assert!(matches!(
        scheduled.claim().expect("claim scheduled lane"),
        ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. }
    ));

    let error = scheduled
        .graph
        .store
        .claim_group_agent_node_dispatch(&legacy)
        .expect_err("scheduled owner must block legacy claimant");
    assert_eq!(
        error,
        HubStoreError::Conflict {
            entity: HubEntity::GroupAgentNodeLifecycle,
            message: "Project lane is already owned by another durable dispatch claim".into(),
        }
    );
}

#[test]
fn legacy_owner_blocks_scheduled_claim_with_scheduled_conflict_semantics() {
    let (scheduled, legacy) = competing_claims();
    assert!(matches!(
        scheduled
            .graph
            .store
            .claim_group_agent_node_dispatch(&legacy)
            .expect("claim legacy lane"),
        ClaimGroupAgentNodeDispatchResult::Claimed { .. }
    ));

    let error = scheduled
        .claim()
        .expect_err("legacy owner must block scheduled claimant");
    assert_eq!(
        error,
        HubStoreError::Conflict {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            message: "Project lane is already owned by a scheduled dispatch".into(),
        }
    );
}

#[test]
fn concurrent_cross_family_claims_have_exactly_one_global_lane_owner() {
    let (scheduled, legacy) = competing_claims();
    let (scheduled_result, legacy_result) = race_claims(&scheduled, legacy);
    assert_one_winner(&scheduled_result, &legacy_result);
    assert_eq!(active_owner_count(&scheduled), 1);
}

fn race_claims(
    scheduled: &ReadyFixture,
    legacy: ClaimGroupAgentNodeDispatch,
) -> (ScheduledClaimResult, LegacyClaimResult) {
    let barrier = Arc::new(Barrier::new(3));
    let scheduled_worker = {
        let store = scheduled.graph.store.clone();
        let request = Arc::new(scheduled.claim_request().clone());
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.claim_group_agent_scheduled_node_dispatch(&request)
        })
    };
    let legacy_worker = {
        let store = scheduled.graph.store.clone();
        let request = Arc::new(legacy);
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            barrier.wait();
            store.claim_group_agent_node_dispatch(&request)
        })
    };
    barrier.wait();
    (
        scheduled_worker.join().expect("scheduled claim worker"),
        legacy_worker.join().expect("legacy claim worker"),
    )
}

type ScheduledClaimResult = Result<ClaimGroupAgentScheduledNodeDispatchResult, HubStoreError>;
type LegacyClaimResult = Result<ClaimGroupAgentNodeDispatchResult, HubStoreError>;

fn assert_one_winner(scheduled_result: &ScheduledClaimResult, legacy_result: &LegacyClaimResult) {
    let scheduled_won = matches!(
        scheduled_result,
        Ok(ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. })
    );
    let legacy_won = matches!(
        legacy_result,
        Ok(ClaimGroupAgentNodeDispatchResult::Claimed { .. })
    );
    let scheduled_conflicted = matches!(
        scheduled_result,
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentScheduledNodeLifecycle,
            ..
        })
    );
    let legacy_conflicted = matches!(
        legacy_result,
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentNodeLifecycle,
            ..
        })
    );
    assert_eq!(usize::from(scheduled_won) + usize::from(legacy_won), 1);
    assert_eq!(
        usize::from(scheduled_conflicted) + usize::from(legacy_conflicted),
        1
    );
}

fn active_owner_count(scheduled: &ReadyFixture) -> i64 {
    let connection = scheduled.graph.connection();
    let legacy_owners: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM group_agent_project_lane_ownerships",
            [],
            |row| row.get(0),
        )
        .expect("count legacy owners");
    let scheduled_owners: i64 = connection
        .query_row(
            "SELECT COUNT(*)
             FROM group_agent_graph_scheduled_node_dispatch_lifecycles
             WHERE lane_active=1",
            [],
            |row| row.get(0),
        )
        .expect("count scheduled owners");
    legacy_owners + scheduled_owners
}
