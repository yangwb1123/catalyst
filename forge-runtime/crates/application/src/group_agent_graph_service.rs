use std::sync::Arc;

use forge_runtime_domain::{
    GroupAgentGraphEdge, GroupAgentGraphInspection, GroupAgentGraphManager, GroupAgentGraphNode,
    GroupAgentGraphRecord, GroupAgentGraphStore, GroupRunStore, HubStoreError,
    PrepareGroupAgentGraphResult,
};
use thiserror::Error;

use crate::group_agent_graph_validation::{
    canonical_request, checked_inspection, checked_source, prepare_with_store, validate_identifier,
    validate_input, validate_list, validate_list_input, validate_members,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentGraphInput {
    pub graph_id: String,
    pub group_run_id: String,
    pub manager: GroupAgentGraphManager,
    pub nodes: Vec<GroupAgentGraphNode>,
    pub edges: Vec<GroupAgentGraphEdge>,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Error)]
pub enum GroupAgentGraphServiceError {
    #[error("Group Agent Graph input is invalid")]
    InvalidInput,
    #[error("frozen Group Run failed Group Agent Graph validation")]
    InvalidSource,
    #[error("Group Agent Graph store returned inconsistent state")]
    InconsistentStoreResult,
    #[error("Group Agent Graph store failed: {0}")]
    Store(#[from] HubStoreError),
}

pub struct GroupAgentGraphService {
    group_runs: Arc<dyn GroupRunStore>,
    graphs: Arc<dyn GroupAgentGraphStore>,
}

impl GroupAgentGraphService {
    #[must_use]
    pub fn new(group_runs: Arc<dyn GroupRunStore>, graphs: Arc<dyn GroupAgentGraphStore>) -> Self {
        Self { group_runs, graphs }
    }

    /// Validates and freezes one local graph definition without executing any Agent.
    ///
    /// # Errors
    ///
    /// Returns validation, source-integrity, consistency, or storage errors.
    pub fn prepare(
        &self,
        input: &PrepareGroupAgentGraphInput,
    ) -> Result<PrepareGroupAgentGraphResult, GroupAgentGraphServiceError> {
        validate_input(input)?;
        let snapshot = self.group_runs.inspect_group_run(&input.group_run_id)?;
        let source = checked_source(&snapshot, &input.group_run_id)?;
        if !validate_members(&input.nodes, &snapshot) {
            return Err(GroupAgentGraphServiceError::InvalidInput);
        }
        let request = canonical_request(input, source)?;
        prepare_with_store(self.graphs.as_ref(), input, &request)
    }

    /// Loads a graph and revalidates its exact source, members, bytes, and digest.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, corruption, inconsistency, or storage failure.
    pub fn inspect(
        &self,
        graph_id: &str,
    ) -> Result<GroupAgentGraphInspection, GroupAgentGraphServiceError> {
        validate_identifier(graph_id)?;
        let inspection = checked_inspection(self.graphs.inspect_group_agent_graph(graph_id)?)?;
        if inspection.graph.graph_id != graph_id {
            return Err(GroupAgentGraphServiceError::InconsistentStoreResult);
        }
        self.revalidate_source(&inspection)?;
        Ok(inspection)
    }

    /// Lists bounded graph metadata without loading source or manifest bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid filters, inconsistent metadata, or storage failure.
    pub fn list(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRecord>, GroupAgentGraphServiceError> {
        validate_list_input(group_run_id, limit)?;
        let records = self.graphs.list_group_agent_graphs(group_run_id, limit)?;
        validate_list(&records, group_run_id, limit)?;
        Ok(records)
    }

    fn revalidate_source(
        &self,
        inspection: &GroupAgentGraphInspection,
    ) -> Result<(), GroupAgentGraphServiceError> {
        let run_id = &inspection.graph.group_run_id;
        let snapshot = self.group_runs.inspect_group_run(run_id)?;
        let source = checked_source(&snapshot, run_id)?;
        if source != inspection.manifest.source
            || !validate_members(&inspection.manifest.nodes, &snapshot)
        {
            return Err(GroupAgentGraphServiceError::InconsistentStoreResult);
        }
        Ok(())
    }
}
