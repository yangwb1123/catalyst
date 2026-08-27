use super::*;

#[tokio::test]
async fn retryable_preclaim_failure_keeps_reservation_and_requires_new_consent() {
    let harness = Harness::new();
    let started = harness.start();
    let first = awaiting(&started.state);

    let output = harness
        .service
        .step(&harness.step_input(&first))
        .await
        .expect("record closed preclaim failure");
    assert_eq!(
        output.retryable_failure,
        Some(ScheduledGraphControllerRetryableFailure::CredentialUnavailable)
    );
    assert_eq!(output.journal.effectful_steps_reserved(), 1);
    assert_eq!(
        output.journal.cost_usd_micros_reserved(),
        output.journal.header.execution_profile.max_cost_usd_micros
    );
    let fresh = awaiting(&output.state);
    assert_ne!(fresh.awaiting_event_sha256, first.awaiting_event_sha256);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 1);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);

    assert_eq!(
        harness.service.step(&harness.step_input(&first)).await,
        Err(ScheduledGraphControllerServiceError::StaleConsent)
    );
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 1);
    assert_eq!(harness.inspect(), output.journal);
}

#[tokio::test]
async fn cost_budget_exhaustion_stops_after_nonrefundable_preclaim_failure() {
    let mut harness = Harness::new();
    harness.start_input.max_total_cost_usd_micros =
        harness.start_input.execution_profile.max_cost_usd_micros;
    let started = harness.start();
    let anchors = awaiting(&started.state);

    let output = harness
        .service
        .step(&harness.step_input(&anchors))
        .await
        .expect("persist exhausted budget stop");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::BudgetExhausted,
            provider_request_id: Some(harness.provider_request_id.clone()),
        }
    );
    assert_eq!(
        output.retryable_failure,
        Some(ScheduledGraphControllerRetryableFailure::CredentialUnavailable)
    );
    assert_eq!(output.journal.effectful_steps_reserved(), 1);
    assert_eq!(
        output.journal.cost_usd_micros_reserved(),
        output.journal.header.max_total_cost_usd_micros
    );
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 1);

    let replay = harness
        .service
        .advance(&advance_input(1_002))
        .expect("terminal budget replay");
    assert_eq!(replay.journal, output.journal);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 1);
}
