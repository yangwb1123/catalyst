use super::*;

#[tokio::test]
async fn dispatch_plan_crash_stops_without_reusing_precrash_consent() {
    let harness = Harness::new();
    let started = harness.start();
    let stale_anchors = awaiting(&started.state);
    let planned = append_crash_dispatch_plan(&harness, &started.journal, &stale_anchors);
    assert_eq!(planned.effectful_steps_reserved(), 1);

    let mut stale = harness.step_input(&stale_anchors);
    stale.observed_at_ms = 1_002;
    let recovered = harness
        .service
        .step(&stale)
        .await
        .expect("persist conservative crash stop");
    assert_eq!(
        recovered.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            provider_request_id: Some(stale_anchors.provider_request_id),
        }
    );
    assert_eq!(recovered.journal.effectful_steps_reserved(), 1);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn completed_lifecycle_repairs_a_dangling_dispatch_without_resend() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    Box::pin(complete_outside_controller(
        &harness,
        &anchors.authorization_sha256,
    ))
    .await;
    assert_eq!(
        harness.service.advance(&advance_input(1_003)),
        Err(ScheduledGraphControllerServiceError::MaterializationFailed)
    );
    let journal = harness.inspect();
    assert!(journal.events.iter().any(|event| matches!(
        event.payload,
        ScheduledGraphControllerEventPayload::NodeCompleted { .. }
    )));
    assert!(matches!(
        journal.head().payload,
        ScheduledGraphControllerEventPayload::MaterializePlanned {
            execution_ordinal: 1,
            ..
        }
    ));
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn completed_after_a_nonfinal_owned_node_stops_as_external_progress() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    complete_outside_controller(&harness, &anchors.authorization_sha256).await;
    let service = controller_service_with_reconcile(
        harness.store.clone(),
        harness.executor_calls.clone(),
        harness.materializer_calls.clone(),
        Arc::new(CompletedReconcile),
    );

    let output = service
        .advance(&advance_input(1_003))
        .expect("persist external-progress stop after owned completion");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::IncompatibleProgress,
            provider_request_id: None,
        }
    );
    assert!(output.journal.events.iter().any(|event| matches!(
        event.payload,
        ScheduledGraphControllerEventPayload::NodeCompleted { .. }
    )));
    assert!(!output.journal.events.iter().any(|event| matches!(
        event.payload,
        ScheduledGraphControllerEventPayload::Completed { .. }
    )));
}

struct CompletedReconcile;

impl ScheduledGraphReconcilePort for CompletedReconcile {
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        ScheduledGraphReconcileDecision {
            v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
            progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
            graph_run_id: snapshot.graph_run_id.clone(),
            schedule_id: snapshot.schedule_id.clone(),
            schedule_sha256: snapshot.schedule_sha256.clone(),
            snapshot_sha256: snapshot.snapshot_sha256.clone(),
            disposition: ScheduledGraphReconcileDisposition::Completed,
            next_execution_ordinal: None,
            next_node_id: None,
            decision_sha256: String::new(),
        }
        .seal()
        .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)
    }
}

async fn complete_outside_controller(harness: &Harness, expected_authorization_sha256: &str) {
    let counters = ready_execution_support::Counters::default();
    let executor = ready_execution_support::service(
        &harness.fixture,
        &counters,
        None,
        ready_execution_support::StoreFault::None,
        ready_execution_support::CoreBehavior::Receipt,
    );
    let preview = ready_execution_support::preview(&harness.fixture);
    assert_eq!(
        preview.authorization.authorization_sha256,
        expected_authorization_sha256
    );
    Box::pin(executor.execute(&ready_execution_support::input(
        &preview,
        harness.fixture.pricing_json(),
    )))
    .await
    .expect("outside contender completes the dispatched request");
    assert_eq!(counters.snapshot().provider, 1);
    let snapshot = harness
        .store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("ready-v2 completion remains visible to progress");
    assert_eq!(
        snapshot.nodes[0].lifecycle_status,
        Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
    );
    assert_eq!(
        snapshot.nodes[0].terminal_outcome,
        Some(GroupAgentNodeTerminalOutcome::Completed)
    );
}

