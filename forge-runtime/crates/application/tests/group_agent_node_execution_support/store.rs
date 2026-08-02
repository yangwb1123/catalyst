use std::sync::{
    Mutex, MutexGuard,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_domain::{
    AdmitGroupAgentNodeExecutionContract, AdmitGroupAgentNodeExecutionContractDisposition,
    AdmitGroupAgentNodeExecutionContractResult, BeginGroupAgentGraphRun,
    BeginGroupAgentGraphRunResult, GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION,
    GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION, GroupAgentGraphInspection, GroupAgentGraphRecord,
    GroupAgentGraphRunInspection, GroupAgentGraphRunRecord, GroupAgentGraphRunStatus,
    GroupAgentGraphRunStore, GroupAgentGraphStore, GroupAgentNodeExecutionContractInspection,
    GroupAgentNodeExecutionContractRecord, GroupAgentNodeExecutionContractStore,
    GroupAgentNodeLifecycleInspection, HubEntity, HubStoreError, PrepareGroupAgentGraph,
    PrepareGroupAgentGraphResult,
};

use super::FixtureBundle;

#[path = "store/dispatch.rs"]
mod dispatch;
#[path = "store/lifecycle.rs"]
mod lifecycle;

pub(super) struct State {
    graph: GroupAgentGraphInspection,
    run: GroupAgentGraphRunInspection,
    admission: Option<StoredAdmission>,
    dispatch: Option<dispatch::StoredDispatch>,
    lifecycle: Option<GroupAgentNodeLifecycleInspection>,
    list_override: Option<Vec<GroupAgentNodeExecutionContractRecord>>,
}

#[derive(Clone)]
pub(super) struct StoredAdmission {
    key: String,
    inspection: GroupAgentNodeExecutionContractInspection,
}

pub(crate) struct MemoryContractHub {
    state: Mutex<State>,
    run_reads: AtomicUsize,
}

impl MemoryContractHub {
    pub(crate) fn new(fixture: &FixtureBundle) -> Self {
        Self {
            state: Mutex::new(State {
                graph: fixture.graph.clone(),
                run: fixture.run.clone(),
                admission: None,
                dispatch: None,
                lifecycle: None,
                list_override: None,
            }),
            run_reads: AtomicUsize::new(0),
        }
    }

    pub(crate) fn run_reads(&self) -> usize {
        self.run_reads.load(Ordering::Relaxed)
    }

    pub(crate) fn set_list(&self, records: Vec<GroupAgentNodeExecutionContractRecord>) {
        self.lock().list_override = Some(records);
    }

    pub(crate) fn set_inspection(&self, inspection: GroupAgentNodeExecutionContractInspection) {
        let mut state = self.lock();
        state.run = inspection.graph_run.clone();
        state
            .admission
            .as_mut()
            .expect("stored admission")
            .inspection = inspection;
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().expect("memory contract hub")
    }
}

impl GroupAgentGraphStore for MemoryContractHub {
    fn prepare_group_agent_graph(
        &self,
        _request: &PrepareGroupAgentGraph,
    ) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
        Err(unavailable("preparation is outside this test store"))
    }

    fn inspect_group_agent_graph(
        &self,
        graph_id: &str,
    ) -> Result<GroupAgentGraphInspection, HubStoreError> {
        let state = self.lock();
        (state.graph.graph.graph_id == graph_id)
            .then(|| state.graph.clone())
            .ok_or_else(|| not_found(HubEntity::GroupAgentGraph, graph_id))
    }

    fn list_group_agent_graphs(
        &self,
        _group_run_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentGraphRecord>, HubStoreError> {
        Ok(vec![self.lock().graph.graph.clone()])
    }
}

impl GroupAgentGraphRunStore for MemoryContractHub {
    fn begin_group_agent_graph_run(
        &self,
        _request: &BeginGroupAgentGraphRun,
    ) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
        Err(unavailable(
            "Graph Run preparation is outside this test store",
        ))
    }

    fn inspect_group_agent_graph_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
        self.run_reads.fetch_add(1, Ordering::Relaxed);
        let state = self.lock();
        (state.run.run.graph_run_id == graph_run_id)
            .then(|| state.run.clone())
            .ok_or_else(|| not_found(HubEntity::GroupAgentGraphRun, graph_run_id))
    }

    fn list_group_agent_graph_runs(
        &self,
        _graph_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError> {
        Ok(vec![self.lock().run.run.clone()])
    }
}

