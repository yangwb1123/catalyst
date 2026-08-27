use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use crate::runtime_domain::{
    AppendScheduledGraphControllerDisposition, AppendScheduledGraphControllerResult,
    GroupAgentGraphExecutionScheduleStore, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractStore,
    GroupAgentScheduledNodeProviderRequestStore, HubStoreError,
    PrepareGroupAgentScheduledNodeProviderRequestDisposition, ScheduledGraphControllerEvent,
    ScheduledGraphControllerEventPayload, ScheduledGraphControllerHeader,
    ScheduledGraphControllerJournal, ScheduledGraphControllerStore, ScheduledGraphProgressStore,
};

use super::*;

const PROVIDER_TABLE: &str = "group_agent_graph_scheduled_node_provider_requests";

#[test]
fn planned_prepare_crash_reenters_with_exact_key_to_observed_consent() {
    let harness = CandidateHarness::new();
    let planned = harness.crash_after_plan();
    let key = exact_planned_key(&planned);

    let output = harness
        .service
        .advance(&advance_input(1_001))
        .expect("reenter planned prepare");

    let anchors = awaiting(&output.state);
    assert_prepare_observed(
        &output.journal,
        &harness.contract,
        &anchors.provider_request_id,
    );
    assert_eq!(harness.stored_key(&anchors.provider_request_id), key);
    assert_eq!(harness.provider_count(), 1);
    harness.assert_no_effect_ports();
}

#[test]
fn durable_prepare_without_observed_append_replays_one_exact_request() {
    let harness = CandidateHarness::new();
    let planned = harness.crash_after_plan();
    let key = exact_planned_key(&planned);
    let request = sqlite_group_agent_scheduled_node_provider_request_support::request(
        &harness.contract,
        &key,
        1_001,
    );
    let expected_id = request.provider_request_id.clone();
    let prepared = harness
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("durably prepare before observed append");
    assert_eq!(
        prepared.disposition,
        PrepareGroupAgentScheduledNodeProviderRequestDisposition::Created
    );
    assert!(matches!(
        harness.inspect().head().payload,
        ScheduledGraphControllerEventPayload::PreparePlanned { .. }
    ));

    let output = harness
        .service
        .advance(&advance_input(1_002))
        .expect("observe already durable request");
    let anchors = awaiting(&output.state);
    assert_eq!(anchors.provider_request_id, expected_id);
    assert_prepare_observed(&output.journal, &harness.contract, &expected_id);
    assert_eq!(harness.stored_key(&expected_id), key);
    assert_eq!(harness.provider_count(), 1);
    harness.assert_no_effect_ports();
}

#[test]
fn durable_prepare_observed_reentry_reaches_consent_without_reprepare() {
    let harness = CandidateHarness::new();
    let started = harness.start_only();
    let key = expected_prepare_key(&started);
    let planned = append_plan(
        &harness,
        &started,
        &harness.contract.record.contract_id,
        &key,
    )
    .expect("append prepare plan");
    let request = sqlite_group_agent_scheduled_node_provider_request_support::request(
        &harness.contract,
        &key,
        1_002,
    );
    let expected_id = request.provider_request_id.clone();
    harness
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("durably prepare request");
    let observed = events::append(
        harness.store.as_ref(),
        &planned,
        prepare_observed_payload(&harness, &expected_id),
        1_003,
    )
    .expect("append durable prepare observation");
    assert!(matches!(
        observed.head().payload,
        ScheduledGraphControllerEventPayload::PrepareObserved { .. }
    ));

    let output = harness
        .service
        .advance(&advance_input(1_004))
        .expect("reenter observed prepare");
    assert_eq!(awaiting(&output.state).provider_request_id, expected_id);
    assert_eq!(harness.provider_count(), 1);
    harness.assert_no_effect_ports();
}

#[test]
fn mismatched_prepare_plan_key_or_source_fails_closed_before_prepare() {
    let harness = CandidateHarness::new();
    let started = harness.start_only();
    let bad_key = "controller-tampered-prepare-0";
    assert_eq!(
        append_plan(
            &harness,
            &started,
            &harness.contract.record.contract_id,
            bad_key
        ),
        Err(ScheduledGraphControllerServiceError::CorruptEvidence)
    );
    assert_eq!(harness.inspect(), started);

    let key = expected_prepare_key(&started);
    let mismatched = append_plan(
        &harness,
        &started,
        "scheduled-node-contract-substituted",
        &key,
    )
    .expect("persist structurally valid substituted source");
    assert!(matches!(
        mismatched.head().payload,
        ScheduledGraphControllerEventPayload::PreparePlanned { .. }
    ));
    assert_eq!(
        harness.service.advance(&advance_input(1_002)),
        Err(ScheduledGraphControllerServiceError::CorruptEvidence)
    );
    assert_eq!(harness.provider_count(), 0);
    harness.assert_no_effect_ports();
}

