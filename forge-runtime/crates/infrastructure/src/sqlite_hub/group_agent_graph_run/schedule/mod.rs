pub(super) mod read;
mod rows;
mod write;

use crate::runtime_domain::{
    AdmitGroupAgentGraphExecutionSchedule, AdmitGroupAgentGraphExecutionScheduleResult,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleRecord,
    GroupAgentGraphExecutionScheduleStore, HubStoreError,
};

use super::super::super::SqliteHubStore;

impl GroupAgentGraphExecutionScheduleStore for SqliteHubStore {
    fn admit_group_agent_graph_execution_schedule(
        &self,
        request: &AdmitGroupAgentGraphExecutionSchedule,
    ) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError> {
        write::admit(&mut self.connect()?, request)
    }

    fn inspect_group_agent_graph_execution_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, schedule_id)
    }

    fn list_group_agent_graph_execution_schedules(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphExecutionScheduleRecord>, HubStoreError> {
        read::list(&mut self.connect()?, graph_run_id, limit)
    }
}

#[cfg(test)]
#[path = "../../tests/group_agent_graph_execution_schedule_atomicity.rs"]
mod atomicity_tests;
#[cfg(test)]
#[path = "../../../../tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
#[allow(dead_code)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[cfg(test)]
#[path = "../../../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code)]
#[allow(clippy::duplicate_mod)]
mod sqlite_group_agent_graph_run_support;