impl GroupAgentNodeExecutionContractStore for MemoryContractHub {
    fn admit_group_agent_node_execution_contract(
        &self,
        request: &AdmitGroupAgentNodeExecutionContract,
    ) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError> {
        request
            .validate()
            .map_err(|error| conflict(&error.to_string()))?;
        let mut state = self.lock();
        if let Some(stored) = &state.admission {
            return replay(stored, request);
        }
        let inspection = create_inspection(&state.run, request)?;
        state.run = inspection.graph_run.clone();
        state.admission = Some(StoredAdmission {
            key: request.idempotency_key.clone(),
            inspection: inspection.clone(),
        });
        Ok(admit_result(
            AdmitGroupAgentNodeExecutionContractDisposition::Created,
            inspection,
        ))
    }

    fn inspect_group_agent_node_execution_contract(
        &self,
        contract_id: &str,
    ) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
        let state = self.lock();
        state
            .admission
            .as_ref()
            .filter(|stored| stored.inspection.record.contract_id == contract_id)
            .map(|stored| stored.inspection.clone())
            .ok_or_else(|| not_found(HubEntity::GroupAgentNodeExecutionContract, contract_id))
    }

    fn list_group_agent_node_execution_contracts(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentNodeExecutionContractRecord>, HubStoreError> {
        let state = self.lock();
        if let Some(records) = &state.list_override {
            return Ok(records.clone());
        }
        Ok(state
            .admission
            .iter()
            .map(|stored| stored.inspection.record.clone())
            .filter(|record| graph_run_id.is_none_or(|id| id == record.graph_run_id))
            .take(limit)
            .collect())
    }
}

fn replay(
    stored: &StoredAdmission,
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError> {
    let exact = stored.key == request.idempotency_key
        && stored.inspection.contract == request.contract
        && stored.inspection.contract_json == request.contract_json;
    if !exact {
        return Err(conflict("replay semantics disagree"));
    }
    Ok(admit_result(
        AdmitGroupAgentNodeExecutionContractDisposition::Replayed,
        stored.inspection.clone(),
    ))
}

fn create_inspection(
    base: &GroupAgentGraphRunInspection,
    request: &AdmitGroupAgentNodeExecutionContract,
) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
    let mut graph_run = base.clone();
    graph_run.v = GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION;
    graph_run.run.v = GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION;
    graph_run.run.status = GroupAgentGraphRunStatus::AwaitingCoreDispatch;
    graph_run.run.execution_contract_present = true;
    graph_run.run.last_event_seq = 2;
    graph_run.run.journal_bytes += request.event_json.len();
    graph_run.events.push(request.event.clone());
    graph_run.event_jsons.push(request.event_json.clone());
    let inspection = GroupAgentNodeExecutionContractInspection {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        record: record(request),
        contract_json: request.contract_json.clone(),
        contract: request.contract.clone(),
        admission_event_json: request.event_json.clone(),
        admission_event: request.event.clone(),
        graph_run,
    };
    inspection
        .validate()
        .map_err(|error| corrupt(&error.to_string()))?;
    Ok(inspection)
}

fn record(request: &AdmitGroupAgentNodeExecutionContract) -> GroupAgentNodeExecutionContractRecord {
    GroupAgentNodeExecutionContractRecord {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        contract_id: request.contract.contract_id.clone(),
        graph_run_id: request.graph_run_id.clone(),
        node_id: request.contract.node.node_id.clone(),
        attempt: request.contract.node.attempt,
        control_snapshot_sha256: request.control_snapshot.snapshot_sha256.clone(),
        contract_sha256: request.contract.contract_sha256.clone(),
        contract_bytes: request.contract_json.len(),
        request_sha256: request.contract.request.request_sha256.clone(),
        project_lane_sha256: request.contract.node.project_lane_sha256.clone(),
        expected_last_event_seq: request.contract.expected_last_event_seq,
        expected_last_event_sha256: request.contract.expected_last_event_sha256.clone(),
        created_at_ms: request.admitted_at_ms,
    }
}

fn admit_result(
    disposition: AdmitGroupAgentNodeExecutionContractDisposition,
    inspection: GroupAgentNodeExecutionContractInspection,
) -> AdmitGroupAgentNodeExecutionContractResult {
    AdmitGroupAgentNodeExecutionContractResult {
        v: GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION,
        disposition,
        inspection,
    }
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentNodeExecutionContract,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
