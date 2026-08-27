use forge_runtime_application::{
    ExecuteGroupAgentScheduledNodeDispatchResult,
    GroupAgentScheduledNodeDispatchExecutionServiceError,
};
use forge_runtime_domain::GroupAgentScheduledNodeLifecycleStatus;

pub use forge_runtime_domain as runtime_domain;

#[path = "scheduled_legacy_node_dispatch_execution_support/mod.rs"]
mod execution_support;
#[path = "../../infrastructure/src/sqlite_hub/scheduled_graph_progress/atomicity_authorization.rs"]
mod legacy_authorization_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
mod sqlite_group_agent_graph_execution_schedule_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_graph_run_support/mod.rs"]
mod sqlite_group_agent_graph_run_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_contract_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../infrastructure/tests/sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_provider_request_support;
#[allow(dead_code)]
#[path = "scheduled_ready_release_sqlite_support/mod.rs"]
mod sqlite_support;

use execution_support::{CoreBehavior, Counters, StoreFault, input, service};

#[tokio::test]
async fn terminal_commit_then_error_recovers_exact_durable_terminalized_result() {
    let fixture = sqlite_support::Fixture::new();
    let counters = Counters::default();
    let service = service(
        &fixture,
        &counters,
        StoreFault::TerminalAfterCommit,
        CoreBehavior::Receipt,
    );

    let result = Box::pin(service.execute(&input(&fixture)))
        .await
        .expect("recover committed terminal result");
    let ExecuteGroupAgentScheduledNodeDispatchResult::Terminalized(inspection) = result else {
        panic!("expected structured terminalized result");
    };
    assert_eq!(
        inspection.status,
        GroupAgentScheduledNodeLifecycleStatus::Terminalized
    );
    assert!(inspection.active_lane.is_none());
    assert!(inspection.artifact.as_ref().unwrap().provider_poll_started);
    assert!(inspection.terminal_receipt.is_some());
    assert_eq!(counters.snapshot(), (1, 1));

    let replay = Box::pin(service.execute(&input(&fixture)))
        .await
        .expect("terminal replay inspection");
    assert!(matches!(
        replay,
        ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(value)
            if value.status == GroupAgentScheduledNodeLifecycleStatus::Terminalized
    ));
    assert_eq!(counters.snapshot(), (1, 1));
}

#[tokio::test]
async fn quarantine_commit_then_error_recovers_exact_durable_quarantined_result() {
    let fixture = sqlite_support::Fixture::new();
    let counters = Counters::default();
    let service = service(
        &fixture,
        &counters,
        StoreFault::TerminalAfterCommit,
        CoreBehavior::Fail,
    );

    let result = Box::pin(service.execute(&input(&fixture)))
        .await
        .expect("recover committed quarantine result");
    let ExecuteGroupAgentScheduledNodeDispatchResult::Quarantined(inspection) = result else {
        panic!("expected structured quarantined result");
    };
    assert_eq!(
        inspection.status,
        GroupAgentScheduledNodeLifecycleStatus::Quarantined
    );
    assert!(inspection.active_lane.is_none());
    assert!(inspection.artifact.as_ref().unwrap().provider_poll_started);
    assert!(inspection.terminal_receipt.is_none());
    assert_eq!(counters.snapshot(), (1, 1));

    let replay = Box::pin(service.execute(&input(&fixture)))
        .await
        .expect("quarantine replay inspection");
    assert!(matches!(
        replay,
        ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(value)
            if value.status == GroupAgentScheduledNodeLifecycleStatus::Quarantined
    ));
    assert_eq!(counters.snapshot(), (1, 1));
}

#[tokio::test]
async fn claim_commit_then_error_is_fixed_no_poll_uncertainty_and_never_resends() {
    let fixture = sqlite_support::Fixture::new();
    let counters = Counters::default();
    let service = service(
        &fixture,
        &counters,
        StoreFault::ClaimAfterCommit,
        CoreBehavior::Receipt,
    );

    let error = Box::pin(service.execute(&input(&fixture)))
        .await
        .expect_err("claim response loss is uncertain");
    assert!(matches!(
        error,
        GroupAgentScheduledNodeDispatchExecutionServiceError::ClaimOutcomeUncertain
    ));
    assert_eq!(counters.snapshot(), (0, 0));

    let replay = Box::pin(service.execute(&input(&fixture)))
        .await
        .expect("claimed replay inspection");
    assert!(matches!(
        replay,
        ExecuteGroupAgentScheduledNodeDispatchResult::AlreadyClaimed(value)
            if value.status == GroupAgentScheduledNodeLifecycleStatus::Claimed
    ));
    assert_eq!(counters.snapshot(), (0, 0));
}
