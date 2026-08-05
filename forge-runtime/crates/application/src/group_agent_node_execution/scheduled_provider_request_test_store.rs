use std::{
    collections::BTreeMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
};

use serde_json::Value;

use crate::runtime_domain::{
    AdmitGroupAgentGraphExecutionSchedule, AdmitGroupAgentGraphExecutionScheduleResult,
    AdmitGroupAgentScheduledNodeContractCandidate, AdmitGroupAgentScheduledNodeContractResult,
    BeginGroupAgentGraphRun, BeginGroupAgentGraphRunResult, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleRecord,
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphInspection, GroupAgentGraphRecord,
    GroupAgentGraphRunInspection, GroupAgentGraphRunRecord, GroupAgentGraphRunStore,
    GroupAgentGraphStatus, GroupAgentGraphStore, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeContractRecord, GroupAgentScheduledNodeContractStore,
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestRecord,
    GroupAgentScheduledNodeProviderRequestStore, HubEntity, HubStoreError, ModelRequest,
    PrepareGroupAgentGraph, PrepareGroupAgentGraphResult,
    PrepareGroupAgentScheduledNodeProviderRequest,
    PrepareGroupAgentScheduledNodeProviderRequestResult, ProviderError,
};

use super::{contract_record, result, source};
use crate::group_agent_node_execution::GroupAgentNodeDispatchRequestCodec;

pub(crate) struct SpyCodec {
    body: Vec<u8>,
    calls: Mutex<Vec<&'static str>>,
}

impl SpyCodec {
    pub(crate) fn new(body: Vec<u8>) -> Self {
        Self {
            body,
            calls: Mutex::new(Vec::new()),
        }
    }

    pub(crate) fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().expect("codec calls").clone()
    }
}

impl GroupAgentNodeDispatchRequestCodec for SpyCodec {
    fn encode_request(
        &self,
        _model: &str,
        _request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        self.calls.lock().expect("codec calls").push("encode");
        Ok(self.body.clone())
    }

    fn validate_exact_request(
        &self,
        _model: &str,
        _expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        self.calls
            .lock()
            .expect("codec calls")
            .push("validate_exact");
        (actual == self.body).then_some(()).ok_or_else(|| {
            ProviderError::new("spy_body_mismatch", "request body was re-encoded", false)
        })
    }
}

pub(crate) struct SpyHub {
    graph: GroupAgentGraphInspection,
    run: GroupAgentGraphRunInspection,
    schedule: GroupAgentGraphExecutionScheduleInspection,
    contract: GroupAgentScheduledNodeContractInspection,
    captured_request: Mutex<Option<GroupAgentScheduledNodeProviderRequestInspection>>,
    mutation_calls: AtomicUsize,
}

impl SpyHub {
    pub(crate) fn new() -> Self {
        Self::new_with_pricing_policy(
            "4444444444444444444444444444444444444444444444444444444444444444",
            1_000_000,
        )
    }

    pub(crate) fn new_with_pricing_policy(
        pricing_snapshot_sha256: &str,
        max_cost_usd_micros: u64,
    ) -> Self {
        let run = super::pristine_run();
        let control = current_control(&run);
        let schedule = schedule(&control);
        let contract = scheduled_contract(
            &control,
            &schedule,
            pricing_snapshot_sha256,
            max_cost_usd_micros,
        );
        Self {
            graph: graph(&control),
            run,
            schedule,
            contract,
            captured_request: Mutex::new(None),
            mutation_calls: AtomicUsize::new(0),
        }
    }

    pub(crate) fn captured_body(&self) -> Vec<u8> {
        self.captured_request
            .lock()
            .expect("captured request")
            .as_ref()
            .map(|inspection| inspection.provider_request_body.clone())
            .expect("request captured")
    }

    pub(crate) fn contract_id(&self) -> String {
        self.contract.record.contract_id.clone()
    }

    pub(crate) fn set_contract(&mut self, contract: GroupAgentScheduledNodeContractInspection) {
        self.contract = contract;
    }

    pub(crate) fn contract(&self) -> GroupAgentScheduledNodeContractInspection {
        self.contract.clone()
    }

    pub(crate) fn mutation_calls(&self) -> usize {
        self.mutation_calls.load(Ordering::SeqCst)
    }

    pub(crate) fn reset_mutation_calls(&self) {
        self.mutation_calls.store(0, Ordering::SeqCst);
    }

    fn request(&self) -> MutexGuard<'_, Option<GroupAgentScheduledNodeProviderRequestInspection>> {
        self.captured_request.lock().expect("captured request")
    }

    fn record_mutation(&self) {
        self.mutation_calls.fetch_add(1, Ordering::SeqCst);
    }
}

