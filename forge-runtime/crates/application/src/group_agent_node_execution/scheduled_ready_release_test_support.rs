use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use serde_json::{Value, json};

use crate::runtime_domain::{
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_VERSION,
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledReadyNodeDispatchAuthorization,
    GroupAgentScheduledReadyNodeDispatchReleaseControl, HubEntity, HubStoreError,
    SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION, SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
    SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION, ScheduledGraphProgressNode,
    ScheduledGraphProgressSnapshot, ScheduledGraphProgressStore, ScheduledGraphReconcileDecision,
    ScheduledGraphReconcileDisposition, ScheduledGraphReconcilePort,
    ScheduledGraphReconcilePortError, ScheduledReadyNodeReleasePort,
    ScheduledReadyNodeReleasePortError, ScheduledReadyNodeReleaseSource,
    ScheduledReadyNodeReleaseStore,
};

use super::super::{
    GroupAgentScheduledNodeProviderRequestService,
    PrepareGroupAgentScheduledNodeProviderRequestInput, ScheduledReadyNodeReleaseService,
    scheduled_provider_request_tests::{SpyCodec, SpyHub},
    scheduled_release_tests::authorization as legacy_authorization,
};

const BODY: &[u8] = br#"{"model":"fixture"}"#;
type Trace = Arc<Mutex<Vec<&'static str>>>;

pub(super) struct Fixture {
    pub(super) graph_run_id: String,
    pub(super) snapshot: ScheduledGraphProgressSnapshot,
    pub(super) source: ScheduledReadyNodeReleaseSource,
}

pub(super) fn fixture() -> Fixture {
    let hub = Arc::new(SpyHub::new());
    let prepare = GroupAgentScheduledNodeProviderRequestService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        Arc::new(SpyCodec::new(BODY.to_vec())),
    );
    let request = prepare
        .prepare(&PrepareGroupAgentScheduledNodeProviderRequestInput {
            scheduled_contract_id: hub.contract_id(),
            idempotency_key: "ready-release-test-request".into(),
            prepared_at_ms: 91,
        })
        .expect("prepared request")
        .inspection;
    source_fixture(&hub, request)
}

fn source_fixture(
    hub: &Arc<SpyHub>,
    request: crate::runtime_domain::GroupAgentScheduledNodeProviderRequestInspection,
) -> Fixture {
    let graph_run_id = request.record.graph_run_id.clone();
    let graph_run = hub
        .inspect_group_agent_graph_run(&graph_run_id)
        .expect("Run");
    let graph = hub
        .inspect_group_agent_graph(&graph_run.run.graph_id)
        .expect("Graph");
    let schedule = hub
        .inspect_group_agent_graph_execution_schedule(&request.record.schedule_id)
        .expect("schedule");
    let snapshot = progress_snapshot(&schedule, &request);
    let source = ScheduledReadyNodeReleaseSource {
        progress_snapshot: snapshot.clone(),
        graph_run,
        graph,
        schedule,
        selected_provider_request: request,
        direct_predecessor_receipts: Vec::new(),
        predecessor_content_artifact: None,
    };
    Fixture {
        graph_run_id,
        snapshot,
        source,
    }
}

fn progress_snapshot(
    schedule: &crate::runtime_domain::GroupAgentGraphExecutionScheduleInspection,
    request: &crate::runtime_domain::GroupAgentScheduledNodeProviderRequestInspection,
) -> ScheduledGraphProgressSnapshot {
    let nodes = schedule
        .schedule
        .nodes
        .iter()
        .map(|node| progress_node(node, request))
        .collect();
    ScheduledGraphProgressSnapshot {
        v: SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: schedule.schedule.graph_run_id.clone(),
        graph_id: schedule.schedule.graph_id.clone(),
        schedule_id: schedule.schedule.schedule_id.clone(),
        schedule_sha256: schedule.schedule.schedule_sha256.clone(),
        node_count: schedule.schedule.node_count,
        execution_mode: schedule.schedule.execution_mode,
        max_in_flight_nodes: schedule.schedule.max_in_flight_nodes,
        progression_policy: schedule.schedule.progression_policy,
        attempt_policy: schedule.schedule.attempt_policy,
        failure_policy: schedule.schedule.failure_policy,
        nodes,
        snapshot_sha256: String::new(),
    }
    .seal()
    .expect("progress snapshot")
}

fn progress_node(
    node: &crate::runtime_domain::GroupAgentGraphExecutionScheduleNode,
    request: &crate::runtime_domain::GroupAgentScheduledNodeProviderRequestInspection,
) -> ScheduledGraphProgressNode {
    let selected = node.execution_ordinal == request.record.execution_ordinal;
    ScheduledGraphProgressNode {
        execution_ordinal: node.execution_ordinal,
        node_id: node.node_id.clone(),
        attempt: node.attempt,
        candidate_id: selected.then(|| request.record.scheduled_contract_id.clone()),
        candidate_sha256: selected.then(|| request.record.scheduled_contract_sha256.clone()),
        provider_request_id: selected.then(|| request.record.provider_request_id.clone()),
        prepared_request_sha256: selected.then(|| request.record.prepared_request_sha256.clone()),
        lifecycle_status: None,
        terminal_outcome: None,
        terminal_receipt_sha256: None,
    }
}

