mod claim;
mod codec;
mod read;
mod rows;
mod source;
mod terminalize;

#[cfg(test)]
#[path = "../tests/group_agent_node_lifecycle_atomicity.rs"]
mod atomicity_tests;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_node_dispatch_request_support/mod.rs"]
#[allow(dead_code)]
mod sqlite_group_agent_node_dispatch_request_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_node_execution_contract_support/mod.rs"]
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_node_lifecycle_support/mod.rs"]
#[allow(dead_code)]
mod sqlite_group_agent_node_lifecycle_support;

use crate::runtime_domain::{
    ClaimGroupAgentNodeDispatch, ClaimGroupAgentNodeDispatchResult,
    GroupAgentNodeLifecycleInspection, GroupAgentNodeLifecycleStore, HubStoreError,
    TerminalizeGroupAgentNodeDispatch, TerminalizeGroupAgentNodeDispatchResult,
};
use rusqlite::{Connection, TransactionBehavior};

use super::{SqliteHubStore, group_agent_graph_run, read_error};

pub(in crate::sqlite_hub) use read::validate_graph_run_binding;

pub(in crate::sqlite_hub) fn inspect_if_present(
    connection: &mut Connection,
    graph_run_id: &str,
) -> Result<Option<GroupAgentNodeLifecycleInspection>, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let version: i64 = transaction
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(read_error)?;
    let graph_run = group_agent_graph_run::read::inspect_in_snapshot(&transaction, graph_run_id)?;
    let inspection = if version == 11 {
        None
    } else {
        read::reconstruct(&transaction, graph_run)?
    };
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

impl GroupAgentNodeLifecycleStore for SqliteHubStore {
    fn claim_group_agent_node_dispatch(
        &self,
        request: &ClaimGroupAgentNodeDispatch,
    ) -> Result<ClaimGroupAgentNodeDispatchResult, HubStoreError> {
        claim::claim(&mut self.connect()?, request)
    }

    fn terminalize_group_agent_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentNodeDispatch,
    ) -> Result<TerminalizeGroupAgentNodeDispatchResult, HubStoreError> {
        terminalize::terminalize(&mut self.connect()?, request)
    }

    fn inspect_group_agent_node_lifecycle(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, graph_run_id)
    }
}
