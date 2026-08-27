use crate::runtime_domain::*;
use crate::sqlite_hub::scheduled_graph_progress::{atomicity_fixture, atomicity_terminal};

use super::ready_claim_test_support;

const TABLE: &str = "group_agent_graph_scheduled_node_dispatch_lifecycles";

#[derive(Clone, Copy, Debug)]
enum Family {
    Legacy,
    Ready,
}

struct LifecycleFixture {
    fixture: atomicity_fixture::ReadyFixture,
    claim: GroupAgentScheduledNodeDispatchClaim,
    family: Family,
}

#[test]
fn adjudicated_timestamp_is_required_for_both_protocol_families() {
    assert_adjudicated_corruption("adjudicated_at_ms=NULL");
}

#[test]
fn negative_adjudicated_timestamp_returns_corrupt_without_panicking() {
    assert_adjudicated_corruption("adjudicated_at_ms=-1");
}

#[test]
fn adjudicated_timestamp_before_release_is_corrupt_for_both_families() {
    assert_adjudicated_corruption("adjudicated_at_ms=69");
}

#[test]
fn claimed_status_forbids_an_extraneous_adjudicated_timestamp() {
    for family in families() {
        let lifecycle = claimed(family);
        mutate(&lifecycle, "adjudicated_at_ms=200");
        assert_corrupt(&lifecycle, "claimed lifecycle with adjudication time");
    }
}

#[test]
fn adjudicated_status_forbids_an_extraneous_terminal_timestamp() {
    for family in families() {
        let lifecycle = adjudicated(family);
        mutate(&lifecycle, "terminalized_at_ms=200");
        assert_corrupt(&lifecycle, "adjudicated lifecycle with terminal time");
    }
}

#[test]
fn persisted_created_time_must_equal_claim_release_for_both_families() {
    for family in families() {
        let lifecycle = claimed(family);
        mutate(&lifecycle, "created_at_ms=71");
        assert_corrupt(&lifecycle, "created time drift");
    }
}

#[test]
fn terminal_time_must_equal_artifact_creation_for_both_families() {
    for family in families() {
        let lifecycle = terminalized(family);
        mutate(&lifecycle, "terminalized_at_ms=81");
        assert_corrupt(&lifecycle, "terminal time drift");
    }
}

#[test]
fn negative_terminal_timestamp_returns_corrupt_without_panicking() {
    for family in families() {
        let lifecycle = terminalized(family);
        mutate(&lifecycle, "terminalized_at_ms=-1");
        assert_corrupt(&lifecycle, "negative terminal time");
    }
}

fn assert_adjudicated_corruption(assignment: &str) {
    for family in families() {
        let lifecycle = adjudicated(family);
        mutate(&lifecycle, assignment);
        assert_corrupt(&lifecycle, "invalid adjudication time");
    }
}

fn claimed(family: Family) -> LifecycleFixture {
    let fixture = atomicity_fixture::ready_fixture();
    let claim = match family {
        Family::Legacy => {
            fixture.claim().expect("claim legacy lifecycle");
            fixture.claim_request().claim.clone()
        }
        Family::Ready => {
            let request = ready_claim_test_support::simple_ready_request(
                &fixture,
                "timestamp-ready-owner",
                70,
            );
            fixture
                .graph
                .store
                .claim_group_agent_scheduled_ready_node_dispatch(&request)
                .expect("claim ready lifecycle");
            request.claim
        }
    };
    LifecycleFixture {
        fixture,
        claim,
        family,
    }
}

fn adjudicated(family: Family) -> LifecycleFixture {
    let lifecycle = claimed(family);
    lifecycle
        .fixture
        .graph
        .store
        .adjudicate_group_agent_scheduled_node_any_dispatch(
            &AdjudicateGroupAgentScheduledNodeDispatch {
                v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
                provider_request_id: lifecycle.claim.provider_request_id.clone(),
                expected_lane_ownership_id: lifecycle.claim.lane_ownership_id.clone(),
                adjudicated_at_ms: 200,
            },
        )
        .expect("adjudicate lifecycle");
    lifecycle
}

fn terminalized(family: Family) -> LifecycleFixture {
    let lifecycle = claimed(family);
    let artifact = atomicity_terminal::completed_artifact(&lifecycle.claim);
    let request = TerminalizeGroupAgentScheduledNodeDispatch {
        v: match family {
            Family::Legacy => GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
            Family::Ready => GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
        },
        control: None,
        control_json: None,
        artifact_json: artifact.canonical_json().expect("artifact JSON"),
        receipt: None,
        receipt_json: None,
        terminalized_at_ms: artifact.created_at_ms,
    };
    match family {
        Family::Legacy => {
            lifecycle
                .fixture
                .graph
                .store
                .terminalize_group_agent_scheduled_node_dispatch(&request)
                .expect("terminalize legacy lifecycle");
        }
        Family::Ready => {
            lifecycle
                .fixture
                .graph
                .store
                .terminalize_group_agent_scheduled_ready_node_dispatch(&request)
                .expect("terminalize ready lifecycle");
        }
    }
    lifecycle
}

fn mutate(lifecycle: &LifecycleFixture, assignment: &str) {
    lifecycle
        .fixture
        .graph
        .connection()
        .execute_batch(&format!(
            "PRAGMA ignore_check_constraints=ON; UPDATE {TABLE} SET {assignment}"
        ))
        .expect("mutate lifecycle timestamp");
}

fn assert_corrupt(lifecycle: &LifecycleFixture, case: &str) {
    let result = lifecycle
        .fixture
        .graph
        .store
        .inspect_group_agent_scheduled_node_any_lifecycle(&lifecycle.claim.provider_request_id);
    assert!(
        matches!(result, Err(HubStoreError::Corrupt { .. })),
        "{case} must fail closed for {:?}, got {result:?}",
        lifecycle.family
    );
}

fn families() -> [Family; 2] {
    [Family::Legacy, Family::Ready]
}