#[test]
fn append_accepts_identical_replay_when_store_already_has_a_descendant() {
    let harness = CandidateHarness::new();
    let started = harness.start_only();
    let payload = prepare_plan_payload(
        &harness,
        &harness.contract.record.contract_id,
        &expected_prepare_key(&started),
    );
    let submitted = events::next(&started, payload.clone(), 1_001).expect("submitted event");
    let mut descendant = started.clone();
    descendant.events.push(submitted.clone());
    let observed = prepare_observed_event(&harness, &descendant);
    descendant.events.push(observed);
    descendant.validate().expect("valid descendant journal");
    let store = DescendantReplayStore {
        expected: submitted,
        journal: descendant.clone(),
    };

    let replayed =
        events::append(&store, &started, payload, 1_001).expect("identical replay with descendant");
    assert_eq!(replayed, descendant);
}

struct CandidateHarness {
    fixture: sqlite_group_agent_graph_run_support::Fixture,
    store: Arc<SqliteHubStore>,
    service: ScheduledGraphControllerService,
    input: StartScheduledGraphControllerInput,
    contract: GroupAgentScheduledNodeContractInspection,
    executor_calls: Arc<AtomicUsize>,
    materializer_calls: Arc<AtomicUsize>,
}

impl CandidateHarness {
    fn new() -> Self {
        let fixture = sqlite_group_agent_graph_execution_schedule_support::prepared_fixture();
        let schedule_request = sqlite_group_agent_graph_execution_schedule_support::request(
            &fixture,
            "prepare-crash-schedule-key",
            40,
        );
        fixture
            .store
            .admit_group_agent_graph_execution_schedule(&schedule_request)
            .expect("admit prepare crash schedule");
        let admission = sqlite_group_agent_scheduled_node_contract_support::admission(
            schedule_request,
            "prepare-crash-contract-key",
            50,
        );
        let contract = fixture
            .store
            .admit_group_agent_scheduled_node_contract(&admission)
            .expect("admit candidate without provider request")
            .inspection;
        let store = Arc::new(fixture.store.clone());
        let snapshot = store
            .snapshot_scheduled_graph_progress("graph-run-1")
            .expect("candidate-only progress");
        assert_eq!(
            snapshot.nodes[0].candidate_id.as_deref(),
            Some(&*contract.record.contract_id)
        );
        assert!(snapshot.nodes[0].provider_request_id.is_none());
        let input = start_input(&snapshot, execution_profile(&contract.candidate));
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
            input,
            contract,
            executor_calls,
            materializer_calls,
        }
    }

    fn crash_after_plan(&self) -> ScheduledGraphControllerJournal {
        let crashing = ScheduledGraphControllerService::new(
            self.store.clone(),
            Arc::new(ReadyReconcile),
            Arc::new(ReadyAuthorize),
            Arc::new(ForbiddenMaterializer {
                calls: self.materializer_calls.clone(),
            }),
            Arc::new(FailingCodec),
            Arc::new(CountingExecutor {
                calls: self.executor_calls.clone(),
            }),
            Arc::new(InputTimeClock),
        );
        let result = crashing.start(&self.input);
        let journal = self.inspect();
        assert_eq!(
            result,
            Err(ScheduledGraphControllerServiceError::PreparationFailed)
        );
        assert_eq!(journal.events.len(), 2);
        assert!(matches!(
            journal.head().payload,
            ScheduledGraphControllerEventPayload::PreparePlanned { .. }
        ));
        assert_eq!(self.provider_count(), 0);
        journal
    }

    fn start_only(&self) -> ScheduledGraphControllerJournal {
        let observation = ready_observation(self.store.as_ref());
        let header = build_header(&self.input, &observation).expect("candidate controller header");
        let event = start_event(&header, &observation, 1_000).expect("candidate start event");
        let result = self
            .store
            .start_scheduled_graph_controller(&header, &event)
            .expect("start candidate controller");
        assert_eq!(
            result.disposition,
            AppendScheduledGraphControllerDisposition::Stored
        );
        result.journal
    }

    fn inspect(&self) -> ScheduledGraphControllerJournal {
        self.store
            .inspect_scheduled_graph_controller("graph-run-1")
            .expect("inspect candidate controller")
    }

    fn provider_count(&self) -> i64 {
        self.fixture.row_count(PROVIDER_TABLE)
    }

    fn stored_key(&self, provider_request_id: &str) -> String {
        self.fixture
            .connection()
            .query_row(
                &format!("SELECT idempotency_key FROM {PROVIDER_TABLE} WHERE id=?1"),
                [provider_request_id],
                |row| row.get(0),
            )
            .expect("stored preparation idempotency key")
    }

    fn assert_no_effect_ports(&self) {
        assert_eq!(self.executor_calls.load(Ordering::Acquire), 0);
        assert_eq!(self.materializer_calls.load(Ordering::Acquire), 0);
    }
}

