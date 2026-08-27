use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_infrastructure::{OpenAiResponsesProvider, SqliteHubStore};

use super::*;
use crate::runtime_domain::*;
use crate::{GroupAgentNodeDispatchRequestCodec, ScheduledGraphControllerState};

#[path = "../../../infrastructure/src/sqlite_hub/scheduled_graph_progress/atomicity_authorization.rs"]
mod legacy_authorization_support;
#[path = "../../../domain/src/group_agent_node_execution/scheduled_ready_dispatch_release_test_authorization.rs"]
mod ready_authorization_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../tests/scheduled_ready_node_dispatch_execution_support/mod.rs"]
mod ready_execution_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../../infrastructure/tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
mod sqlite_group_agent_graph_execution_schedule_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../../infrastructure/tests/sqlite_group_agent_graph_run_support/mod.rs"]
mod sqlite_group_agent_graph_run_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../../infrastructure/tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_contract_support;
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../../../infrastructure/tests/sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
mod sqlite_group_agent_scheduled_node_provider_request_support;
#[allow(dead_code)]
#[path = "../../tests/scheduled_ready_release_sqlite_support/mod.rs"]
mod sqlite_support;

const CORE_SHA: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const STALE_SHA: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
#[path = "tests/budget.rs"]
mod budget;
#[path = "tests/compatibility.rs"]
mod compatibility;
#[path = "tests/concurrency_clock.rs"]
mod concurrency_clock;
#[path = "tests/legacy_lifecycle.rs"]
mod legacy_lifecycle;
#[path = "tests/lifecycle_matrix.rs"]
mod lifecycle_matrix;
#[path = "tests/prepare_recovery.rs"]
mod prepare_recovery;
#[path = "tests/pricing.rs"]
mod pricing;
#[path = "tests/recovery.rs"]
mod recovery;
#[path = "tests/start_validation.rs"]
mod start_validation;

use pricing::{InputTimeClock, StaticPricing};
#[test]
fn start_passively_advances_to_fresh_consent_without_effect_reservation() {
    let harness = Harness::new();
    let output = harness.start();

    let AwaitingAnchors {
        provider_request_id,
        ..
    } = awaiting(&output.state);
    assert_eq!(provider_request_id, harness.provider_request_id);
    assert_eq!(output.journal.events.len(), 2);
    assert!(matches!(
        output.journal.events[0].payload,
        ScheduledGraphControllerEventPayload::Started { .. }
    ));
    assert!(matches!(
        output.journal.head().payload,
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent { .. }
    ));
    assert_uncharged_and_uninvoked(&harness, &output.journal);
}

#[test]
fn start_rejects_durable_candidate_profile_drift_before_controller_creation() {
    let mut harness = Harness::new();
    let mut drifted = harness.start_input.execution_profile.clone();
    drifted.timeout_ms -= 1;
    drifted.profile_sha256.clear();
    harness.start_input.execution_profile = drifted.seal().expect("valid drifted profile");

    assert_eq!(
        harness.service.start(&harness.start_input),
        Err(ScheduledGraphControllerServiceError::CorruptEvidence)
    );
    assert!(matches!(
        harness
            .store
            .inspect_scheduled_graph_controller("graph-run-1"),
        Err(HubStoreError::NotFound { .. })
    ));
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
}

#[test]
fn durable_candidate_repairs_materialize_plan_after_source_snapshot_changes() {
    let harness = Harness::new();
    let observation = ready_observation(harness.store.as_ref());
    let node = &observation.snapshot.nodes[0];
    assert!(node.candidate_id.is_some());
    assert_ne!(observation.snapshot.snapshot_sha256, STALE_SHA);
    let planned = stale_materialize_plan(&harness, &observation);

    let repaired = harness
        .service
        .materialize_ready(&planned, &observation, node, 1_002)
        .expect("observe already durable candidate");
    assert!(matches!(
        repaired.head().payload,
        ScheduledGraphControllerEventPayload::MaterializeObserved { .. }
    ));
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
}
fn stale_materialize_plan(
    harness: &Harness,
    observation: &crate::ScheduledGraphReconcileObservation,
) -> ScheduledGraphControllerJournal {
    let header = super::build_header(&harness.start_input, observation).expect("controller header");
    let started = super::start_event(&header, observation, 1_000).expect("start event");
    let journal = harness
        .store
        .start_scheduled_graph_controller(&header, &started)
        .expect("start recovery fixture")
        .journal;
    super::events::append(
        harness.store.as_ref(),
        &journal,
        ScheduledGraphControllerEventPayload::MaterializePlanned {
            execution_ordinal: 0,
            node_id: observation.snapshot.nodes[0].node_id.clone(),
            snapshot_sha256: STALE_SHA.into(),
            decision_sha256: STALE_SHA.into(),
            idempotency_key: format!("controller-{}-materialize-0", header.controller_sha256),
        },
        1_001,
    )
    .expect("append stale materialize plan")
}
fn ready_observation(store: &SqliteHubStore) -> crate::ScheduledGraphReconcileObservation {
    let snapshot = store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("progress observation");
    let decision = ReadyReconcile.decide(&snapshot).expect("ready decision");
    crate::ScheduledGraphReconcileObservation { snapshot, decision }
}