impl GroupAgentGraphStore for SpyHub {
    fn prepare_group_agent_graph(
        &self,
        _request: &PrepareGroupAgentGraph,
    ) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
        self.record_mutation();
        Err(unavailable("Graph preparation is outside this test"))
    }

    fn inspect_group_agent_graph(
        &self,
        graph_id: &str,
    ) -> Result<GroupAgentGraphInspection, HubStoreError> {
        exact(graph_id, &self.graph.graph.graph_id, &self.graph)
    }

    fn list_group_agent_graphs(
        &self,
        _group_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentGraphRecord>, HubStoreError> {
        Ok(vec![self.graph.graph.clone()])
    }
}

impl GroupAgentGraphRunStore for SpyHub {
    fn begin_group_agent_graph_run(
        &self,
        _request: &BeginGroupAgentGraphRun,
    ) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
        self.record_mutation();
        Err(unavailable("Graph Run preparation is outside this test"))
    }

    fn inspect_group_agent_graph_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
        exact(graph_run_id, &self.run.run.graph_run_id, &self.run)
    }

    fn list_group_agent_graph_runs(
        &self,
        _graph_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError> {
        Ok(vec![self.run.run.clone()])
    }
}

impl GroupAgentGraphExecutionScheduleStore for SpyHub {
    fn admit_group_agent_graph_execution_schedule(
        &self,
        _request: &AdmitGroupAgentGraphExecutionSchedule,
    ) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError> {
        self.record_mutation();
        Err(unavailable("schedule admission is outside this test"))
    }

    fn inspect_group_agent_graph_execution_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
        exact(
            schedule_id,
            &self.schedule.record.schedule_id,
            &self.schedule,
        )
    }

    fn list_group_agent_graph_execution_schedules(
        &self,
        _graph_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentGraphExecutionScheduleRecord>, HubStoreError> {
        Ok(vec![self.schedule.record.clone()])
    }
}

impl GroupAgentScheduledNodeContractStore for SpyHub {
    fn admit_group_agent_scheduled_node_contract(
        &self,
        _request: &AdmitGroupAgentScheduledNodeContractCandidate,
    ) -> Result<AdmitGroupAgentScheduledNodeContractResult, HubStoreError> {
        self.record_mutation();
        Err(unavailable("contract admission is outside this test"))
    }

    fn inspect_group_agent_scheduled_node_contract(
        &self,
        contract_id: &str,
    ) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
        exact(
            contract_id,
            &self.contract.record.contract_id,
            &self.contract,
        )
    }

    fn list_group_agent_scheduled_node_contracts(
        &self,
        _graph_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentScheduledNodeContractRecord>, HubStoreError> {
        Ok(vec![self.contract.record.clone()])
    }
}

impl GroupAgentScheduledNodeProviderRequestStore for SpyHub {
    fn prepare_group_agent_scheduled_node_provider_request(
        &self,
        request: &PrepareGroupAgentScheduledNodeProviderRequest,
    ) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError> {
        self.record_mutation();
        request
            .validate()
            .map_err(|error| conflict(&error.to_string()))?;
        let result = result(request, self.contract.clone());
        *self.request() = Some(result.inspection.clone());
        Ok(result)
    }

    fn inspect_group_agent_scheduled_node_provider_request(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
        let request = self.request();
        request
            .as_ref()
            .filter(|value| value.record.provider_request_id == provider_request_id)
            .cloned()
            .ok_or_else(|| {
                not_found(
                    HubEntity::GroupAgentScheduledNodeProviderRequest,
                    provider_request_id,
                )
            })
    }

