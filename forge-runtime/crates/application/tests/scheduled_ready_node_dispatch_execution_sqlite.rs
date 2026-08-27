use std::sync::{Arc, Barrier};

use forge_runtime_application::{
    ExecuteGroupAgentScheduledReadyNodeDispatchResult,
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
};
use forge_runtime_domain::*;

use execution_support::{
    CoreBehavior, Counters, EffectCounts, ProviderBehavior, StoreFault, different_pricing_json,
    input, preview, service, service_with_provider_behavior,
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
async fn one_ready_step_terminalizes_once_and_repeat_never_resends() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let service = service(
        &fixture,
        &counters,
        None,
        StoreFault::None,
        CoreBehavior::Receipt,
    );
    let input = input(&preview, fixture.pricing_json());

    let first = Box::pin(service.execute(&input))
        .await
        .expect("first ready step");
    assert!(matches!(
        first,
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized { .. }
    ));
    assert_eq!(counters.snapshot(), completed_counts());

    let second = Box::pin(service.execute(&input))
        .await
        .expect("repeat ready step");
    assert!(matches!(
        second,
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed {
            effects,
            ..
        } if !effects.preclaim_effects_performed
    ));
    assert_eq!(counters.snapshot(), completed_counts());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn competing_steps_yield_one_authority_and_one_provider_send() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let barrier = Arc::new(Barrier::new(2));
    let service = Arc::new(service(
        &fixture,
        &counters,
        Some(barrier),
        StoreFault::None,
        CoreBehavior::Receipt,
    ));
    let input = Arc::new(input(&preview, fixture.pricing_json()));

    let left = tokio::spawn({
        let service = service.clone();
        let input = input.clone();
        async move { Box::pin(service.execute(&input)).await }
    });
    let right = tokio::spawn({
        let service = service.clone();
        let input = input.clone();
        async move { Box::pin(service.execute(&input)).await }
    });
    let results = [
        left.await.expect("left task"),
        right.await.expect("right task"),
    ];

    assert_competing_results(&results);
    assert_eq!(
        counters.snapshot(),
        EffectCounts {
            provider: 1,
            credential: 2,
            core: 1,
            owner_created: 2,
            owner_preserved: 2,
            owner_cleaned: 2,
        }
    );
}

#[tokio::test]
async fn missing_fresh_consent_has_no_credential_owner_claim_or_send() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let service = service(
        &fixture,
        &counters,
        None,
        StoreFault::None,
        CoreBehavior::Receipt,
    );
    let mut input = input(&preview, fixture.pricing_json());
    input.confirm_off_machine = false;

    assert!(matches!(
        Box::pin(service.execute(&input)).await,
        Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ConsentRequired)
    ));
    assert_eq!(counters.snapshot(), EffectCounts::default());
    assert!(matches!(
        fixture
            .writer()
            .inspect_group_agent_scheduled_ready_node_lifecycle(
                &preview.authorization.scheduled_provider_request_id,
            ),
        Err(HubStoreError::NotFound { .. })
    ));
}

#[tokio::test]
async fn predecessor_content_consent_precedes_credential_owner_claim_and_send() {
    let fixture = sqlite_support::Fixture::with_predecessor_content();
    let preview = preview(&fixture);
    assert!(
        preview
            .release_control
            .scheduled_contract
            .request
            .predecessor_content_included
    );
    let counters = Counters::default();
    let service = replay_service(&fixture, &counters);
    let input = input(&preview, fixture.pricing_json());

    assert!(matches!(
        Box::pin(service.execute(&input)).await,
        Err(
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PredecessorContentConsentRequired
        )
    ));
    assert_eq!(counters.snapshot(), EffectCounts::default());
    assert!(matches!(
        fixture
            .writer()
            .inspect_group_agent_scheduled_ready_node_lifecycle(
                &preview.authorization.scheduled_provider_request_id,
            ),
        Err(HubStoreError::NotFound { .. })
    ));
}