pub(super) fn ready(snapshot: &ScheduledGraphProgressSnapshot) -> ScheduledGraphReconcileDecision {
    decision(
        snapshot,
        ScheduledGraphReconcileDisposition::Ready,
        Some((0, "frontend")),
    )
}

pub(super) fn non_ready(
    snapshot: &ScheduledGraphProgressSnapshot,
) -> ScheduledGraphReconcileDecision {
    decision(
        snapshot,
        ScheduledGraphReconcileDisposition::ClaimedUnknown,
        None,
    )
}

fn decision(
    snapshot: &ScheduledGraphProgressSnapshot,
    disposition: ScheduledGraphReconcileDisposition,
    selected: Option<(usize, &str)>,
) -> ScheduledGraphReconcileDecision {
    ScheduledGraphReconcileDecision {
        v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: snapshot.graph_run_id.clone(),
        schedule_id: snapshot.schedule_id.clone(),
        schedule_sha256: snapshot.schedule_sha256.clone(),
        snapshot_sha256: snapshot.snapshot_sha256.clone(),
        disposition,
        next_execution_ordinal: selected.map(|value| value.0),
        next_node_id: selected.map(|value| value.1.into()),
        decision_sha256: String::new(),
    }
    .seal()
    .expect("decision")
}

pub(super) struct Harness {
    pub(super) service: ScheduledReadyNodeReleaseService,
    pub(super) progress: Arc<FakeResult<ScheduledGraphProgressSnapshot, HubStoreError>>,
    pub(super) sources: Arc<FakeSources>,
    pub(super) reconcile:
        Arc<FakeResult<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError>>,
    pub(super) authorize: Arc<FakeAuthorize>,
    trace: Trace,
}

impl Harness {
    pub(super) fn trace(&self) -> Vec<&'static str> {
        self.trace.lock().expect("trace").clone()
    }
}

pub(super) fn harness(
    fixture: &Fixture,
    sources: Vec<ScheduledReadyNodeReleaseSource>,
    reconcile: Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError>,
    mode: AuthMode,
) -> Harness {
    make_harness(Ok(fixture.snapshot.clone()), Ok(sources), reconcile, mode)
}

pub(super) fn harness_with_progress_error(fixture: &Fixture, error: HubStoreError) -> Harness {
    make_harness(
        Err(error),
        Ok(vec![fixture.source.clone()]),
        Ok(ready(&fixture.snapshot)),
        AuthMode::Valid,
    )
}

pub(super) fn make_harness(
    progress_result: Result<ScheduledGraphProgressSnapshot, HubStoreError>,
    source_values: Result<Vec<ScheduledReadyNodeReleaseSource>, HubStoreError>,
    reconcile_result: Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError>,
    mode: AuthMode,
) -> Harness {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let progress = Arc::new(FakeResult::new(progress_result, trace.clone(), "progress"));
    let sources = Arc::new(FakeSources::new(source_values, trace.clone()));
    let reconcile = Arc::new(FakeResult::new(
        reconcile_result,
        trace.clone(),
        "reconcile",
    ));
    let authorize = Arc::new(FakeAuthorize::new(mode, trace.clone()));
    let service = ScheduledReadyNodeReleaseService::new(
        progress.clone(),
        sources.clone(),
        reconcile.clone(),
        authorize.clone(),
    );
    Harness {
        service,
        progress,
        sources,
        reconcile,
        authorize,
        trace,
    }
}

pub(super) struct FakeResult<T, E> {
    result: Result<T, E>,
    calls: AtomicUsize,
    trace: Trace,
    label: &'static str,
}

impl<T: Clone, E: Clone> FakeResult<T, E> {
    fn new(result: Result<T, E>, trace: Trace, label: &'static str) -> Self {
        Self {
            result,
            calls: AtomicUsize::new(0),
            trace,
            label,
        }
    }
    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    fn get(&self) -> Result<T, E> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.trace.lock().expect("trace").push(self.label);
        self.result.clone()
    }
}

impl ScheduledGraphProgressStore for FakeResult<ScheduledGraphProgressSnapshot, HubStoreError> {
    fn snapshot_scheduled_graph_progress(
        &self,
        _id: &str,
    ) -> Result<ScheduledGraphProgressSnapshot, HubStoreError> {
        self.get()
    }
}

pub(super) struct FakeSources {
    values: Result<Vec<ScheduledReadyNodeReleaseSource>, HubStoreError>,
    calls: AtomicUsize,
    trace: Trace,
}