    fn list_group_agent_scheduled_node_provider_requests(
        &self,
        _graph_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentScheduledNodeProviderRequestRecord>, HubStoreError> {
        Ok(self
            .request()
            .as_ref()
            .map(|value| vec![value.record.clone()])
            .unwrap_or_default())
    }
}

fn graph(
    control: &crate::runtime_domain::GroupAgentGraphControlSnapshot,
) -> GroupAgentGraphInspection {
    let manifest_json = sorted_json(&control.manifest);
    GroupAgentGraphInspection {
        v: control.manifest.v,
        graph: GroupAgentGraphRecord {
            v: control.manifest.v,
            graph_id: control.graph_id.clone(),
            group_run_id: control.manifest.source.group_run_id.clone(),
            status: GroupAgentGraphStatus::Prepared,
            source_snapshot_sha256: control.source_snapshot_sha256.clone(),
            manifest_sha256: control.graph_manifest_sha256.clone(),
            manifest_bytes: manifest_json.len(),
            node_count: control.manifest.nodes.len(),
            edge_count: control.manifest.edges.len(),
            wave_count: control.manifest.waves.len(),
            created_at_ms: 72,
        },
        manifest: control.manifest.clone(),
        manifest_json,
    }
}

fn schedule(
    control: &crate::runtime_domain::GroupAgentGraphControlSnapshot,
) -> GroupAgentGraphExecutionScheduleInspection {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-execution-schedule-v1.json"
    )))
    .expect("schedule fixture");
    let json = fixture["canonical_execution_schedule_json"]
        .as_str()
        .expect("schedule JSON")
        .to_owned();
    let mut value = GroupAgentGraphExecutionSchedule::decode_exact(&json).expect("schedule");
    value.control_snapshot_sha256 = control.snapshot_sha256.clone();
    value.expected_last_event_sha256 = control.last_event_sha256.clone();
    let digest = value.expected_sha256().expect("schedule digest");
    value.schedule_id = format!("graph-execution-schedule-{digest}");
    value.schedule_sha256 = digest;
    let json = value.canonical_json().expect("schedule JSON");
    GroupAgentGraphExecutionScheduleInspection {
        v: value.v,
        record: schedule_record(&value, json.len()),
        schedule_json: json,
        schedule: value,
    }
}

fn current_control(
    run: &GroupAgentGraphRunInspection,
) -> crate::runtime_domain::GroupAgentGraphControlSnapshot {
    let mut control = super::control_fixture();
    control.last_event_sha256 = run.events[0].expected_sha256().expect("event digest");
    control.snapshot_sha256 = control.expected_sha256().expect("control digest");
    control
}

fn scheduled_contract(
    control: &crate::runtime_domain::GroupAgentGraphControlSnapshot,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
    pricing_snapshot_sha256: &str,
    max_cost_usd_micros: u64,
) -> GroupAgentScheduledNodeContractInspection {
    let mut candidate = source().candidate;
    candidate.control_snapshot_sha256 = control.snapshot_sha256.clone();
    candidate.schedule_id = schedule.record.schedule_id.clone();
    candidate.schedule_sha256 = schedule.record.schedule_sha256.clone();
    candidate.expected_last_event_sha256 = control.last_event_sha256.clone();
    candidate.budgets.pricing_snapshot_sha256 = pricing_snapshot_sha256.into();
    candidate.budgets.max_cost_usd_micros = max_cost_usd_micros;
    candidate.request.schedule_id = candidate.schedule_id.clone();
    candidate.request.schedule_sha256 = candidate.schedule_sha256.clone();
    let request_sha256 = candidate.request.expected_sha256().expect("request digest");
    candidate.request.request_id = format!("scheduled-node-request-{request_sha256}");
    candidate.request.request_sha256 = request_sha256;
    let contract_sha256 = candidate.expected_sha256().expect("contract digest");
    candidate.contract_id = format!("scheduled-node-contract-{contract_sha256}");
    candidate.contract_sha256 = contract_sha256;
    let candidate_json = candidate.canonical_json().expect("candidate JSON");
    GroupAgentScheduledNodeContractInspection {
        v: candidate.v,
        record: contract_record(&candidate, candidate_json.len()),
        candidate_json,
        candidate,
    }
}

fn schedule_record(
    value: &GroupAgentGraphExecutionSchedule,
    bytes: usize,
) -> GroupAgentGraphExecutionScheduleRecord {
    GroupAgentGraphExecutionScheduleRecord {
        v: value.v,
        schedule_id: value.schedule_id.clone(),
        graph_run_id: value.graph_run_id.clone(),
        graph_id: value.graph_id.clone(),
        control_snapshot_sha256: value.control_snapshot_sha256.clone(),
        schedule_sha256: value.schedule_sha256.clone(),
        schedule_bytes: bytes,
        node_count: value.node_count,
        wave_count: value.wave_count,
        expected_last_event_seq: value.expected_last_event_seq,
        expected_last_event_sha256: value.expected_last_event_sha256.clone(),
        execution_contract_present: false,
        dispatch_authority_released: false,
        created_at_ms: 40,
    }
}

fn sorted_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(&sort(serde_json::to_value(value).expect("serialize")))
        .expect("sorted JSON")
}

fn sort(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort).collect()),
        Value::Object(items) => Value::Object(
            items
                .into_iter()
                .map(|(key, value)| (key, sort(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        other => other,
    }
}

fn exact<T: Clone>(provided: &str, expected: &str, value: &T) -> Result<T, HubStoreError> {
    (provided == expected)
        .then(|| value.clone())
        .ok_or_else(|| not_found(HubEntity::GroupAgentScheduledNodeProviderRequest, provided))
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentScheduledNodeProviderRequest,
        message: message.into(),
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
