mod identity;
pub(in crate::sqlite_hub) mod read;
mod rows;
mod write;

use crate::runtime_domain::{
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestRecord,
    GroupAgentScheduledNodeProviderRequestStore, HubStoreError,
    PrepareGroupAgentScheduledNodeProviderRequest,
    PrepareGroupAgentScheduledNodeProviderRequestResult,
};

use super::SqliteHubStore;

impl GroupAgentScheduledNodeProviderRequestStore for SqliteHubStore {
    fn prepare_group_agent_scheduled_node_provider_request(
        &self,
        request: &PrepareGroupAgentScheduledNodeProviderRequest,
    ) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError> {
        write::prepare(&mut self.connect()?, request)
    }

    fn inspect_group_agent_scheduled_node_provider_request(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, provider_request_id)
    }

    fn list_group_agent_scheduled_node_provider_requests(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentScheduledNodeProviderRequestRecord>, HubStoreError> {
        read::list(&mut self.connect()?, graph_run_id, limit)
    }
}

#[cfg(test)]
#[path = "../tests/group_agent_scheduled_node_provider_request_atomicity.rs"]
mod atomicity_tests;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_run_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_scheduled_node_contract_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_scheduled_node_provider_request_support;