#[tokio::test]
async fn external_completion_before_dispatch_stops_instead_of_claiming_ownership() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    complete_outside_controller(&harness, &anchors.authorization_sha256).await;

    let output = harness
        .service
        .advance(&advance_input(1_002))
        .expect("persist external-progress stop");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::IncompatibleProgress,
            provider_request_id: Some(anchors.provider_request_id),
        }
    );
    assert!(!output.journal.events.iter().any(|event| matches!(
        event.payload,
        ScheduledGraphControllerEventPayload::NodeCompleted { .. }
    )));
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
    let replay = harness
        .service
        .advance(&advance_input(1_003))
        .expect("terminal replay");
    assert_eq!(replay.journal, output.journal);
}

#[tokio::test]
async fn progress_after_crash_at_started_is_not_adopted_as_controller_owned() {
    let harness = Harness::new();
    let observation = ready_observation(harness.store.as_ref());
    let header =
        super::super::build_header(&harness.start_input, &observation).expect("controller header");
    let started =
        super::super::start_event(&header, &observation, 1_000).expect("started crash point");
    harness
        .store
        .start_scheduled_graph_controller(&header, &started)
        .expect("persist only Started");
    let preview = ready_execution_support::preview(&harness.fixture);
    complete_outside_controller(&harness, &preview.authorization.authorization_sha256).await;

    let output = harness
        .service
        .advance(&advance_input(1_002))
        .expect("persist progress-drift stop");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::IncompatibleProgress,
            provider_request_id: None,
        }
    );
    assert_eq!(output.journal.events.len(), 2);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
    let replay = harness
        .service
        .advance(&advance_input(1_003))
        .expect("terminal replay");
    assert_eq!(replay.journal, output.journal);
}

#[tokio::test]
async fn terminal_commit_wins_over_postclaim_uncertain_return() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let counters = ready_execution_support::Counters::default();
    let service = service_with_faulty_executor(
        &harness,
        &counters,
        ready_execution_support::StoreFault::TerminalAfterCommit,
    );

    let output = service
        .step(&harness.step_input(&anchors))
        .await
        .expect("preserve successful invocation despite passive follow-up failure");
    assert!(output.invocation.is_some());
    assert_eq!(
        output.post_invocation_error,
        Some(ScheduledGraphControllerServiceError::MaterializationFailed)
    );
    assert!(output.journal_current_observed);
    let journal = output.journal;
    assert!(journal.events.iter().any(|event| matches!(
        event.payload,
        ScheduledGraphControllerEventPayload::NodeCompleted { .. }
    )));
    assert!(!journal.events.iter().any(|event| matches!(
        event.payload,
        ScheduledGraphControllerEventPayload::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            ..
        }
    )));
    assert!(matches!(
        journal.head().payload,
        ScheduledGraphControllerEventPayload::MaterializePlanned {
            execution_ordinal: 1,
            ..
        }
    ));
    assert_eq!(counters.snapshot().provider, 1);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn claim_commit_wins_over_uncertain_return_without_provider_poll() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let counters = ready_execution_support::Counters::default();
    let service = service_with_faulty_executor(
        &harness,
        &counters,
        ready_execution_support::StoreFault::ClaimAfterCommit,
    );

    let output = service
        .step(&harness.step_input(&anchors))
        .await
        .expect("classify committed claim");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            provider_request_id: Some(anchors.provider_request_id),
        }
    );
    let ScheduledGraphControllerEventPayload::Stopped {
        snapshot_sha256,
        decision_sha256,
        ..
    } = &output.journal.head().payload
    else {
        panic!("claim recovery must append a stop");
    };
    assert_eq!((snapshot_sha256, decision_sha256), (&None, &None));
    assert_eq!(counters.snapshot().provider, 0);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn uncertain_return_without_lifecycle_is_durably_stopped() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let service = ScheduledGraphControllerService::new(
        harness.store.clone(),
        Arc::new(ReadyReconcile),
        Arc::new(ReadyAuthorize),
        Arc::new(ForbiddenMaterializer {
            calls: harness.materializer_calls.clone(),
        }),
        Arc::new(OpenAiCodec),
        Arc::new(UncertainExecutor),
        Arc::new(InputTimeClock),
    );

    let output = service
        .step(&harness.step_input(&anchors))
        .await
        .expect("persist conservative uncertainty stop");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            provider_request_id: Some(anchors.provider_request_id),
        }
    );
    let ScheduledGraphControllerEventPayload::Stopped {
        snapshot_sha256,
        decision_sha256,
        ..
    } = &output.journal.head().payload
    else {
        panic!("uncertainty fallback must append a stop");
    };
    assert!(snapshot_sha256.is_some() && decision_sha256.is_some());
    let replay = service
        .advance(&advance_input(1_002))
        .expect("terminal uncertainty replay");
    assert_eq!(replay.journal, output.journal);
}

