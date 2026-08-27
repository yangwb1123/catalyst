use std::sync::{Arc, Barrier};

use forge_runtime_application::{
    ExecuteGroupAgentScheduledReadyNodeDispatchResult,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    GroupAgentScheduledReadyNodeOwnerCleanup,
};
use forge_runtime_domain::*;

use execution_support::{
    CoreBehavior, Counters, EffectCounts, OwnerBehavior, StoreFault, input, preview, service,
    service_with_owner_behavior,
};

pub use forge_runtime_domain as runtime_domain;

#[path = "scheduled_ready_node_dispatch_execution_support/mod.rs"]
mod execution_support;
#[path = "../../infrastructure/src/sqlite_hub/scheduled_graph_progress/atomicity_authorization.rs"]
mod legacy_authorization_support;
#[path = "../../domain/src/group_agent_node_execution/scheduled_ready_dispatch_release_test_authorization.rs"]
mod ready_authorization_support;
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

#[tokio::test]
async fn terminal_write_before_commit_preserves_owner_and_never_resends() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let faulting = service(
        &fixture,
        &counters,
        None,
        StoreFault::TerminalBeforeCommit,
        CoreBehavior::Receipt,
    );
    let input = input(&preview, fixture.pricing_json());

    assert!(matches!(
        Box::pin(faulting.execute(&input)).await,
        Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain)
    ));
    assert_claimed(&fixture, &preview);
    assert_eq!(counters.snapshot(), post_claim_owner_preserved_counts());

    let replay = service(
        &fixture,
        &counters,
        None,
        StoreFault::None,
        CoreBehavior::Receipt,
    );
    assert!(matches!(
        Box::pin(replay.execute(&input)).await,
        Ok(ExecuteGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed { effects, .. })
            if !effects.preclaim_effects_performed
    ));
    assert_eq!(counters.snapshot(), post_claim_owner_preserved_counts());
}

#[tokio::test]
async fn terminal_cleanup_failure_keeps_the_durable_invocation_receipt() {
    assert_durable_cleanup_failure(CoreBehavior::Receipt, true).await;
}

#[tokio::test]
async fn quarantine_cleanup_failure_keeps_the_durable_invocation_receipt() {
    assert_durable_cleanup_failure(CoreBehavior::Fail, false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn claim_race_cleanup_failure_keeps_both_known_invocation_receipts() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let service = Arc::new(service_with_owner_behavior(
        &fixture,
        &counters,
        Some(Arc::new(Barrier::new(2))),
        StoreFault::None,
        CoreBehavior::Receipt,
        OwnerBehavior::CleanupFails,
    ));
    let input = Arc::new(input(&preview, fixture.pricing_json()));
    let left = tokio::spawn(run_step(service.clone(), input.clone()));
    let right = tokio::spawn(run_step(service, input));
    let results = [
        left.await.expect("left task"),
        right.await.expect("right task"),
    ];

    assert!(results.iter().all(|result| match result {
        Ok(ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized { effects, .. }) => {
            effects.owner_sidecar_cleanup == GroupAgentScheduledReadyNodeOwnerCleanup::Failed
        }
        Ok(ExecuteGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed {
            effects, ..
        }) => {
            effects.preclaim_effects_performed
                && effects.owner_sidecar_cleanup == GroupAgentScheduledReadyNodeOwnerCleanup::Failed
        }
        _ => false,
    }));
    assert_eq!(counters.snapshot(), race_cleanup_failed_counts());
}

async fn assert_durable_cleanup_failure(core: CoreBehavior, receipt: bool) {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let service = service_with_owner_behavior(
        &fixture,
        &counters,
        None,
        StoreFault::None,
        core,
        OwnerBehavior::CleanupFails,
    );
    let result = Box::pin(service.execute(&input(&preview, fixture.pricing_json())))
        .await
        .expect("durable result survives cleanup failure");
    let effects = match result {
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized { effects, .. }
            if receipt =>
        {
            effects
        }
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::Quarantined { effects, .. }
            if !receipt =>
        {
            effects
        }
        _ => panic!("unexpected durable result"),
    };
    assert!(effects.logical_hub_mutated && effects.provider_stream_polled);
    assert_eq!(effects.terminal_receipt_recorded, receipt);
    assert_eq!(
        effects.owner_sidecar_cleanup,
        GroupAgentScheduledReadyNodeOwnerCleanup::Failed
    );
    assert_eq!(counters.snapshot(), post_claim_owner_preserved_counts());
}

async fn run_step(
    service: Arc<forge_runtime_application::GroupAgentScheduledReadyNodeDispatchExecutionService>,
    input: Arc<forge_runtime_application::ExecuteGroupAgentScheduledReadyNodeDispatchInput>,
) -> Result<
    ExecuteGroupAgentScheduledReadyNodeDispatchResult,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
> {
    Box::pin(service.execute(&input)).await
}

fn assert_claimed(
    fixture: &sqlite_support::Fixture,
    preview: &forge_runtime_application::AuthorizedScheduledReadyNodeRelease,
) {
    let inspection = fixture
        .writer()
        .inspect_group_agent_scheduled_ready_node_lifecycle(
            &preview.authorization.scheduled_provider_request_id,
        )
        .expect("inspect claimed lifecycle");
    assert_eq!(
        inspection.status,
        GroupAgentScheduledNodeLifecycleStatus::Claimed
    );
    assert!(inspection.active_lane.is_some());
}

fn post_claim_owner_preserved_counts() -> EffectCounts {
    EffectCounts {
        provider: 1,
        credential: 1,
        core: 1,
        owner_created: 1,
        owner_preserved: 1,
        owner_cleaned: 0,
    }
}

fn race_cleanup_failed_counts() -> EffectCounts {
    EffectCounts {
        provider: 1,
        credential: 2,
        core: 1,
        owner_created: 2,
        owner_preserved: 2,
        owner_cleaned: 0,
    }
}