#[tokio::test]
async fn stale_or_missing_consent_never_invokes_executor_or_reserves_budget() {
    let harness = Harness::new();
    let started = harness.start();
    let anchors = awaiting(&started.state);
    let event_count = started.journal.events.len();

    let mut stale = harness.step_input(&anchors);
    stale.expected_authorization_sha256 = STALE_SHA.into();
    assert_eq!(
        harness.service.step(&stale).await,
        Err(ScheduledGraphControllerServiceError::StaleConsent)
    );

    let mut missing = harness.step_input(&anchors);
    missing.confirm_off_machine = false;
    assert_eq!(
        harness.service.step(&missing).await,
        Err(ScheduledGraphControllerServiceError::ConsentRequired)
    );

    let journal = harness.inspect();
    assert_eq!(journal.events.len(), event_count);
    assert_uncharged_and_uninvoked(&harness, &journal);
}

fn assert_uncharged_and_uninvoked(harness: &Harness, journal: &ScheduledGraphControllerJournal) {
    assert_eq!(journal.effectful_steps_reserved(), 0);
    assert_eq!(journal.cost_usd_micros_reserved(), 0);
    assert_eq!(harness.executor_calls.load(Ordering::Acquire), 0);
    assert_eq!(harness.materializer_calls.load(Ordering::Acquire), 0);
}

struct Harness {
    fixture: sqlite_support::Fixture,
    store: Arc<SqliteHubStore>,
    service: ScheduledGraphControllerService,
    start_input: StartScheduledGraphControllerInput,
    provider_request_id: String,
    executor_calls: Arc<AtomicUsize>,
    materializer_calls: Arc<AtomicUsize>,
}

impl Harness {
    fn new() -> Self {
        let fixture = sqlite_support::Fixture::new();
        let store = fixture.writer();
        let snapshot = store
            .snapshot_scheduled_graph_progress("graph-run-1")
            .expect("controller source progress");
        let provider_request_id = snapshot.nodes[0]
            .provider_request_id
            .clone()
            .expect("prepared initial request");
        let profile = execution_profile(&store, &provider_request_id);
        let start_input = start_input(&snapshot, profile);
        let executor_calls = Arc::new(AtomicUsize::new(0));
        let materializer_calls = Arc::new(AtomicUsize::new(0));
        let service = controller_service(
            store.clone(),
            executor_calls.clone(),
            materializer_calls.clone(),
        );
        Self {
            fixture,
            store,
            service,
            start_input,
            provider_request_id,
            executor_calls,
            materializer_calls,
        }
    }

    fn start(&self) -> ScheduledGraphControllerOutput {
        self.service
            .start(&self.start_input)
            .expect("start controller")
    }

    fn inspect(&self) -> ScheduledGraphControllerJournal {
        self.store
            .inspect_scheduled_graph_controller("graph-run-1")
            .expect("inspect controller")
    }

    fn step_input(&self, anchors: &AwaitingAnchors) -> StepScheduledGraphControllerInput {
        StepScheduledGraphControllerInput {
            graph_run_id: "graph-run-1".into(),
            core_bin_sha256: CORE_SHA.into(),
            expected_awaiting_event_sha256: anchors.awaiting_event_sha256.clone(),
            expected_provider_request_id: anchors.provider_request_id.clone(),
            expected_authorization_sha256: anchors.authorization_sha256.clone(),
            pricing_source: Arc::new(StaticPricing(self.fixture.pricing_json().into())),
            confirm_off_machine: true,
            confirm_predecessor_content: false,
            cancellation: Cancellation::default(),
            observed_at_ms: 1_001,
        }
    }
}