fn service_with_faulty_executor(
    harness: &Harness,
    counters: &ready_execution_support::Counters,
    fault: ready_execution_support::StoreFault,
) -> ScheduledGraphControllerService {
    let executor = ready_execution_support::service(
        &harness.fixture,
        counters,
        None,
        fault,
        ready_execution_support::CoreBehavior::Receipt,
    );
    ScheduledGraphControllerService::new(
        harness.store.clone(),
        Arc::new(ReadyReconcile),
        Arc::new(ReadyAuthorize),
        Arc::new(ForbiddenMaterializer {
            calls: harness.materializer_calls.clone(),
        }),
        Arc::new(OpenAiCodec),
        Arc::new(executor),
        Arc::new(InputTimeClock),
    )
}

struct UncertainExecutor;

impl ScheduledGraphReadyNodeExecutor for UncertainExecutor {
    fn execute<'a>(
        &'a self,
        _input: &'a ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ) -> super::super::StepFuture<'a> {
        Box::pin(async {
            Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain)
        })
    }
}

#[test]
fn legacy_concurrent_claim_is_classified_without_ready_authorization_equality() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let legacy_authorization = GroupAgentScheduledNodeDispatchAuthorization::decode_exact(
        harness.fixture.authorization_json(),
    )
    .expect("decode fixture legacy authorization");
    assert_ne!(
        legacy_authorization.authorization_sha256,
        anchors.authorization_sha256
    );
    harness
        .fixture
        .claim()
        .expect("commit concurrent legacy claim");

    let output = harness
        .service
        .advance(&advance_input(1_002))
        .expect("classify legacy claim");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            provider_request_id: Some(anchors.provider_request_id),
        }
    );
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn dispatch_recovery_stops_before_an_unavailable_pricing_source_is_read() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    harness
        .fixture
        .claim()
        .expect("commit a concurrent legacy claim");
    let pricing_reads = Arc::new(AtomicUsize::new(0));
    let mut input = harness.step_input(&anchors);
    input.observed_at_ms = 1_002;
    input.pricing_source = Arc::new(UnavailablePricing(pricing_reads.clone()));

    let output = harness
        .service
        .step(&input)
        .await
        .expect("recover claim into terminal stop");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            provider_request_id: Some(anchors.provider_request_id),
        }
    );
    assert_eq!(pricing_reads.load(Ordering::Acquire), 0);
}

struct UnavailablePricing(Arc<AtomicUsize>);

impl ScheduledGraphControllerPricingSource for UnavailablePricing {
    fn read_pricing_json(&self) -> Result<String, ScheduledGraphControllerPricingSourceError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        Err(ScheduledGraphControllerPricingSourceError)
    }
}