struct FailingCodec;

impl GroupAgentNodeDispatchRequestCodec for FailingCodec {
    fn encode_request(
        &self,
        _model: &str,
        _request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        Err(ProviderError::new(
            "prepare_crash",
            "simulated crash before durable prepare",
            false,
        ))
    }

    fn validate_exact_request(
        &self,
        _model: &str,
        _expected: &ModelRequest,
        _actual: &[u8],
    ) -> Result<(), ProviderError> {
        Err(ProviderError::new(
            "prepare_crash",
            "simulated crash before durable prepare",
            false,
        ))
    }
}

struct DescendantReplayStore {
    expected: ScheduledGraphControllerEvent,
    journal: ScheduledGraphControllerJournal,
}

impl ScheduledGraphControllerStore for DescendantReplayStore {
    fn start_scheduled_graph_controller(
        &self,
        _header: &ScheduledGraphControllerHeader,
        _event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
        panic!("replay test does not start a controller")
    }

    fn append_scheduled_graph_controller_event(
        &self,
        event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError> {
        assert_eq!(event, &self.expected);
        Ok(AppendScheduledGraphControllerResult {
            disposition: AppendScheduledGraphControllerDisposition::Replayed,
            journal: self.journal.clone(),
        })
    }

    fn inspect_scheduled_graph_controller(
        &self,
        _graph_run_id: &str,
    ) -> Result<ScheduledGraphControllerJournal, HubStoreError> {
        Ok(self.journal.clone())
    }
}

fn execution_profile(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> ScheduledGraphControllerExecutionProfile {
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
    .expect("valid candidate execution profile")
}

fn append_plan(
    harness: &CandidateHarness,
    journal: &ScheduledGraphControllerJournal,
    contract_id: &str,
    key: &str,
) -> Result<ScheduledGraphControllerJournal, ScheduledGraphControllerServiceError> {
    events::append(
        harness.store.as_ref(),
        journal,
        prepare_plan_payload(harness, contract_id, key),
        1_001,
    )
}

fn prepare_plan_payload(
    harness: &CandidateHarness,
    contract_id: &str,
    key: &str,
) -> ScheduledGraphControllerEventPayload {
    ScheduledGraphControllerEventPayload::PreparePlanned {
        execution_ordinal: 0,
        node_id: harness.contract.record.node_id.clone(),
        contract_id: contract_id.into(),
        idempotency_key: key.into(),
    }
}

fn prepare_observed_event(
    harness: &CandidateHarness,
    planned: &ScheduledGraphControllerJournal,
) -> ScheduledGraphControllerEvent {
    events::next(
        planned,
        prepare_observed_payload(harness, "scheduled-node-provider-request-replayed"),
        1_002,
    )
    .expect("descendant observed event")
}

fn prepare_observed_payload(
    harness: &CandidateHarness,
    provider_request_id: &str,
) -> ScheduledGraphControllerEventPayload {
    ScheduledGraphControllerEventPayload::PrepareObserved {
        execution_ordinal: 0,
        node_id: harness.contract.record.node_id.clone(),
        contract_id: harness.contract.record.contract_id.clone(),
        provider_request_id: provider_request_id.into(),
    }
}

fn exact_planned_key(journal: &ScheduledGraphControllerJournal) -> String {
    let ScheduledGraphControllerEventPayload::PreparePlanned {
        idempotency_key, ..
    } = &journal.head().payload
    else {
        panic!("controller head must be PreparePlanned");
    };
    assert_eq!(idempotency_key, &expected_prepare_key(journal));
    idempotency_key.clone()
}

fn expected_prepare_key(journal: &ScheduledGraphControllerJournal) -> String {
    format!("controller-{}-prepare-0", journal.header.controller_sha256)
}

fn assert_prepare_observed(
    journal: &ScheduledGraphControllerJournal,
    contract: &GroupAgentScheduledNodeContractInspection,
    provider_request_id: &str,
) {
    assert!(journal.events.iter().any(|event| matches!(
        &event.payload,
        ScheduledGraphControllerEventPayload::PrepareObserved {
            execution_ordinal: 0,
            node_id,
            contract_id,
            provider_request_id: observed,
        } if node_id == &contract.record.node_id
            && contract_id == &contract.record.contract_id
            && observed == provider_request_id
    )));
    assert!(matches!(
        journal.head().payload,
        ScheduledGraphControllerEventPayload::AwaitingFreshConsent { .. }
    ));
}
