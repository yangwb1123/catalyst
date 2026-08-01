use std::sync::Arc;

use crate::runtime_domain::{
    GroupAgentGraphRunStore, GroupAgentGraphStore, GroupAgentNodeExecutionContractStore,
};

use super::{
    AdmitGroupAgentNodeExecutionContractResult, GroupAgentNodeExecutionContractInspection,
    GroupAgentNodeExecutionContractRecord, GroupAgentNodeExecutionContractServiceError,
    validation::{
        admission_request, checked_graph, checked_inspection, checked_run, parse_admit_input,
        validate_admit_result, validate_identifier, validate_inspection_source_and_control,
        validate_list, validate_list_input,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentNodeExecutionContractInput {
    pub graph_run_id: String,
    pub contract_json: String,
    pub idempotency_key: String,
    pub admitted_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportGroupAgentGraphControl {
    pub snapshot: super::GroupAgentGraphControlSnapshot,
    pub snapshot_json: String,
}

pub struct GroupAgentNodeExecutionContractService {
    graphs: Arc<dyn GroupAgentGraphStore>,
    runs: Arc<dyn GroupAgentGraphRunStore>,
    contracts: Arc<dyn GroupAgentNodeExecutionContractStore>,
}

impl GroupAgentNodeExecutionContractService {
    #[must_use]
    pub fn new(
        graphs: Arc<dyn GroupAgentGraphStore>,
        runs: Arc<dyn GroupAgentGraphRunStore>,
        contracts: Arc<dyn GroupAgentNodeExecutionContractStore>,
    ) -> Self {
        Self {
            graphs,
            runs,
            contracts,
        }
    }

    /// Exports exact private control bytes only from the complete v1 base state.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, or storage error.
    pub fn export_control(
        &self,
        graph_run_id: &str,
    ) -> Result<ExportGroupAgentGraphControl, GroupAgentNodeExecutionContractServiceError> {
        validate_identifier(graph_run_id, "Graph Run ID")?;
        let run = self.load_run(graph_run_id)?;
        super::snapshot::require_base_run(&run)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        super::snapshot::export(&run, &graph)
    }

    /// Admits one exact first-node contract without releasing dispatch authority.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, conflict, corruption, or storage error.
    pub fn admit(
        &self,
        input: &AdmitGroupAgentNodeExecutionContractInput,
    ) -> Result<
        AdmitGroupAgentNodeExecutionContractResult,
        GroupAgentNodeExecutionContractServiceError,
    > {
        let contract = parse_admit_input(input)?;
        let run = self.load_run(&input.graph_run_id)?;
        let graph = self.load_graph(&run.run.graph_id)?;
        let control = super::snapshot::for_admission(&run, &graph)?;
        let request = admission_request(input, contract, control)?;
        let result = self
            .contracts
            .admit_group_agent_node_execution_contract(&request)
            .map_err(GroupAgentNodeExecutionContractServiceError::from)?;
        let result = validate_admit_result(&request, result)?;
        let graph = self.load_graph(&result.inspection.graph_run.run.graph_id)?;
        validate_inspection_source_and_control(&result.inspection, &graph)?;
        Ok(result)
    }

    /// Loads and fully revalidates one admitted contract and its Graph source.
    ///
    /// # Errors
    ///
    /// Returns a structured input, source, corruption, or storage error.
    pub fn inspect(
        &self,
        contract_id: &str,
    ) -> Result<
        GroupAgentNodeExecutionContractInspection,
        GroupAgentNodeExecutionContractServiceError,
    > {
        validate_identifier(contract_id, "contract ID")?;
        let inspection = self
            .contracts
            .inspect_group_agent_node_execution_contract(contract_id)
            .map_err(GroupAgentNodeExecutionContractServiceError::from)?;
        let inspection = checked_inspection(inspection)?;
        if inspection.record.contract_id != contract_id {
            return Err(super::error::corrupt(
                "store returned a different contract identity",
            ));
        }
        let graph = self.load_graph(&inspection.graph_run.run.graph_id)?;
        validate_inspection_source_and_control(&inspection, &graph)?;
        Ok(inspection)
    }

    /// Lists bounded metadata without exposing contract or Prompt plaintext.
    ///
    /// # Errors
    ///
    /// Returns a structured input, corruption, or storage error.
    pub fn list(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<
        Vec<GroupAgentNodeExecutionContractRecord>,
        GroupAgentNodeExecutionContractServiceError,
    > {
        validate_list_input(graph_run_id, limit)?;
        let records = self
            .contracts
            .list_group_agent_node_execution_contracts(graph_run_id, limit)
            .map_err(GroupAgentNodeExecutionContractServiceError::from)?;
        validate_list(&records, graph_run_id, limit)?;
        Ok(records)
    }

    fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentGraphInspection,
        GroupAgentNodeExecutionContractServiceError,
    > {
        let graph = self
            .graphs
            .inspect_group_agent_graph(graph_id)
            .map_err(GroupAgentNodeExecutionContractServiceError::from)?;
        checked_graph(graph)
    }

    pub(super) fn load_run(
        &self,
        graph_run_id: &str,
    ) -> Result<
        crate::runtime_domain::GroupAgentGraphRunInspection,
        GroupAgentNodeExecutionContractServiceError,
    > {
        let run = self
            .runs
            .inspect_group_agent_graph_run(graph_run_id)
            .map_err(GroupAgentNodeExecutionContractServiceError::from)?;
        let run = checked_run(run)?;
        if run.run.graph_run_id != graph_run_id {
            return Err(super::error::corrupt(
                "store returned a different Graph Run identity",
            ));
        }
        Ok(run)
    }
}
