#[cfg(test)]
#[path = "tests/group_agent_graph_run_atomicity.rs"]
pub(in crate::sqlite_hub) mod atomicity_tests;
mod codec;
pub(in crate::sqlite_hub) mod read;
mod rows;
#[path = "group_agent_graph_run/schedule/mod.rs"]
pub(in crate::sqlite_hub) mod schedule;
mod write;

use crate::runtime_domain::{
    BeginGroupAgentGraphRun, BeginGroupAgentGraphRunDisposition, BeginGroupAgentGraphRunResult,
    GROUP_AGENT_GRAPH_RUN_VERSION, GroupAgentGraphCorePlan, GroupAgentGraphInspection,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, GroupAgentGraphRunStatus, GroupAgentGraphRunStore, HubEntity,
    HubStoreError, MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES, MAX_GROUP_AGENT_GRAPH_RUN_LIST_LIMIT,
};

#[cfg(test)]
use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_CORE_PLAN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphStore,
};

use super::SqliteHubStore;

impl GroupAgentGraphRunStore for SqliteHubStore {
    fn begin_group_agent_graph_run(
        &self,
        request: &BeginGroupAgentGraphRun,
    ) -> Result<BeginGroupAgentGraphRunResult, HubStoreError> {
        write::begin(&mut self.connect()?, request)
    }

    fn inspect_group_agent_graph_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, HubStoreError> {
        read::inspect(&mut self.connect()?, graph_run_id)
    }

    fn list_group_agent_graph_runs(
        &self,
        graph_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError> {
        read::list(&self.connect()?, graph_id, limit)
    }
}
