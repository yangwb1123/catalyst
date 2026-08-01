#[cfg(test)]
#[path = "../tests/group_agent_node_dispatch_request_atomicity.rs"]
mod atomicity_tests;
#[path = "dispatch_request/codec.rs"]
mod codec;
#[path = "dispatch_request/read.rs"]
pub(in crate::sqlite_hub) mod read;
#[path = "dispatch_request/rows.rs"]
mod rows;
#[path = "dispatch_request/write.rs"]
mod write;

use crate::runtime_domain::{
    GroupAgentNodeDispatchRequestInspection, GroupAgentNodeDispatchRequestRecord,
    GroupAgentNodeDispatchRequestStore, HubStoreError, PrepareGroupAgentNodeDispatchRequest,
    PrepareGroupAgentNodeDispatchRequestResult,
};

use super::super::SqliteHubStore;

impl GroupAgentNodeDispatchRequestStore for SqliteHubStore {
    fn prepare_group_agent_node_dispatch_request(
        &self,
        request: &PrepareGroupAgentNodeDispatchRequest,
    ) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError> {
        write::prepare(&mut self.connect()?, request)
    }

    fn inspect_group_agent_node_dispatch_request(
        &self,
        dispatch_request_id: &str,
    ) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, dispatch_request_id)
    }

    fn list_group_agent_node_dispatch_requests(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentNodeDispatchRequestRecord>, HubStoreError> {
        read::list(&self.connect()?, graph_run_id, limit)
    }
}
