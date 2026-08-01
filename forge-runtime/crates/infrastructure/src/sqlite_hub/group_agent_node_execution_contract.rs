#[cfg(test)]
#[path = "tests/group_agent_node_execution_contract_atomicity.rs"]
mod atomicity_tests;
mod codec;
mod read;
mod rows;
mod snapshot;
mod write;

use crate::runtime_domain::{
    AdmitGroupAgentNodeExecutionContract, AdmitGroupAgentNodeExecutionContractResult,
    GroupAgentNodeExecutionContractInspection, GroupAgentNodeExecutionContractRecord,
    GroupAgentNodeExecutionContractStore, HubStoreError,
};

use super::SqliteHubStore;

impl GroupAgentNodeExecutionContractStore for SqliteHubStore {
    fn admit_group_agent_node_execution_contract(
        &self,
        request: &AdmitGroupAgentNodeExecutionContract,
    ) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError> {
        write::admit(&mut self.connect()?, request)
    }

    fn inspect_group_agent_node_execution_contract(
        &self,
        contract_id: &str,
    ) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, contract_id)
    }

    fn list_group_agent_node_execution_contracts(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentNodeExecutionContractRecord>, HubStoreError> {
        read::list(&self.connect()?, graph_run_id, limit)
    }
}
