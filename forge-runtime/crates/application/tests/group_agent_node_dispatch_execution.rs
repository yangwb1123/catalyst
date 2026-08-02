#![allow(dead_code)]

mod group_agent_node_dispatch_execution_support;
mod group_agent_node_execution_support;

use std::sync::atomic::Ordering;

use forge_runtime_application::{
    ExecuteGroupAgentNodeDispatchResult, GroupAgentNodeDispatchExecutionServiceError,
};
use forge_runtime_domain::{
    GroupAgentGraphRunStatus, GroupAgentNodeLifecycleStore, GroupAgentNodeTerminalArtifactKind,
    GroupAgentNodeTerminalClassification, ModelEvent, ModelFinishReason, ProviderError, Usage,
};

use group_agent_node_dispatch_execution_support::ExecutionHarness;

#[tokio::test]
async fn completed_dispatch_terminalizes_and_reinvocation_never_resends() {
    let harness = ExecutionHarness::new(completed_events(), false);
    let result = harness
        .service
        .execute(&harness.input)
        .await
        .expect("complete dispatch");
    let ExecuteGroupAgentNodeDispatchResult::Terminalized(inspection) = result else {
        panic!("first execution must terminalize");
    };

    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::Completed
    );
    assert!(inspection.active_lane.is_none());
    let artifact = inspection.artifact.expect("result artifact");
    assert_eq!(
        artifact.artifact_kind,
        GroupAgentNodeTerminalArtifactKind::Result
    );
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::Completed
    );
    assert_eq!(artifact.output_text, "answer");
    assert!(artifact.actual_cost_calculated);
    assert_one_effect(&harness);

    let replay = harness
        .service
        .execute(&harness.input)
        .await
        .expect("inspect completed lifecycle");
    assert!(matches!(
        replay,
        ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(_)
    ));
    assert_one_effect(&harness);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_execute_streams_the_durable_claim_exactly_once() {
    let harness = ExecutionHarness::concurrent(completed_events());
    let left_service = harness.service.clone();
    let left_input = harness.input.clone();
    let right_service = harness.service.clone();
    let right_input = harness.input.clone();

    let left = tokio::spawn(async move { left_service.execute(&left_input).await });
    let right = tokio::spawn(async move { right_service.execute(&right_input).await });
    let (left, right) = tokio::join!(left, right);
    let results = [
        left.expect("left task").expect("left execution"),
        right.expect("right task").expect("right execution"),
    ];

    assert_eq!(terminalized_count(&results), 1);
    assert_eq!(already_claimed_count(&results), 1);
    assert_eq!(count(&harness.credential_reads), 2);
    assert_eq!(count(&harness.provider_calls), 1);
    assert_eq!(count(&harness.core_calls), 1);
}

#[tokio::test]
async fn empty_length_terminal_is_a_known_failed_result() {
    let harness = ExecutionHarness::new(
        vec![Ok(usage(9, 1)), Ok(terminal(ModelFinishReason::Length))],
        false,
    );
    let result = harness
        .service
        .execute(&harness.input)
        .await
        .expect("length result");
    let ExecuteGroupAgentNodeDispatchResult::Terminalized(inspection) = result else {
        panic!("length must terminalize");
    };

    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::Failed
    );
    let artifact = inspection.artifact.expect("length artifact");
    assert_eq!(
        artifact.artifact_kind,
        GroupAgentNodeTerminalArtifactKind::Result
    );
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::Length
    );
    assert!(artifact.output_text.is_empty());
}

#[tokio::test]
async fn missing_usage_terminalizes_as_uncertainty_without_fake_cost() {
    let harness = ExecutionHarness::new(
        vec![
            Ok(text("partial")),
            Ok(terminal(ModelFinishReason::Completed)),
        ],
        false,
    );
    let inspection = terminalized(&harness).await;

    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::FailedUncertain
    );
    let artifact = inspection.artifact.expect("uncertainty artifact");
    assert_eq!(
        artifact.artifact_kind,
        GroupAgentNodeTerminalArtifactKind::Uncertainty
    );
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::MissingUsage
    );
    assert!(!artifact.usage_observed);
    assert!(!artifact.actual_cost_calculated);
    assert_eq!(artifact.actual_cost_usd_micros, 0);
}

#[tokio::test]
async fn invalid_observed_usage_becomes_schema_valid_local_limit() {
    let harness = ExecutionHarness::new(
        vec![
            Ok(text("partial")),
            Ok(usage(400_001, 1)),
            Ok(terminal(ModelFinishReason::Completed)),
        ],
        false,
    );
    let inspection = terminalized(&harness).await;
    let artifact = inspection.artifact.expect("local-limit artifact");

    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::LocalLimit
    );
    assert!(!artifact.usage_observed);
    assert_eq!(artifact.input_tokens, 0);
    assert_eq!(artifact.output_tokens, 0);
    assert!(!artifact.actual_cost_calculated);
}

#[tokio::test]
async fn result_byte_limit_terminalizes_as_uncertainty_instead_of_quarantine() {
    let harness = ExecutionHarness::with_max_result_bytes(
        vec![
            Ok(text("12345")),
            Ok(usage(1, 1)),
            Ok(terminal(ModelFinishReason::Completed)),
        ],
        4,
    );
    let inspection = terminalized(&harness).await;
    let artifact = inspection.artifact.expect("local-limit artifact");

    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::FailedUncertain
    );
    assert!(inspection.active_lane.is_none());
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::LocalLimit
    );
    assert!(artifact.output_text.is_empty());
    assert_one_effect(&harness);
}

