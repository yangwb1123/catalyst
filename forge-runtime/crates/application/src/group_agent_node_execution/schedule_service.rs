use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleRecord,
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunInspection, GroupAgentGraphRunStore,
    GroupAgentGraphStore,
};

use super::{
    AdmitGroupAgentGraphExecutionScheduleResult, GroupAgentGraphExecutionScheduleServiceError,
    schedule_error::corrupt,
    schedule_validation::{
        admission_request, checked_graph, checked_inspection, checked_run, parse_admit_input,
        validate_admit_result, validate_identifier, validate_inspection_source, validate_list,
        validate_list_input,
    },
};

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentGraphExecutionScheduleInput {
    pub graph_run_id: String,
    pub schedule_json: String,
    pub idempotency_key: String,
    pub admitted_at_ms: u64,
}

pub struct GroupAgentGraphExecutionScheduleService {
    graphs: Arc<dyn GroupAgentGraphStore>,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
}

impl GroupAgentGraphExecutionScheduleService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
    ) -> Self {
        Self {
            graphs,
            runs,
            schedules,
        }
    }

    /// Admits one immutable, effect-free schedule sidecar without changing the Graph Run.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, or storage error.
    pub fn admit(
        &self,
        input: &AdmitGroupAgentGraphExecutionScheduleInput,
    ) -> Result<
        AdmitGroupAgentGraphExecutionScheduleResult,
        GroupAgentGraphExecutionScheduleServiceError,
    > {
        let schedule = parse_admit_input(input)?;
        let run_before = self.load_run(&input.graph_run_id)?;
        let graph_before = self.load_graph(&run_before.run.graph_id)?;
        let control = super::snapshot::export(&run_before, &graph_before)?;
        let request = admission_request(input, schedule, control)?;
        let result = self
            .schedules
            .admit_group_agent_graph_execution_schedule(&request)
            .map_err(GroupAgentGraphExecutionScheduleServiceError::from)?;
        let result = validate_admit_result(&request, result)?;
        // The store revalidates and commits against one atomic source snapshot.
        // A post-commit reread would race a legitimate contract admission and
        // could turn a durable success into a false corruption error.
        validate_inspection_source(&result.inspection, &run_before, &graph_before)?;
        Ok(result)
    }

    /// Loads and fully revalidates one schedule against its current exact v1 source.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, or storage error.
    pub fn inspect(
        &self,
        schedule_id: &str,
    ) -> Result<
        GroupAgentGraphExecutionScheduleInspection,
        GroupAgentGraphExecutionScheduleServiceError,
    > {
        validate_identifier(schedule_id, "schedule ID")?;
        let inspection = self
            .schedules
            .inspect_group_agent_graph_execution_schedule(schedule_id)
            .map_err(GroupAgentGraphExecutionScheduleServiceError::from)?;
        let inspection = checked_inspection(inspection)?;
        if inspection.record.schedule_id != schedule_id {
            return Err(corrupt("store returned a different schedule identity"));
        }
        let run = self.load_run(&inspection.record.graph_run_id)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        validate_inspection_source(&inspection, &run, &graph)?;
        Ok(inspection)
    }

    /// Lists bounded, content-free schedule metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured input, corruption, or storage error.
    pub fn list(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<
        Vec<GroupAgentGraphExecutionScheduleRecord>,
        GroupAgentGraphExecutionScheduleServiceError,
    > {
        validate_list_input(graph_run_id, limit)?;
        let records = self
            .schedules
            .list_group_agent_graph_execution_schedules(graph_run_id, limit)
            .map_err(GroupAgentGraphExecutionScheduleServiceError::from)?;
        validate_list(&records, graph_run_id, limit)?;
        Ok(records)
    }

    fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentGraphInspection,
        GroupAgentGraphExecutionScheduleServiceError,
    > {
        let graph = self
            .graphs
            .inspect_group_agent_graph(graph_id)
            .map_err(GroupAgentGraphExecutionScheduleServiceError::from)?;
        checked_graph(graph)
    }

    fn load_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, GroupAgentGraphExecutionScheduleServiceError> {
        let run = self
            .runs
            .inspect_group_agent_graph_run(graph_run_id)
            .map_err(GroupAgentGraphExecutionScheduleServiceError::from)?;
        let run = checked_run(run)?;
        if run.run.graph_run_id != graph_run_id {
            return Err(corrupt("store returned a different Graph Run identity"));
        }
        Ok(run)
    }
}
