mod node;
mod read;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
mod sqlite_group_agent_graph_run_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
mod sqlite_group_agent_scheduled_node_contract_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
mod sqlite_group_agent_scheduled_node_provider_request_support;

use crate::runtime_domain::{
    HubStoreError, ScheduledGraphProgressSnapshot, ScheduledGraphProgressStore,
};

use super::SqliteHubStore;

impl ScheduledGraphProgressStore for SqliteHubStore {
    fn snapshot_scheduled_graph_progress(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphProgressSnapshot, HubStoreError> {
        read::snapshot(&mut self.connect()?, graph_run_id)
    }
}