impl FakeSources {
    fn new(
        values: Result<Vec<ScheduledReadyNodeReleaseSource>, HubStoreError>,
        trace: Trace,
    ) -> Self {
        Self {
            values,
            calls: AtomicUsize::new(0),
            trace,
        }
    }
    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ScheduledReadyNodeReleaseStore for FakeSources {
    fn inspect_scheduled_ready_node_release(
        &self,
        graph_run_id: &str,
        sha256: &str,
        ordinal: usize,
        node_id: &str,
    ) -> Result<ScheduledReadyNodeReleaseSource, HubStoreError> {
        let index = self.calls.fetch_add(1, Ordering::SeqCst);
        self.trace.lock().expect("trace").push("source");
        let values = self.values.as_ref().map_err(Clone::clone)?;
        let value = values
            .get(index)
            .or_else(|| values.last())
            .expect("source fixture");
        let selected = &value.selected_provider_request.record;
        let exact = graph_run_id == value.progress_snapshot.graph_run_id
            && sha256 == value.progress_snapshot.snapshot_sha256
            && ordinal == selected.execution_ordinal
            && node_id == selected.node_id;
        exact.then(|| value.clone()).ok_or_else(corrupt)
    }
}

impl ScheduledGraphReconcilePort
    for FakeResult<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError>
{
    fn decide(
        &self,
        _snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError> {
        self.get()
    }
}

#[derive(Clone, Copy)]
pub(super) enum AuthMode {
    Valid,
    Unavailable,
    Invalid,
}

pub(super) struct FakeAuthorize {
    mode: AuthMode,
    calls: AtomicUsize,
    trace: Trace,
}

impl FakeAuthorize {
    fn new(mode: AuthMode, trace: Trace) -> Self {
        Self {
            mode,
            calls: AtomicUsize::new(0),
            trace,
        }
    }
    pub(super) fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ScheduledReadyNodeReleasePort for FakeAuthorize {
    fn authorize(
        &self,
        control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ) -> Result<GroupAgentScheduledReadyNodeDispatchAuthorization, ScheduledReadyNodeReleasePortError>
    {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.trace.lock().expect("trace").push("authorize");
        match self.mode {
            AuthMode::Valid => Ok(authorization(control)),
            AuthMode::Unavailable => Err(ScheduledReadyNodeReleasePortError::Unavailable),
            AuthMode::Invalid => Err(ScheduledReadyNodeReleasePortError::InvalidAuthorization),
        }
    }
}

fn authorization(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> GroupAgentScheduledReadyNodeDispatchAuthorization {
    let legacy = legacy_control(control);
    let mut value = serde_json::to_value(legacy_authorization(&legacy)).expect("legacy policy");
    let fields = value.as_object_mut().expect("authorization fields");
    let Value::Object(updates) = json!({
        "v": GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_VERSION,
        "dispatch_authorization_protocol_version": GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
        "release_control_snapshot_sha256": control.snapshot_sha256,
        "progress_snapshot_sha256": control.progress_snapshot.snapshot_sha256,
        "reconcile_decision_sha256": control.reconcile_decision.decision_sha256,
        "maximum_future_node_releases": 1, "release_requirements": ready_requirements(),
        "authorization_id": "", "authorization_sha256": ""
    }) else {
        unreachable!()
    };
    fields.extend(updates);
    serde_json::from_value::<GroupAgentScheduledReadyNodeDispatchAuthorization>(value)
        .expect("ready policy fields")
        .seal()
        .expect("sealed ready policy")
}

fn legacy_control(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> GroupAgentScheduledNodeDispatchReleaseControl {
    let mut legacy = GroupAgentScheduledNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: control.graph_run.clone(),
        journal_events: control.journal_events.clone(),
        control_snapshot: control.control_snapshot.clone(),
        schedule_record: control.schedule_record.clone(),
        schedule: control.schedule.clone(),
        scheduled_contract_record: control.scheduled_contract_record.clone(),
        scheduled_contract: control.scheduled_contract.clone(),
        provider_request: control.provider_request.clone(),
        provider_request_json: control.provider_request_json.clone(),
        snapshot_sha256: String::new(),
    };
    legacy.snapshot_sha256 = legacy.expected_sha256().expect("legacy control digest");
    legacy.validate().expect("valid legacy control");
    legacy
}

fn ready_requirements() -> Value {
    json!({
        "consent": "fresh_off_machine", "consent_contract_version": 1,
        "credential_preflight": "header_safe_environment",
        "destination_preflight": "exact_registered_destination",
        "pricing_preflight": "exact_snapshot_within_max_cost",
        "project_lane_claim": "global_exclusive_until_terminal",
        "provider_health_check": "forbidden",
        "atomic_transition": "exact_progress_snapshot_selected_node_admission_release_and_lane_claim",
        "successor": "exact_ordered_direct_predecessor_terminal_receipts_before_successor"
    })
}

pub(super) fn not_found() -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentGraphRun,
        id: "missing".into(),
    }
}

pub(super) fn unavailable() -> HubStoreError {
    HubStoreError::Unavailable {
        message: "secret path".into(),
    }
}
pub(super) fn conflict() -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraphRun,
        message: "secret conflict".into(),
    }
}
pub(super) fn corrupt() -> HubStoreError {
    HubStoreError::Corrupt {
        message: "secret bytes".into(),
    }
}
