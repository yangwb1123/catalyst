pub(super) mod read;
pub(super) mod rows;
mod write;

use crate::runtime_domain::{
    AdmitGroupAgentScheduledNodeContractCandidate, AdmitGroupAgentScheduledNodeContractResult,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeContractRecord,
    GroupAgentScheduledNodeSuccessorStore, HubStoreError,
};

use super::SqliteHubStore;

impl GroupAgentScheduledNodeSuccessorStore for SqliteHubStore {
    fn admit_group_agent_scheduled_node_successor(
        &self,
        request: &AdmitGroupAgentScheduledNodeContractCandidate,
    ) -> Result<AdmitGroupAgentScheduledNodeContractResult, HubStoreError> {
        write::admit(&mut self.connect()?, request)
    }

    fn inspect_group_agent_scheduled_node_successor(
        &self,
        contract_id: &str,
    ) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, contract_id)
    }

    fn list_group_agent_scheduled_node_successors(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentScheduledNodeContractRecord>, HubStoreError> {
        read::list(&mut self.connect()?, graph_run_id, limit)
    }
}

#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_run_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_node_execution_contract_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_node_execution_contract_support;
