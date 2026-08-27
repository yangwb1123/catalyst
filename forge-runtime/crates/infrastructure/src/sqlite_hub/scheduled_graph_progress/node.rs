use rusqlite::Connection;

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeLifecycleProgressInspection,
    GroupAgentScheduledNodeProviderRequestInspection, HubStoreError, MAX_GROUP_AGENT_GRAPH_NODES,
    ScheduledGraphProgressNode,
};

use super::super::{
    group_agent_scheduled_node_contract, group_agent_scheduled_node_lifecycle,
    group_agent_scheduled_node_provider_request, group_agent_scheduled_node_successor,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct ObservedProgressCounts {
    candidates: usize,
    provider_requests: usize,
    lifecycles: usize,
}

impl ObservedProgressCounts {
    pub(super) fn from_stored(values: (i64, i64, i64)) -> Option<Self> {
        let counts = Self {
            candidates: usize::try_from(values.0).ok()?,
            provider_requests: usize::try_from(values.1).ok()?,
            lifecycles: usize::try_from(values.2).ok()?,
        };
        (counts.candidates <= MAX_GROUP_AGENT_GRAPH_NODES
            && counts.provider_requests <= MAX_GROUP_AGENT_GRAPH_NODES
            && counts.lifecycles <= MAX_GROUP_AGENT_GRAPH_NODES)
            .then_some(counts)
    }

    fn observe(
        &mut self,
        candidate: Option<&GroupAgentScheduledNodeContractInspection>,
        provider: Option<&GroupAgentScheduledNodeProviderRequestInspection>,
        lifecycle: Option<&GroupAgentScheduledNodeLifecycleProgressInspection>,
    ) {
        self.candidates += usize::from(candidate.is_some());
        self.provider_requests += usize::from(provider.is_some());
        self.lifecycles += usize::from(lifecycle.is_some());
    }
}

pub(super) fn project_nodes(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
) -> Result<(Vec<ScheduledGraphProgressNode>, ObservedProgressCounts), HubStoreError> {
    let mut nodes = Vec::with_capacity(schedule.schedule.node_count);
    let mut counts = ObservedProgressCounts::default();
    for source in &schedule.schedule.nodes {
        let candidate = load_candidate(connection, run, graph, schedule, source.execution_ordinal)?;
        let provider =
            group_agent_scheduled_node_provider_request::read::inspect_ordinal_in_snapshot(
                connection,
                &schedule.record.schedule_id,
                source.execution_ordinal,
                candidate.as_ref(),
            )?;
        let lifecycle = load_lifecycle(connection, run, provider.as_ref())?;
        counts.observe(candidate.as_ref(), provider.as_ref(), lifecycle.as_ref());
        nodes.push(project_node(
            source,
            candidate.as_ref(),
            provider.as_ref(),
            lifecycle.as_ref(),
        ));
    }
    Ok((nodes, counts))
}

fn load_candidate(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
    ordinal: usize,
) -> Result<Option<GroupAgentScheduledNodeContractInspection>, HubStoreError> {
    if ordinal == 0 {
        group_agent_scheduled_node_contract::read::inspect_ordinal_in_snapshot(
            connection, run, graph, schedule, ordinal,
        )
    } else {
        group_agent_scheduled_node_successor::read::inspect_ordinal_in_snapshot(
            connection, run, graph, schedule, ordinal,
        )
    }
}

fn load_lifecycle(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    provider: Option<&GroupAgentScheduledNodeProviderRequestInspection>,
) -> Result<Option<GroupAgentScheduledNodeLifecycleProgressInspection>, HubStoreError> {
    provider.map_or(Ok(None), |provider| {
        group_agent_scheduled_node_lifecycle::read::inspect_for_progress_in_snapshot(
            connection, run, provider,
        )
    })
}

fn project_node(
    source: &crate::runtime_domain::GroupAgentGraphExecutionScheduleNode,
    candidate: Option<&GroupAgentScheduledNodeContractInspection>,
    provider: Option<&GroupAgentScheduledNodeProviderRequestInspection>,
    lifecycle: Option<&GroupAgentScheduledNodeLifecycleProgressInspection>,
) -> ScheduledGraphProgressNode {
    let receipt = lifecycle.and_then(|inspection| inspection.terminal_receipt.as_ref());
    ScheduledGraphProgressNode {
        execution_ordinal: source.execution_ordinal,
        node_id: source.node_id.clone(),
        attempt: source.attempt,
        candidate_id: candidate.map(|value| value.record.contract_id.clone()),
        candidate_sha256: candidate.map(|value| value.record.contract_sha256.clone()),
        provider_request_id: provider.map(|value| value.record.provider_request_id.clone()),
        prepared_request_sha256: provider.map(|value| value.record.prepared_request_sha256.clone()),
        lifecycle_status: lifecycle.map(|value| value.status),
        terminal_outcome: receipt.map(|value| value.node_outcome),
        terminal_receipt_sha256: receipt.map(|value| value.receipt_sha256.clone()),
    }
}