#[tokio::test]
async fn core_failure_quarantines_claim_and_reinvocation_never_resends() {
    let harness = ExecutionHarness::new(completed_events(), true);
    let error = harness
        .service
        .execute(&harness.input)
        .await
        .expect_err("Core rejection quarantines");
    assert!(matches!(
        error,
        GroupAgentNodeDispatchExecutionServiceError::DispatchQuarantined
    ));
    let lifecycle = harness
        .hub
        .inspect_group_agent_node_lifecycle(&harness.input.graph_run_id)
        .expect("durable claim");
    assert_eq!(
        lifecycle.graph_run.run.status,
        GroupAgentGraphRunStatus::DispatchUnknown
    );
    assert!(lifecycle.active_lane.is_some());
    assert_one_effect(&harness);

    let replay = harness
        .service
        .execute(&harness.input)
        .await
        .expect("quarantined inspection");
    assert!(matches!(
        replay,
        ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(_)
    ));
    assert_one_effect(&harness);
}

#[tokio::test]
async fn missing_fresh_consent_has_no_credential_provider_or_claim_effect() {
    let mut harness = ExecutionHarness::new(completed_events(), false);
    harness.input.confirm_off_machine = false;
    let error = harness
        .service
        .execute(&harness.input)
        .await
        .expect_err("consent is mandatory");

    assert!(matches!(
        error,
        GroupAgentNodeDispatchExecutionServiceError::ConsentRequired
    ));
    assert_eq!(count(&harness.credential_reads), 0);
    assert_eq!(count(&harness.provider_calls), 0);
    assert_eq!(count(&harness.core_calls), 0);
    assert!(
        harness
            .hub
            .inspect_group_agent_node_lifecycle(&harness.input.graph_run_id)
            .is_err()
    );
}

#[tokio::test]
async fn missing_consent_precedes_authorization_and_pricing_readiness() {
    let mut harness = ExecutionHarness::new(completed_events(), false);
    harness.input.confirm_off_machine = false;
    harness.input.authorization_json = "{}".into();
    harness.input.pricing_json = "{}".into();

    let error = harness
        .service
        .execute(&harness.input)
        .await
        .expect_err("consent fence precedes readiness");

    assert!(matches!(
        error,
        GroupAgentNodeDispatchExecutionServiceError::ConsentRequired
    ));
    assert_eq!(count(&harness.credential_reads), 0);
    assert_eq!(count(&harness.provider_calls), 0);
    assert_eq!(count(&harness.core_calls), 0);
}

#[tokio::test]
async fn preclaim_cancellation_has_no_irreversible_effect() {
    let harness = ExecutionHarness::new(completed_events(), false);
    harness.input.cancellation.cancel();
    let error = harness
        .service
        .execute(&harness.input)
        .await
        .expect_err("preclaim cancellation");

    assert!(matches!(
        error,
        GroupAgentNodeDispatchExecutionServiceError::InvalidInput
    ));
    assert_eq!(count(&harness.credential_reads), 0);
    assert_eq!(count(&harness.provider_calls), 0);
    assert_eq!(count(&harness.core_calls), 0);
}

#[tokio::test]
async fn factory_quote_drift_is_rejected_before_credentials_or_claim() {
    let harness = ExecutionHarness::with_drifted_quote(completed_events());
    let error = harness
        .service
        .execute(&harness.input)
        .await
        .expect_err("resolved quote must match the local recomputation");

    assert!(matches!(
        error,
        GroupAgentNodeDispatchExecutionServiceError::ProviderUnavailable
    ));
    assert_eq!(count(&harness.credential_reads), 0);
    assert_eq!(count(&harness.provider_calls), 0);
    assert_eq!(count(&harness.core_calls), 0);
    assert!(
        harness
            .hub
            .inspect_group_agent_node_lifecycle(&harness.input.graph_run_id)
            .is_err()
    );
}

async fn terminalized(
    harness: &ExecutionHarness,
) -> forge_runtime_domain::GroupAgentNodeLifecycleInspection {
    match harness
        .service
        .execute(&harness.input)
        .await
        .expect("terminalize lifecycle")
    {
        ExecuteGroupAgentNodeDispatchResult::Terminalized(inspection) => inspection,
        ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(_) => {
            panic!("first execution must terminalize")
        }
    }
}

fn assert_one_effect(harness: &ExecutionHarness) {
    assert_eq!(count(&harness.credential_reads), 1);
    assert_eq!(count(&harness.provider_calls), 1);
    assert_eq!(count(&harness.core_calls), 1);
}

fn count(counter: &std::sync::Arc<std::sync::atomic::AtomicUsize>) -> usize {
    counter.load(Ordering::Acquire)
}

fn terminalized_count(results: &[ExecuteGroupAgentNodeDispatchResult]) -> usize {
    results
        .iter()
        .filter(|result| matches!(result, ExecuteGroupAgentNodeDispatchResult::Terminalized(_)))
        .count()
}

fn already_claimed_count(results: &[ExecuteGroupAgentNodeDispatchResult]) -> usize {
    results
        .iter()
        .filter(|result| {
            matches!(
                result,
                ExecuteGroupAgentNodeDispatchResult::AlreadyClaimed(_)
            )
        })
        .count()
}

fn completed_events() -> Vec<Result<ModelEvent, ProviderError>> {
    vec![
        Ok(text("answer")),
        Ok(usage(11, 2)),
        Ok(terminal(ModelFinishReason::Completed)),
    ]
}

fn text(delta: &str) -> ModelEvent {
    ModelEvent::TextDelta {
        delta: delta.into(),
    }
}

fn usage(input_tokens: u64, output_tokens: u64) -> ModelEvent {
    ModelEvent::Usage {
        usage: Usage {
            input_tokens,
            output_tokens,
        },
    }
}

fn terminal(reason: ModelFinishReason) -> ModelEvent {
    ModelEvent::Finished { reason }
}
