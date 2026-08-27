use std::sync::atomic::{AtomicBool, AtomicU64};

use super::*;

#[test]
fn losing_controller_head_cas_is_not_reported_as_corruption() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let planned = append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let competing_payload = planned.head().payload.clone();

    assert_eq!(
        events::append(
            harness.store.as_ref(),
            &started.journal,
            competing_payload,
            1_002,
        ),
        Err(ScheduledGraphControllerServiceError::ConcurrentUpdate)
    );
    assert_eq!(harness.inspect(), planned);
}

#[test]
fn equal_head_conflict_is_reported_as_corrupt_evidence() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let planned = append_crash_dispatch_plan(&harness, &started.journal, &anchors);
    let store = EqualHeadConflictStore(started.journal.clone());

    assert_eq!(
        events::append(
            &store,
            &started.journal,
            planned.head().payload.clone(),
            1_002,
        ),
        Err(ScheduledGraphControllerServiceError::CorruptEvidence)
    );
}

#[test]
fn concurrent_start_with_a_different_creation_time_is_an_honest_update() {
    let harness = Harness::new();
    let started = harness.start();
    let observation = ready_observation(harness.store.as_ref());
    let mut different = harness.start_input.clone();
    different.observed_at_ms += 1;
    let header = build_header(&different, &observation).expect("valid distinct header");

    assert_ne!(header, started.journal.header);
    assert_eq!(
        harness.service.classify_concurrent_start(&header),
        ScheduledGraphControllerServiceError::ConcurrentUpdate
    );
}

#[test]
fn lifecycle_unavailable_with_writable_controller_persists_stop() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let planned = append_crash_dispatch_plan(&harness, &started.journal, &anchors);

    let recovery = harness
        .service
        .resolve_dispatch_recovery(
            &planned,
            1_002,
            Err(ScheduledGraphControllerServiceError::StoreUnavailable),
        )
        .expect("writable controller resolves unavailable lifecycle conservatively");
    let super::super::recovery::Recovery::Updated(stopped) = recovery else {
        panic!("unavailable lifecycle must persist a terminal stop");
    };
    assert!(matches!(
        stopped.head().payload,
        ScheduledGraphControllerEventPayload::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            ..
        }
    ));
    assert_eq!(harness.inspect(), *stopped);
}

#[tokio::test]
async fn completed_event_samples_the_clock_after_dispatch_planning() {
    let harness = Harness::new();
    let clock = Arc::new(IncrementingClock(AtomicU64::new(1_000)));
    let counters = ready_execution_support::Counters::default();
    let executor = ready_execution_support::service(
        &harness.fixture,
        &counters,
        None,
        ready_execution_support::StoreFault::None,
        ready_execution_support::CoreBehavior::Receipt,
    );
    let service = ScheduledGraphControllerService::new(
        harness.store.clone(),
        Arc::new(ReadyReconcile),
        Arc::new(ReadyAuthorize),
        Arc::new(ForbiddenMaterializer {
            calls: harness.materializer_calls.clone(),
        }),
        Arc::new(OpenAiCodec),
        Arc::new(executor),
        clock,
    );
    let started = service
        .start(&harness.start_input)
        .expect("start controller");
    let anchors = awaiting(&started.state);
    let mut input = harness.step_input(&anchors);
    input.observed_at_ms = started.journal.head().created_at_ms;

    let output = service.step(&input).await.expect("execute one node");
    let dispatch_time = event_time(&output.journal, |payload| {
        matches!(
            payload,
            ScheduledGraphControllerEventPayload::DispatchPlanned { .. }
        )
    });
    let completion_time = event_time(&output.journal, |payload| {
        matches!(
            payload,
            ScheduledGraphControllerEventPayload::NodeCompleted { .. }
        )
    });
    assert!(completion_time > dispatch_time);
    assert_eq!(counters.snapshot().provider, 1);
}

