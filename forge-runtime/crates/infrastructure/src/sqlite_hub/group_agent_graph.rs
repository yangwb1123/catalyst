#[cfg(test)]
#[path = "tests/group_agent_graph_atomicity.rs"]
pub(in crate::sqlite_hub) mod atomicity_tests;
mod codec;
pub(in crate::sqlite_hub) mod read;
mod rows;
mod write;

use crate::runtime_domain::{
    GroupAgentGraphInspection, GroupAgentGraphRecord, GroupAgentGraphStore, HubStoreError,
    PrepareGroupAgentGraph, PrepareGroupAgentGraphResult,
};

use super::SqliteHubStore;

impl GroupAgentGraphStore for SqliteHubStore {
    fn prepare_group_agent_graph(
        &self,
        request: &PrepareGroupAgentGraph,
    ) -> Result<PrepareGroupAgentGraphResult, HubStoreError> {
        write::prepare(&mut self.connect()?, request)
    }

    fn inspect_group_agent_graph(
        &self,
        graph_id: &str,
    ) -> Result<GroupAgentGraphInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, graph_id)
    }

    fn list_group_agent_graphs(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRecord>, HubStoreError> {
        read::list(&self.connect()?, group_run_id, limit)
    }
}