#[tokio::test]
async fn existing_content_dispatch_still_requires_fresh_consent_without_new_effects() {
    let fixture = sqlite_support::Fixture::with_predecessor_content();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let service = replay_service(&fixture, &counters);
    let mut input = input(&preview, fixture.pricing_json());
    input.confirm_predecessor_content = true;

    let first = Box::pin(service.execute(&input))
        .await
        .expect("consented content dispatch");
    assert!(matches!(
        first,
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized { .. }
    ));
    assert_eq!(counters.snapshot(), completed_counts());

    input.confirm_predecessor_content = false;
    assert!(matches!(
        Box::pin(service.execute(&input)).await,
        Err(
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PredecessorContentConsentRequired
        )
    ));
    assert_eq!(counters.snapshot(), completed_counts());
}

#[tokio::test]
async fn committed_claim_unavailable_preserves_owner_and_reentry_never_sends() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let faulting = service(
        &fixture,
        &counters,
        None,
        StoreFault::ClaimAfterCommit,
        CoreBehavior::Receipt,
    );
    let input = input(&preview, fixture.pricing_json());

    assert!(matches!(
        Box::pin(faulting.execute(&input)).await,
        Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::ClaimOutcomeUncertain)
    ));
    assert_lifecycle(
        &fixture,
        &preview,
        GroupAgentScheduledNodeLifecycleStatus::Claimed,
        true,
    );
    assert_eq!(counters.snapshot(), claim_uncertain_counts());

    let replay = replay_service(&fixture, &counters);
    assert_pricing_conflict(&replay, &input, fixture.pricing_json()).await;
    let result = Box::pin(replay.execute(&input))
        .await
        .expect("exact replay");
    assert_already(result, GroupAgentScheduledNodeLifecycleStatus::Claimed);
    assert_eq!(counters.snapshot(), claim_uncertain_counts());
}

#[tokio::test]
async fn committed_terminal_unavailable_returns_durable_effect_facts_and_never_resends() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let faulting = service(
        &fixture,
        &counters,
        None,
        StoreFault::TerminalAfterCommit,
        CoreBehavior::Receipt,
    );
    let input = input(&preview, fixture.pricing_json());

    let result = Box::pin(faulting.execute(&input))
        .await
        .expect("recover committed terminal outcome");
    assert!(matches!(
        result,
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized { effects, .. }
            if effects.provider_stream_polled
                && effects.logical_hub_mutated
                && effects.terminal_receipt_recorded
    ));
    assert_lifecycle(
        &fixture,
        &preview,
        GroupAgentScheduledNodeLifecycleStatus::Terminalized,
        false,
    );
    assert_eq!(counters.snapshot(), completed_counts());

    let replay = replay_service(&fixture, &counters);
    assert_pricing_conflict(&replay, &input, fixture.pricing_json()).await;
    let result = Box::pin(replay.execute(&input))
        .await
        .expect("exact replay");
    assert_already(result, GroupAgentScheduledNodeLifecycleStatus::Terminalized);
    assert_eq!(counters.snapshot(), completed_counts());
}

#[tokio::test]
async fn core_receipt_failure_durably_quarantines_and_reentry_never_resends() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let service = service(
        &fixture,
        &counters,
        None,
        StoreFault::None,
        CoreBehavior::Fail,
    );
    let input = input(&preview, fixture.pricing_json());

    let result = Box::pin(service.execute(&input))
        .await
        .expect("persist durable quarantine");
    assert!(matches!(
        result,
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::Quarantined { effects, .. }
            if effects.provider_stream_polled
                && effects.logical_hub_mutated
                && !effects.terminal_receipt_recorded
    ));
    let inspection = inspect(&fixture, &preview);
    assert_eq!(
        inspection.status,
        GroupAgentScheduledNodeLifecycleStatus::Quarantined
    );
    assert!(inspection.active_lane.is_none());
    assert!(inspection.terminal_receipt.is_none());
    assert_eq!(counters.snapshot(), completed_counts());

    let result = Box::pin(service.execute(&input))
        .await
        .expect("quarantine replay");
    assert_already(result, GroupAgentScheduledNodeLifecycleStatus::Quarantined);
    assert_eq!(counters.snapshot(), completed_counts());
}