fn controller_service(
    store: Arc<SqliteHubStore>,
    executor_calls: Arc<AtomicUsize>,
    materializer_calls: Arc<AtomicUsize>,
) -> ScheduledGraphControllerService {
    controller_service_with_reconcile(
        store,
        executor_calls,
        materializer_calls,
        Arc::new(ReadyReconcile),
    )
}

fn controller_service_with_reconcile(
    store: Arc<SqliteHubStore>,
    executor_calls: Arc<AtomicUsize>,
    materializer_calls: Arc<AtomicUsize>,
    reconcile: Arc<dyn ScheduledGraphReconcilePort>,
) -> ScheduledGraphControllerService {
    ScheduledGraphControllerService::new(
        store,
        reconcile,
        Arc::new(ReadyAuthorize),
        Arc::new(ForbiddenMaterializer {
            calls: materializer_calls,
        }),
        Arc::new(OpenAiCodec),
        Arc::new(CountingExecutor {
            calls: executor_calls,
        }),
        Arc::new(InputTimeClock),
    )
}

fn start_input(
    snapshot: &ScheduledGraphProgressSnapshot,
    execution_profile: ScheduledGraphControllerExecutionProfile,
) -> StartScheduledGraphControllerInput {
    let max_effectful_steps = u16::try_from(snapshot.node_count).expect("bounded node count");
    StartScheduledGraphControllerInput {
        graph_run_id: snapshot.graph_run_id.clone(),
        expected_schedule_sha256: snapshot.schedule_sha256.clone(),
        core_bin_sha256: CORE_SHA.into(),
        max_total_cost_usd_micros: execution_profile
            .max_cost_usd_micros
            .checked_mul(u64::from(max_effectful_steps))
            .expect("bounded aggregate cost"),
        execution_profile,
        max_effectful_steps,
        observed_at_ms: 1_000,
    }
}

fn execution_profile(
    store: &SqliteHubStore,
    provider_request_id: &str,
) -> ScheduledGraphControllerExecutionProfile {
    let request = store
        .inspect_group_agent_scheduled_node_provider_request(provider_request_id)
        .expect("inspect prepared request");
    let candidate = &request.scheduled_contract.candidate;
    ScheduledGraphControllerExecutionProfile {
        endpoint: candidate.provider.endpoint.clone(),
        model: candidate.provider.model.clone(),
        max_output_tokens: u64::from(candidate.budgets.max_output_tokens),
        max_model_output_bytes: candidate.budgets.max_model_output_bytes as u64,
        max_model_events: u64::from(candidate.budgets.max_model_events),
        timeout_ms: candidate.budgets.timeout_ms,
        max_cost_usd_micros: candidate.budgets.max_cost_usd_micros,
        pricing_snapshot_sha256: candidate.budgets.pricing_snapshot_sha256.clone(),
        max_result_bytes: candidate.result.max_result_bytes as u64,
        profile_sha256: String::new(),
    }
    .seal()
    .expect("valid controller profile")
}

struct AwaitingAnchors {
    awaiting_event_sha256: String,
    execution_ordinal: usize,
    node_id: String,
    provider_request_id: String,
    authorization_sha256: String,
    snapshot_sha256: String,
    decision_sha256: String,
}

fn awaiting(state: &ScheduledGraphControllerState) -> AwaitingAnchors {
    let ScheduledGraphControllerState::AwaitingFreshConsent {
        awaiting_event_sha256,
        execution_ordinal,
        node_id,
        provider_request_id,
        authorization_sha256,
        snapshot_sha256,
        decision_sha256,
        ..
    } = state
    else {
        panic!("controller must await fresh consent: {state:?}");
    };
    AwaitingAnchors {
        awaiting_event_sha256: awaiting_event_sha256.clone(),
        execution_ordinal: *execution_ordinal,
        node_id: node_id.clone(),
        provider_request_id: provider_request_id.clone(),
        authorization_sha256: authorization_sha256.clone(),
        snapshot_sha256: snapshot_sha256.clone(),
        decision_sha256: decision_sha256.clone(),
    }
}

