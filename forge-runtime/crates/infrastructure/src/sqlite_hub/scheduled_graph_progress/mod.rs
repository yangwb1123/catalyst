mod node;
mod read;
pub(in crate::sqlite_hub) mod ready_release;

#[cfg(test)]
pub(in crate::sqlite_hub) use read::atomicity_fixture;

#[cfg(test)]
pub(in crate::sqlite_hub) mod atomicity_terminal;
#[cfg(test)]
mod bounds;
#[cfg(test)]
mod bounds_support;
#[cfg(test)]
mod corruption_tests;
#[cfg(test)]
mod global_project_lane_tests;
#[cfg(test)]
mod ready_release_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
pub(in crate::sqlite_hub) mod sqlite_group_agent_graph_execution_schedule_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
pub(in crate::sqlite_hub) mod sqlite_group_agent_graph_run_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_node_dispatch_request_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
mod sqlite_group_agent_node_dispatch_request_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_node_execution_contract_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
mod sqlite_group_agent_node_execution_contract_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_node_lifecycle_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
#[allow(unused_imports)]
mod sqlite_group_agent_node_lifecycle_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
pub(in crate::sqlite_hub) mod sqlite_group_agent_scheduled_node_contract_support;
#[cfg(test)]
#[path = "../../../tests/sqlite_group_agent_scheduled_node_provider_request_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
pub(in crate::sqlite_hub) mod sqlite_group_agent_scheduled_node_provider_request_support;

use crate::runtime_domain::{
    HubStoreError, ScheduledGraphProgressSnapshot, ScheduledGraphProgressStore,
    ScheduledReadyNodeReleaseSource, ScheduledReadyNodeReleaseStore,
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

impl ScheduledReadyNodeReleaseStore for SqliteHubStore {
    fn inspect_scheduled_ready_node_release(
        &self,
        graph_run_id: &str,
        expected_snapshot_sha256: &str,
        execution_ordinal: usize,
        node_id: &str,
    ) -> Result<ScheduledReadyNodeReleaseSource, HubStoreError> {
        ready_release::inspect(
            &mut self.connect()?,
            graph_run_id,
            expected_snapshot_sha256,
            execution_ordinal,
            node_id,
        )
    }
}
