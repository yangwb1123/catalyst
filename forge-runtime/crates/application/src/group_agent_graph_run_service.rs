use std::sync::Arc;

pub(crate) use crate::runtime_domain::{
    BeginGroupAgentGraphRun, GroupAgentGraphInspection, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
};
pub use crate::runtime_domain::{
    BeginGroupAgentGraphRunDisposition, BeginGroupAgentGraphRunResult,
    GROUP_AGENT_GRAPH_CORE_PLAN_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
    GROUP_AGENT_GRAPH_RUN_EVENT_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_RUN_VERSION,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentGraphCorePlan, GroupAgentGraphEdge,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, GroupAgentGraphRunStatus, MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES,
    MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES, MAX_GROUP_AGENT_GRAPH_RUN_LIST_LIMIT,
};
use crate::runtime_domain::{GroupAgentGraphRunStore, GroupAgentGraphStore, HubStoreError};
use thiserror::Error;

use crate::group_agent_graph_run_validation::{
    begin_request, checked_graph, checked_run, validate_list, validate_list_input,
    validate_prepare_input, validate_prepare_result, validate_run_graph,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentGraphRunInput {
    pub graph_run_id: String,
    pub graph_id: String,
    pub plan_json: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Error)]
pub enum GroupAgentGraphRunServiceError {
    #[error("Group Agent Graph Run input is invalid")]
    InvalidInput,
    #[error("Group Agent Graph Core Plan is invalid")]
    InvalidPlan,
    #[error("Group Agent Graph Run source is invalid")]
    InvalidGraph,
    #[error("Group Agent Graph Run store returned inconsistent state")]
    InconsistentStoreResult,
    #[error("Group Agent Graph Run store failed: {0}")]
    Store(#[from] HubStoreError),
}

pub struct GroupAgentGraphRunService {
    graphs: Arc<dyn GroupAgentGraphStore>,
    runs: Arc<dyn GroupAgentGraphRunStore>,
}

impl GroupAgentGraphRunService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
    ) -> Self {
        Self { graphs, runs }
    }

    /// Prepares one passive Graph Run without releasing execution authority.
    ///
    /// # Errors
    ///
    /// Returns strict input, plan, source, consistency, or storage errors.
    pub fn prepare(
        &self,
        input: &PrepareGroupAgentGraphRunInput,
    ) -> Result<BeginGroupAgentGraphRunResult, GroupAgentGraphRunServiceError> {
        let plan = validate_prepare_input(input)?;
        let graph = checked_graph(self.graphs.inspect_group_agent_graph(&input.graph_id)?)?;
        validate_run_graph(&plan, &graph)?;
        let request = begin_request(input, plan, &graph)?;
        let result = self.runs.begin_group_agent_graph_run(&request)?;
        validate_prepare_result(&request, result)
    }

    /// Loads and revalidates one passive Graph Run and its exact source Graph.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, corruption, inconsistency, or storage.
    pub fn inspect(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, GroupAgentGraphRunServiceError> {
        crate::group_agent_graph_validation::validate_identifier(graph_run_id)
            .map_err(|_| GroupAgentGraphRunServiceError::InvalidInput)?;
        let inspection = checked_run(self.runs.inspect_group_agent_graph_run(graph_run_id)?)?;
        if inspection.run.graph_run_id != graph_run_id {
            return Err(GroupAgentGraphRunServiceError::InconsistentStoreResult);
        }
        let graph = checked_graph(
            self.graphs
                .inspect_group_agent_graph(&inspection.run.graph_id)?,
        )?;
        validate_run_graph(&inspection.plan, &graph)?;
        validate_run_binding(&inspection, &graph)?;
        Ok(inspection)
    }

    /// Lists bounded, metadata-only passive Graph Runs.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid filters, inconsistent metadata, or storage.
    pub fn list(
        &self,
        graph_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, GroupAgentGraphRunServiceError> {
        validate_list_input(graph_id, limit)?;
        let records = self.runs.list_group_agent_graph_runs(graph_id, limit)?;
        validate_list(&records, graph_id, limit)?;
        Ok(records)
    }
}

fn validate_run_binding(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<(), GroupAgentGraphRunServiceError> {
    let valid = run.run.graph_id == graph.graph.graph_id
        && run.run.source_snapshot_sha256 == graph.graph.source_snapshot_sha256
        && run.run.graph_manifest_sha256 == graph.graph.manifest_sha256;
    valid
        .then_some(())
        .ok_or(GroupAgentGraphRunServiceError::InconsistentStoreResult)
}