fn advance_input(observed_at_ms: u64) -> AdvanceScheduledGraphControllerInput {
    AdvanceScheduledGraphControllerInput {
        graph_run_id: "graph-run-1".into(),
        core_bin_sha256: CORE_SHA.into(),
        observed_at_ms,
    }
}

fn append_crash_dispatch_plan(
    harness: &Harness,
    journal: &ScheduledGraphControllerJournal,
    anchors: &AwaitingAnchors,
) -> ScheduledGraphControllerJournal {
    super::events::append(
        harness.store.as_ref(),
        journal,
        ScheduledGraphControllerEventPayload::DispatchPlanned {
            execution_ordinal: anchors.execution_ordinal,
            node_id: anchors.node_id.clone(),
            provider_request_id: anchors.provider_request_id.clone(),
            authorization_sha256: anchors.authorization_sha256.clone(),
            snapshot_sha256: anchors.snapshot_sha256.clone(),
            decision_sha256: anchors.decision_sha256.clone(),
            effectful_step_reservation: 1,
            reserved_cost_usd_micros: journal.header.execution_profile.max_cost_usd_micros,
            off_machine_consent_observed: true,
            predecessor_content_consent_observed: false,
        },
        1_001,
    )
    .expect("persist crash-point dispatch plan")
}

struct ReadyReconcile;

impl ScheduledGraphReconcilePort for ReadyReconcile {
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        let ready = snapshot.nodes.iter().find(|node| {
            node.lifecycle_status != Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
                || node.terminal_outcome != Some(GroupAgentNodeTerminalOutcome::Completed)
        });
        let (disposition, next_ordinal, next_node) = match ready {
            Some(node) => (
                ScheduledGraphReconcileDisposition::Ready,
                Some(node.execution_ordinal),
                Some(node.node_id.clone()),
            ),
            None => (ScheduledGraphReconcileDisposition::Completed, None, None),
        };
        ScheduledGraphReconcileDecision {
            v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
            progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
            graph_run_id: snapshot.graph_run_id.clone(),
            schedule_id: snapshot.schedule_id.clone(),
            schedule_sha256: snapshot.schedule_sha256.clone(),
            snapshot_sha256: snapshot.snapshot_sha256.clone(),
            disposition,
            next_execution_ordinal: next_ordinal,
            next_node_id: next_node,
            decision_sha256: String::new(),
        }
        .seal()
        .map_err(|_| ScheduledGraphReconcilePortError::InvalidDecision)
    }
}

struct ReadyAuthorize;

impl ScheduledReadyNodeReleasePort for ReadyAuthorize {
    fn authorize(
        &self,
        control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ) -> Result<GroupAgentScheduledReadyNodeDispatchAuthorization, ScheduledReadyNodeReleasePortError>
    {
        Ok(ready_authorization_support::authorization(control))
    }
}

struct ForbiddenMaterializer {
    calls: Arc<AtomicUsize>,
}

impl ScheduledGraphNodeMaterializationPort for ForbiddenMaterializer {
    fn materialize(
        &self,
        _input: &ScheduledGraphNodeMaterializationInput,
    ) -> Result<ScheduledGraphNodeMaterialization, ScheduledGraphNodeMaterializationPortError> {
        self.calls.fetch_add(1, Ordering::AcqRel);
        Err(ScheduledGraphNodeMaterializationPortError::Unavailable)
    }
}

struct OpenAiCodec;

impl GroupAgentNodeDispatchRequestCodec for OpenAiCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        OpenAiResponsesProvider::encode_request_bytes(model, request)
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        (self.encode_request(model, expected)? == actual)
            .then_some(())
            .ok_or_else(|| ProviderError::new("mismatch", "request bytes disagree", false))
    }
}

struct CountingExecutor {
    calls: Arc<AtomicUsize>,
}

impl ScheduledGraphReadyNodeExecutor for CountingExecutor {
    fn execute<'a>(
        &'a self,
        _input: &'a ExecuteGroupAgentScheduledReadyNodeDispatchInput,
    ) -> super::StepFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::AcqRel);
            Err(GroupAgentScheduledReadyNodeDispatchExecutionServiceError::CredentialUnavailable)
        })
    }
}
