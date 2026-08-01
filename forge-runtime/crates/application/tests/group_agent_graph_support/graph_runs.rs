use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

use forge_runtime_domain::{
    BeginGroupAgentGraphRun, BeginGroupAgentGraphRunDisposition, BeginGroupAgentGraphRunResult,
    GROUP_AGENT_GRAPH_RUN_VERSION, GroupAgentGraphRunInspection, GroupAgentGraphRunRecord,
    GroupAgentGraphRunStatus, GroupAgentGraphRunStore, HubEntity, HubStoreError,
};

#[derive(Default)]
struct GraphRunState {
    runs: BTreeMap<String, GroupAgentGraphRunInspection>,
    keys: BTreeMap<String, String>,
    last_request: Option<BeginGroupAgentGraphRun>,
    list_override: Option<Vec<GroupAgentGraphRunRecord>>,
    corrupt_next_prepare: bool,
}

#[derive(Default)]
pub(crate) struct MemoryGraphRunStore {
    state: Mutex<GraphRunState>,
}

impl MemoryGraphRunStore {
    pub(crate) fn last_request(&self) -> Option<BeginGroupAgentGraphRun> {
        self.lock().last_request.clone()
    }

    pub(crate) fn inspection(&self, id: &str) -> GroupAgentGraphRunInspection {
        self.lock().runs.get(id).cloned().expect("stored Graph Run")
    }

    pub(crate) fn replace(&self, id: &str, inspection: GroupAgentGraphRunInspection) {
        self.lock().runs.insert(id.into(), inspection);
    }

    pub(crate) fn set_list(&self, records: Vec<GroupAgentGraphRunRecord>) {
        self.lock().list_override = Some(records);
    }

    pub(crate) fn corrupt_next_prepare(&self) {
        self.lock().corrupt_next_prepare = true;
    }

    fn lock(&self) -> MutexGuard<'_, GraphRunState> {
        self.state.lock().expect("Graph Run state")
    }
}

impl GroupAgentGraphRunStore for MemoryGraphRunStore {
    fn begin_group_agent_graph_run(
        &self,
        request: &BeginGroupAgentGraphRun,
    ) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
        let mut state = self.lock();
        state.last_request = Some(request.clone());
        if let Some(run_id) = state.keys.get(&request.idempotency_key).cloned() {
            return replay(&state, &run_id, request);
        }
        if state.runs.contains_key(&request.graph_run_id) {
            return Err(conflict("Graph Run identifier is already used"));
        }
        let inspection = inspection(request);
        state.keys.insert(
            request.idempotency_key.clone(),
            request.graph_run_id.clone(),
        );
        state
            .runs
            .insert(request.graph_run_id.clone(), inspection.clone());
        let returned = corrupt_if_requested(&mut state, inspection);
        Ok(result(
            BeginGroupAgentGraphRunDisposition::Created,
            returned,
        ))
    }

    fn inspect_group_agent_graph_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
        self.lock()
            .runs
            .get(graph_run_id)
            .cloned()
            .ok_or_else(|| not_found(graph_run_id))
    }

    fn list_group_agent_graph_runs(
        &self,
        graph_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError> {
        let state = self.lock();
        if let Some(records) = &state.list_override {
            return Ok(records.clone());
        }
        Ok(state
            .runs
            .values()
            .filter(|item| graph_id.is_none_or(|id| id == item.run.graph_id))
            .take(limit)
            .map(|item| item.run.clone())
            .collect())
    }
}

fn replay(
    state: &GraphRunState,
    run_id: &str,
    request: &BeginGroupAgentGraphRun,
) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
    let stored = state.runs.get(run_id).expect("key-bound Graph Run");
    if !same_semantics(stored, request) {
        return Err(conflict(
            "idempotency key has different Graph Run semantics",
        ));
    }
    Ok(result(
        BeginGroupAgentGraphRunDisposition::Replayed,
        stored.clone(),
    ))
}

fn same_semantics(
    stored: &GroupAgentGraphRunInspection,
    request: &BeginGroupAgentGraphRun,
) -> bool {
    stored.run.graph_id == request.graph_id
        && stored.run.source_snapshot_sha256 == request.source_snapshot_sha256
        && stored.run.graph_manifest_sha256 == request.graph_manifest_sha256
        && stored.plan == request.plan
        && stored.plan_json == request.plan_json
}

fn corrupt_if_requested(
    state: &mut GraphRunState,
    mut inspection: GroupAgentGraphRunInspection,
) -> GroupAgentGraphRunInspection {
    if state.corrupt_next_prepare {
        inspection.run.plan_sha256 = "0".repeat(64);
        state.corrupt_next_prepare = false;
    }
    inspection
}

fn inspection(request: &BeginGroupAgentGraphRun) -> GroupAgentGraphRunInspection {
    GroupAgentGraphRunInspection {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        run: record(request),
        plan_json: request.plan_json.clone(),
        plan: request.plan.clone(),
        event_jsons: vec![request.event_json.clone()],
        events: vec![request.event.clone()],
    }
}

fn record(request: &BeginGroupAgentGraphRun) -> GroupAgentGraphRunRecord {
    GroupAgentGraphRunRecord {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: request.graph_run_id.clone(),
        graph_id: request.graph_id.clone(),
        status: GroupAgentGraphRunStatus::AwaitingExecutionContract,
        source_snapshot_sha256: request.source_snapshot_sha256.clone(),
        graph_manifest_sha256: request.graph_manifest_sha256.clone(),
        scheduler_protocol_version: request.plan.scheduler_protocol_version,
        plan_sha256: request.plan.plan_sha256.clone(),
        plan_bytes: request.plan_json.len(),
        node_count: request.plan.authored_node_ids.len(),
        wave_count: request.plan.waves.len(),
        execution_contract_present: false,
        dispatch_request_present: false,
        dispatch_authority_released: false,
        last_event_seq: 1,
        journal_bytes: request.event_json.len(),
        created_at_ms: request.created_at_ms,
    }
}

fn result(
    disposition: BeginGroupAgentGraphRunDisposition,
    inspection: GroupAgentGraphRunInspection,
) -> BeginGroupAgentGraphRunResult {
    BeginGroupAgentGraphRunResult {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        disposition,
        inspection,
    }
}

fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupAgentGraphRun,
        id: id.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraphRun,
        message: message.into(),
    }
}
