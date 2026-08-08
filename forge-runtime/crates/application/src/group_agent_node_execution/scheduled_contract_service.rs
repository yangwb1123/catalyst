use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleStore,
    GroupAgentGraphRunInspection, GroupAgentGraphRunStore, GroupAgentGraphStore,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractRecord,
    GroupAgentScheduledNodeContractStore,
};

use super::{
    AdmitGroupAgentScheduledNodeContractResult, GroupAgentScheduledNodeContractServiceError,
    scheduled_contract_error::corrupt,
    scheduled_contract_validation::{
        admission_request, checked_graph, checked_inspection, checked_run, checked_schedule,
        parse_admit_input, validate_admit_result, validate_identifier, validate_initial_scope,
        validate_list, validate_list_input, validate_sources,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentScheduledNodeContractInput {
    pub graph_run_id: String,
    pub contract_json: String,
    pub idempotency_key: String,
    pub admitted_at_ms: u64,
}

pub struct GroupAgentScheduledNodeContractService {
    graphs: Arc<dyn GroupAgentGraphStore>,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
    candidates: Arc<dyn GroupAgentScheduledNodeContractStore>,
}

impl GroupAgentScheduledNodeContractService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        schedules: Arc<dyn GroupAgentGraphExecutionScheduleStore>,
        candidates: Arc<dyn GroupAgentScheduledNodeContractStore>,
    ) -> Self {
        Self {
            graphs,
            runs,
            schedules,
            candidates,
        }
    }

    /// Validates every pure admission input before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid identifier, key,
    /// candidate envelope, byte bound, time, or Graph Run binding.
    pub fn preflight_admit(
        input: &AdmitGroupAgentScheduledNodeContractInput,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        validate_initial_scope(&parse_admit_input(input)?)
    }

    /// Validates an inspection identifier before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid candidate identifier.
    pub fn preflight_inspect(
        contract_id: &str,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        validate_identifier(contract_id, "scheduled contract ID")
    }

    /// Validates list filters and bounds before a caller opens storage.
    ///
    /// # Errors
    ///
    /// Returns a structured input error for an invalid Run filter or limit.
    pub fn preflight_list(
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<(), GroupAgentScheduledNodeContractServiceError> {
        validate_list_input(graph_run_id, limit)
    }

    /// Admits one passive schedule-bound initial-node contract candidate.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, or storage error.
    pub fn admit(
        &self,
        input: &AdmitGroupAgentScheduledNodeContractInput,
    ) -> Result<
        AdmitGroupAgentScheduledNodeContractResult,
        GroupAgentScheduledNodeContractServiceError,
    > {
        let candidate = parse_admit_input(input)?;
        validate_initial_scope(&candidate)?;
        let run = self.load_run(&input.graph_run_id)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        let control = super::snapshot::export(&run, &graph)?;
        let schedule = self.load_schedule(&candidate.schedule_id)?;
        let request = admission_request(input, candidate, control, &schedule)?;
        let result = self
            .candidates
            .admit_group_agent_scheduled_node_contract(&request)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        validate_admit_result(&request, result)
    }

    /// Loads and fully revalidates one candidate against its source and schedule.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, or storage error.
    pub fn inspect(
        &self,
        contract_id: &str,
    ) -> Result<
        GroupAgentScheduledNodeContractInspection,
        GroupAgentScheduledNodeContractServiceError,
    > {
        Self::preflight_inspect(contract_id)?;
        let inspection = self
            .candidates
            .inspect_group_agent_scheduled_node_contract(contract_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        let inspection = checked_inspection(inspection)?;
        validate_initial_scope(&inspection.candidate)?;
        if inspection.record.contract_id != contract_id {
            return Err(corrupt(
                "store returned a different scheduled contract identity",
            ));
        }
        let run = self.load_run(&inspection.record.graph_run_id)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        let control = super::snapshot::historical_base(&run, &graph)?;
        let schedule = self.load_schedule(&inspection.record.schedule_id)?;
        validate_sources(&inspection.candidate, &control, &schedule)?;
        Ok(inspection)
    }

    /// Lists bounded candidate metadata without Prompt or contract plaintext.
    ///
    /// # Errors
    ///
    /// Returns a structured input, corruption, or storage error.
    pub fn list(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<
        Vec<GroupAgentScheduledNodeContractRecord>,
        GroupAgentScheduledNodeContractServiceError,
    > {
        Self::preflight_list(graph_run_id, limit)?;
        let records = self
            .candidates
            .list_group_agent_scheduled_node_contracts(graph_run_id, limit)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        validate_list(&records, graph_run_id, limit)?;
        Ok(records)
    }

    fn load_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<
        GroupAgentGraphExecutionScheduleInspection,
        GroupAgentScheduledNodeContractServiceError,
    > {
        let inspection = self
            .schedules
            .inspect_group_agent_graph_execution_schedule(schedule_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        let inspection = checked_schedule(inspection)?;
        if inspection.record.schedule_id != schedule_id {
            return Err(corrupt(
                "store returned a different execution schedule identity",
            ));
        }
        Ok(inspection)
    }

    fn load_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, GroupAgentScheduledNodeContractServiceError> {
        let run = self
            .runs
            .inspect_group_agent_graph_run(graph_run_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        let run = checked_run(run)?;
        if run.run.graph_run_id != graph_run_id {
            return Err(corrupt("store returned a different Graph Run identity"));
        }
        Ok(run)
    }

    fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentGraphInspection,
        GroupAgentScheduledNodeContractServiceError,
    > {
        let graph = self
            .graphs
            .inspect_group_agent_graph(graph_id)
            .map_err(GroupAgentScheduledNodeContractServiceError::from)?;
        checked_graph(graph)
    }
}
