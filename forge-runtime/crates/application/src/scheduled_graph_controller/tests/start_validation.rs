use super::*;

#[test]
fn existing_start_clamps_clock_regression_while_finishing_planned_work() {
    let harness = Harness::new();
    let observation = ready_observation(harness.store.as_ref());
    let planned = stale_materialize_plan(&harness, &observation);
    assert_eq!(planned.head().created_at_ms, 1_001);
    let mut input = harness.start_input.clone();
    input.observed_at_ms = 1_000;

    let output = harness
        .service
        .start(&input)
        .expect("clamp regressed wall clock to durable head");
    assert!(output.journal.head().created_at_ms >= planned.head().created_at_ms);
    assert!(matches!(
        output.state,
        ScheduledGraphControllerState::AwaitingFreshConsent { .. }
    ));
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}

#[test]
fn start_rejects_independent_bounds_before_controller_creation() {
    let harness = Harness::new();
    let mut invalid = harness.start_input.clone();
    invalid.max_effectful_steps = 0;
    assert_invalid_without_controller(&harness, &invalid);

    invalid = harness.start_input.clone();
    invalid.observed_at_ms = u64::MAX;
    assert_invalid_without_controller(&harness, &invalid);

    invalid = harness.start_input.clone();
    invalid.max_total_cost_usd_micros = invalid.execution_profile.max_cost_usd_micros - 1;
    assert_invalid_without_controller(&harness, &invalid);
}

#[test]
fn advance_rejects_an_unrepresentable_clock_before_planned_work() {
    let harness = Harness::new();
    let observation = ready_observation(harness.store.as_ref());
    let planned = stale_materialize_plan(&harness, &observation);

    assert_eq!(
        harness.service.advance(&advance_input(u64::MAX)),
        Err(ScheduledGraphControllerServiceError::InvalidInput)
    );
    assert_eq!(harness.inspect(), planned);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}

#[test]
fn every_public_preflight_rejects_domain_unsafe_bidi_identifiers() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    for unsafe_character in [
        '\u{061c}', '\u{200e}', '\u{200f}', '\u{2028}', '\u{202e}', '\u{2066}', '\u{2069}',
    ] {
        let invalid = format!("graph{unsafe_character}run");
        let mut start = harness.start_input.clone();
        start.graph_run_id.clone_from(&invalid);
        assert_eq!(
            ScheduledGraphControllerService::preflight_start(&start),
            Err(ScheduledGraphControllerServiceError::InvalidInput)
        );
        let mut advance = advance_input(1_001);
        advance.graph_run_id.clone_from(&invalid);
        assert_eq!(
            ScheduledGraphControllerService::preflight_advance(&advance),
            Err(ScheduledGraphControllerServiceError::InvalidInput)
        );
        assert_eq!(
            ScheduledGraphControllerQueryService::preflight_inspect(&invalid),
            Err(ScheduledGraphControllerServiceError::InvalidInput)
        );
        let mut step = harness.step_input(&anchors);
        step.expected_provider_request_id = invalid;
        assert_eq!(
            ScheduledGraphControllerService::preflight_step(&step),
            Err(ScheduledGraphControllerServiceError::InvalidInput)
        );
    }
}

fn assert_invalid_without_controller(
    harness: &Harness,
    input: &StartScheduledGraphControllerInput,
) {
    assert_eq!(
        harness.service.start(input),
        Err(ScheduledGraphControllerServiceError::InvalidInput)
    );
    assert!(matches!(
        harness
            .store
            .inspect_scheduled_graph_controller("graph-run-1"),
        Err(HubStoreError::NotFound { .. })
    ));
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}