#[tokio::test]
async fn uncertain_stop_rebases_over_a_passive_advance() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let service = gated_uncertain_service(&harness, entered.clone(), release.clone());
    let input = harness.step_input(&anchors);
    let competitor = async {
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        let advanced = harness
            .service
            .advance(&advance_input(1_002))
            .expect("passive contender persists conservative stop");
        assert_eq!(
            advanced.state,
            ScheduledGraphControllerState::Stopped {
                reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
                provider_request_id: Some(anchors.provider_request_id.clone()),
            }
        );
        release.store(true, Ordering::Release);
    };

    let (result, ()) = tokio::join!(service.step(&input), competitor);
    let output = result.expect("uncertainty stop rebases over descendant");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            provider_request_id: Some(anchors.provider_request_id),
        }
    );
    assert_eq!(harness.inspect(), output.journal);
}

#[tokio::test]
async fn uncertain_lifecycle_recovery_rebases_over_a_passive_advance() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let entered = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let service = gated_uncertain_service(&harness, entered.clone(), release.clone());
    let input = harness.step_input(&anchors);
    let competitor = async {
        while !entered.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
        let advanced = harness
            .service
            .advance(&advance_input(1_002))
            .expect("passive contender stops before claim observation");
        assert_eq!(
            advanced.state,
            ScheduledGraphControllerState::Stopped {
                reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
                provider_request_id: Some(anchors.provider_request_id.clone()),
            }
        );
        harness.fixture.claim().expect("persist concurrent claim");
        release.store(true, Ordering::Release);
    };

    let (result, ()) = tokio::join!(service.step(&input), competitor);
    let output = result.expect("claimed recovery rebases over descendant");
    assert_eq!(
        output.state,
        ScheduledGraphControllerState::Stopped {
            reason: ScheduledGraphControllerStopReason::ClaimedUnknown,
            provider_request_id: Some(anchors.provider_request_id),
        }
    );
    assert_eq!(harness.inspect(), output.journal);
}

fn gated_uncertain_service(
    harness: &Harness,
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
) -> ScheduledGraphControllerService {
    ScheduledGraphControllerService::new(
        harness.store.clone(),
        Arc::new(ReadyReconcile),
        Arc::new(ReadyAuthorize),
        Arc::new(ForbiddenMaterializer {
            calls: harness.materializer_calls.clone(),
        }),
        Arc::new(OpenAiCodec),
        Arc::new(GatedUncertainExecutor { entered, release }),
        Arc::new(InputTimeClock),
    )
}

fn event_time(
    journal: &ScheduledGraphControllerJournal,
    predicate: impl Fn(&ScheduledGraphControllerEventPayload) -> bool,
) -> u64 {
    journal
        .events
        .iter()
        .find(|event| predicate(&event.payload))
        .expect("expected controller event")
        .created_at_ms
}

struct IncrementingClock(AtomicU64);

impl ScheduledGraphControllerClock for IncrementingClock {
    fn now_ms(&self) -> u64 {
        self.0.fetch_add(1, Ordering::AcqRel)
    }
}

struct GatedUncertainExecutor {
    entered: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
}

impl ScheduledGraphReadyNodeExecutor for GatedUncertainExecutor {
    fn execute<'a>(
        &'a self,
        _input: &'a ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ) -> super::StepFuture<'a> {
        Box::pin(async move {
            self.entered.store(true, Ordering::Release);
            while !self.release.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
            Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain)
        })
    }
}

struct EqualHeadConflictStore(ScheduledGraphControllerJournal);

impl ScheduledGraphControllerStore for EqualHeadConflictStore {
    fn start_scheduled_graph_controller(
        &self,
        _header: &ScheduledGraphControllerHeader,
        _event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
        panic!("conflict store cannot start a controller")
    }

    fn append_scheduled_graph_controller_event(
        &self,
        _event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
        Err(HubStoreError::Conflict {
            entity: HubEntity::ScheduledGraphController,
            message: "injected equal-head conflict".into(),
        })
    }

    fn inspect_scheduled_graph_controller(
        &self,
        _graph_run_id: &str,
    ) -> Result<ScheduledGraphControllerJournal, HubStoreError> {
        Ok(self.0.clone())
    }
}