#[tokio::test]
async fn provider_transport_uncertainty_terminalizes_and_reentry_never_resends() {
    let fixture = sqlite_support::Fixture::new();
    let preview = preview(&fixture);
    let counters = Counters::default();
    let service = service_with_provider_behavior(
        &fixture,
        &counters,
        None,
        StoreFault::None,
        CoreBehavior::Receipt,
        ProviderBehavior::TransportError,
    );
    let input = input(&preview, fixture.pricing_json());

    let first = Box::pin(service.execute(&input))
        .await
        .expect("persist transport uncertainty");
    let ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized {
        inspection,
        effects,
    } = first
    else {
        panic!("first uncertainty dispatch must terminalize");
    };
    assert!(effects.provider_stream_polled);
    let artifact = inspection.artifact.expect("uncertainty artifact");
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::TransportError
    );
    assert!(!artifact.retry_authorized);
    let receipt = inspection.terminal_receipt.expect("uncertainty receipt");
    assert_eq!(
        receipt.node_outcome,
        GroupAgentNodeTerminalOutcome::FailedUncertain
    );
    assert!(!receipt.retry_authorized);
    assert_eq!(counters.snapshot(), completed_counts());

    let replay = Box::pin(service.execute(&input))
        .await
        .expect("transport uncertainty replay");
    assert_already(replay, GroupAgentScheduledNodeLifecycleStatus::Terminalized);
    assert_eq!(counters.snapshot(), completed_counts());
}

fn replay_service(
    fixture: &sqlite_support::Fixture,
    counters: &Counters,
) -> forge_runtime_application::GroupAgentScheduledReadyNodeDispatchExecutionService {
    service(
        fixture,
        counters,
        None,
        StoreFault::None,
        CoreBehavior::Receipt,
    )
}

async fn assert_pricing_conflict(
    service: &forge_runtime_application::GroupAgentScheduledReadyNodeDispatchExecutionService,
    input: &forge_runtime_application::ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    original_pricing_json: &str,
) {
    let mut mismatched = input.clone();
    mismatched.pricing_json = different_pricing_json(original_pricing_json);
    assert!(matches!(
        Box::pin(service.execute(&mismatched)).await,
        Err(
            GroupAgentScheduledReadyNodeDispatchExecutionServiceError::Store(
                HubStoreError::Conflict { .. }
            )
        )
    ));
}

fn inspect(
    fixture: &sqlite_support::Fixture,
    preview: &forge_runtime_application::AuthorizedScheduledReadyNodeRelease,
) -> GroupAgentScheduledReadyNodeLifecycleInspection {
    fixture
        .writer()
        .inspect_group_agent_scheduled_ready_node_lifecycle(
            &preview.authorization.scheduled_provider_request_id,
        )
        .expect("inspect durable ready lifecycle")
}

fn assert_lifecycle(
    fixture: &sqlite_support::Fixture,
    preview: &forge_runtime_application::AuthorizedScheduledReadyNodeRelease,
    status: GroupAgentScheduledNodeLifecycleStatus,
    active: bool,
) {
    let inspection = inspect(fixture, preview);
    assert_eq!(inspection.status, status);
    assert_eq!(inspection.active_lane.is_some(), active);
}

fn assert_already(
    result: ExecuteGroupAgentScheduledReadyNodeDispatchResult,
    status: GroupAgentScheduledNodeLifecycleStatus,
) {
    assert!(matches!(
        result,
        ExecuteGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed {
            inspection,
            effects,
        } if inspection.status == status && !effects.preclaim_effects_performed
    ));
}

fn assert_competing_results(
    results: &[Result<
        ExecuteGroupAgentScheduledReadyNodeDispatchResult,
        GroupAgentScheduledReadyNodeDispatchExecutionServiceError,
    >; 2],
) {
    let terminalized = results.iter().filter(|value| {
        matches!(
            value,
            Ok(ExecuteGroupAgentScheduledReadyNodeDispatchResult::Terminalized { .. })
        )
    });
    let already = results.iter().filter(|value| {
        matches!(
            value,
            Ok(ExecuteGroupAgentScheduledReadyNodeDispatchResult::AlreadyClaimed {
                effects,
                ..
            }) if effects.preclaim_effects_performed
        )
    });
    assert_eq!(terminalized.count(), 1);
    assert_eq!(already.count(), 1);
}

fn completed_counts() -> EffectCounts {
    EffectCounts {
        provider: 1,
        credential: 1,
        core: 1,
        owner_created: 1,
        owner_preserved: 1,
        owner_cleaned: 1,
    }
}

fn claim_uncertain_counts() -> EffectCounts {
    EffectCounts {
        provider: 0,
        credential: 1,
        core: 0,
        owner_created: 1,
        owner_preserved: 1,
        owner_cleaned: 0,
    }
}
