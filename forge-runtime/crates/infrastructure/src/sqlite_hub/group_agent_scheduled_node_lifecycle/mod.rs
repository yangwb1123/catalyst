mod adjudicate;
mod claim;
mod claim_ready;
pub(in crate::sqlite_hub) mod read;
mod read_ready;
mod terminalize;

#[cfg(test)]
mod ready_claim_cas_tests;
#[cfg(test)]
mod ready_claim_test_support;
#[cfg(test)]
mod timestamp_corruption_tests;
use crate::runtime_domain::{
    AdjudicateGroupAgentScheduledNodeDispatch, ClaimGroupAgentScheduledNodeDispatch,
    ClaimGroupAgentScheduledNodeDispatchResult, ClaimGroupAgentScheduledReadyNodeDispatch,
    ClaimGroupAgentScheduledReadyNodeDispatchResult, GroupAgentScheduledNodeAnyLifecycleInspection,
    GroupAgentScheduledNodeAnyLifecycleStore, GroupAgentScheduledNodeLifecycleInspection,
    GroupAgentScheduledNodeLifecycleStore, GroupAgentScheduledReadyNodeLifecycleInspection,
    GroupAgentScheduledReadyNodeLifecycleStore, HubStoreError,
    TerminalizeGroupAgentScheduledNodeDispatch, TerminalizeGroupAgentScheduledNodeDispatchResult,
    TerminalizeGroupAgentScheduledReadyNodeDispatchResult,
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

    fn adjudicate_group_agent_scheduled_node_dispatch(
        &self,
        request: &AdjudicateGroupAgentScheduledNodeDispatch,
    ) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
        adjudicate::adjudicate_legacy(&mut self.connect()?, request)
    }
}

impl GroupAgentScheduledReadyNodeLifecycleStore for SqliteHubStore {
    fn claim_group_agent_scheduled_ready_node_dispatch(
        &self,
        request: &ClaimGroupAgentScheduledReadyNodeDispatch,
    ) -> Result<ClaimGroupAgentScheduledReadyNodeDispatchResult, HubStoreError> {
        claim_ready::claim(&mut self.connect()?, request)
    }

    fn terminalize_group_agent_scheduled_ready_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentScheduledNodeDispatch,
    ) -> Result<TerminalizeGroupAgentScheduledReadyNodeDispatchResult, HubStoreError> {
        terminalize::terminalize_ready(&mut self.connect()?, request)
    }

    fn inspect_group_agent_scheduled_ready_node_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledReadyNodeLifecycleInspection, HubStoreError> {
        read_ready::inspect(&mut self.connect()?, provider_request_id)
    }
}

impl GroupAgentScheduledNodeAnyLifecycleStore for SqliteHubStore {
    fn inspect_group_agent_scheduled_node_any_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Deferred)
            .map_err(super::read_error)?;
        let inspection = read::inspect_any_in_snapshot(&transaction, provider_request_id)?;
        transaction.commit().map_err(super::read_error)?;
        Ok(inspection)
    }

    fn adjudicate_group_agent_scheduled_node_any_dispatch(
        &self,
        request: &AdjudicateGroupAgentScheduledNodeDispatch,
    ) -> Result<GroupAgentScheduledNodeAnyLifecycleInspection, HubStoreError> {
        adjudicate::adjudicate_any(&mut self.connect()?, request)
    }
}
