mod claim;
mod read;
mod terminalize;

use crate::runtime_domain::{
    ClaimGroupAgentScheduledNodeDispatch, ClaimGroupAgentScheduledNodeDispatchResult,
    GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStore,
    HubStoreError, TerminalizeGroupAgentScheduledNodeDispatch,
    TerminalizeGroupAgentScheduledNodeDispatchResult,
};

pub(in crate::sqlite_hub) use read::{has_graph_run_child, validate_graph_run_binding};

use super::SqliteHubStore;

impl GroupAgentScheduledNodeLifecycleStore for SqliteHubStore {
    fn claim_group_agent_scheduled_node_dispatch(
        &self,
        request: &ClaimGroupAgentScheduledNodeDispatch,
    ) -> Result<ClaimGroupAgentScheduledNodeDispatchResult, HubStoreError> {
        claim::claim(&mut self.connect()?, request)
    }

    fn terminalize_group_agent_scheduled_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentScheduledNodeDispatch,
    ) -> Result<TerminalizeGroupAgentScheduledNodeDispatchResult, HubStoreError> {
        terminalize::terminalize(&mut self.connect()?, request)
    }

    fn inspect_group_agent_scheduled_node_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, provider_request_id)
    }
}
