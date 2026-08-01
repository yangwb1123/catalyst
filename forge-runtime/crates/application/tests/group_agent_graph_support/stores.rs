use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphInspection, GroupAgentGraphRecord,
    GroupAgentGraphStatus, GroupAgentGraphStore, GroupRunRecord, GroupRunSnapshot, GroupRunStore,
    HubEntity, HubStoreError, PrepareGroupAgentGraph, PrepareGroupAgentGraphDisposition,
    PrepareGroupAgentGraphResult, PrepareGroupRun, PrepareGroupRunResult,
};

struct RunState {
    snapshot: GroupRunSnapshot,
}

pub(crate) struct MemoryRunStore {
    state: Mutex<RunState>,
}

impl MemoryRunStore {
    pub(crate) fn new(snapshot: GroupRunSnapshot) -> Self {
        Self {
            state: Mutex::new(RunState { snapshot }),
        }
    }

    pub(crate) fn snapshot(&self) -> GroupRunSnapshot {
        self.lock().snapshot.clone()
    }

    pub(crate) fn respond_with(&self, snapshot: GroupRunSnapshot) {
        self.lock().snapshot = snapshot;
    }

    fn lock(&self) -> MutexGuard<'_, RunState> {
        self.state.lock().expect("run state")
    }
}

impl GroupRunStore for MemoryRunStore {
    fn prepare_group_run(
        &self,
        _request: &PrepareGroupRun,
    ) -> Result<PrepareGroupRunResult, HubStoreError> {
        Err(unavailable("run preparation is unused"))
    }

    fn inspect_group_run(&self, run_id: &str) -> Result<GroupRunSnapshot, HubStoreError> {
        let snapshot = self.snapshot();
        if snapshot.run.run_id == run_id {
            Ok(snapshot)
        } else {
            Err(not_found(HubEntity::GroupRun, run_id))
        }
    }

    fn list_group_runs(
        &self,
        group_id: Option<&str>,
        _limit: usize,
    ) -> Result<Vec<GroupRunRecord>, HubStoreError> {
        let snapshot = self.snapshot();
        Ok(group_id
            .is_none_or(|id| id == snapshot.run.group_id)
            .then_some(snapshot.run)
            .into_iter()
            .collect())
    }
}

#[derive(Default)]
struct GraphState {
    graphs: BTreeMap<String, GroupAgentGraphInspection>,
    keys: BTreeMap<String, String>,
    last_request: Option<PrepareGroupAgentGraph>,
    list_override: Option<Vec<GroupAgentGraphRecord>>,
    corrupt_next_prepare: bool,
}

#[derive(Default)]
/// Application-layer fake only. It does not model the production store's
/// same-transaction source/member revalidation; `SQLite` contract tests cover it.
pub(crate) struct MemoryGraphStore {
    state: Mutex<GraphState>,
}

impl MemoryGraphStore {
    pub(crate) fn last_request(&self) -> Option<PrepareGroupAgentGraph> {
        self.lock().last_request.clone()
    }

    pub(crate) fn corrupt_next_prepare(&self) {
        self.lock().corrupt_next_prepare = true;
    }

    pub(crate) fn inspection(&self, id: &str) -> GroupAgentGraphInspection {
        self.lock().graphs.get(id).cloned().expect("stored graph")
    }

    pub(crate) fn replace(&self, id: &str, inspection: GroupAgentGraphInspection) {
        self.lock().graphs.insert(id.into(), inspection);
    }

    pub(crate) fn set_list(&self, records: Vec<GroupAgentGraphRecord>) {
        self.lock().list_override = Some(records);
    }

    fn lock(&self) -> MutexGuard<'_, GraphState> {
        self.state.lock().expect("graph state")
    }
}

impl GroupAgentGraphStore for MemoryGraphStore {
    fn prepare_group_agent_graph(
        &self,
        request: &PrepareGroupAgentGraph,
    ) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
        let mut state = self.lock();
        state.last_request = Some(request.clone());
        if let Some(graph_id) = state.keys.get(&request.idempotency_key) {
            return replay(&state, graph_id, request);
        }
        if state.graphs.contains_key(&request.graph_id) {
            return Err(conflict("graph identifier is already used"));
        }
        let mut inspection = inspection(request);
        if state.corrupt_next_prepare {
            inspection.graph.manifest_sha256 = "0".repeat(64);
            state.corrupt_next_prepare = false;
        }
        state
            .keys
            .insert(request.idempotency_key.clone(), request.graph_id.clone());
        state
            .graphs
            .insert(request.graph_id.clone(), inspection.clone());
        Ok(result(
            PrepareGroupAgentGraphDisposition::Created,
            inspection,
        ))
    }

    fn inspect_group_agent_graph(
        &self,
        graph_id: &str,
    ) -> Result<GroupAgentGraphInspection, HubStoreError> {
        self.lock()
            .graphs
            .get(graph_id)
            .cloned()
            .ok_or_else(|| not_found(HubEntity::GroupAgentGraph, graph_id))
    }

    fn list_group_agent_graphs(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRecord>, HubStoreError> {
        let state = self.lock();
        if let Some(records) = &state.list_override {
            return Ok(records.clone());
        }
        Ok(state
            .graphs
            .values()
            .filter(|item| group_run_id.is_none_or(|id| item.graph.group_run_id == id))
            .take(limit)
            .map(|item| item.graph.clone())
            .collect())
    }
}

fn replay(
    state: &GraphState,
    graph_id: &str,
    request: &PrepareGroupAgentGraph,
) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
    let stored = state.graphs.get(graph_id).expect("key graph");
    if stored.manifest != request.manifest
        || stored.manifest_json != request.manifest_json
        || stored.graph.manifest_sha256 != request.manifest_sha256
    {
        return Err(conflict("idempotency key has different graph semantics"));
    }
    Ok(result(
        PrepareGroupAgentGraphDisposition::Replayed,
        stored.clone(),
    ))
}

fn inspection(request: &PrepareGroupAgentGraph) -> GroupAgentGraphInspection {
    GroupAgentGraphInspection {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph: GroupAgentGraphRecord {
            v: GROUP_AGENT_GRAPH_VERSION,
            graph_id: request.graph_id.clone(),
            group_run_id: request.manifest.source.group_run_id.clone(),
            status: GroupAgentGraphStatus::Prepared,
            source_snapshot_sha256: request.manifest.source.snapshot_sha256.clone(),
            manifest_sha256: request.manifest_sha256.clone(),
            manifest_bytes: request.manifest_json.len(),
            node_count: request.manifest.nodes.len(),
            edge_count: request.manifest.edges.len(),
            wave_count: request.manifest.waves.len(),
            created_at_ms: request.created_at_ms,
        },
        manifest: request.manifest.clone(),
        manifest_json: request.manifest_json.clone(),
    }
}

fn result(
    disposition: PrepareGroupAgentGraphDisposition,
    inspection: GroupAgentGraphInspection,
) -> PrepareGroupAgentGraphResult {
    PrepareGroupAgentGraphResult {
        v: GROUP_AGENT_GRAPH_VERSION,
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
        entity: HubEntity::GroupAgentGraph,
        message: message.into(),
    }
}

fn unavailable(message: &str) -> HubStoreError {
    HubStoreError::Unavailable {
        message: message.into(),
    }
}
