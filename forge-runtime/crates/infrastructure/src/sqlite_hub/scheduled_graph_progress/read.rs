use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, HubEntity, HubStoreError,
    SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION, SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
    ScheduledGraphProgressSnapshot,
};

use super::super::{
    group_agent_graph, group_agent_graph_run, read_error,
    scheduled_graph_progress::node::{ObservedProgressCounts, project_nodes},
};

pub(super) fn snapshot(
    connection: &mut Connection,
    graph_run_id: &str,
) -> Result<ScheduledGraphProgressSnapshot, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let snapshot = build(&transaction, graph_run_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(snapshot)
}

fn build(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<ScheduledGraphProgressSnapshot, HubStoreError> {
    let run = group_agent_graph_run::read::inspect_in_snapshot(connection, graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(connection, &run.run.graph_id)?;
    let schedule = required_schedule(connection, &run, &graph)?;
    let (nodes, observed) = project_nodes(connection, &run, &graph, &schedule)?;
    validate_stored_counts(connection, graph_run_id, observed)?;
    seal_snapshot(schedule, nodes)
}

fn required_schedule(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
    group_agent_graph_run::schedule::read::validate_graph_run_binding(connection, run, graph)?
        .ok_or_else(|| HubStoreError::NotFound {
            entity: HubEntity::GroupAgentGraphExecutionSchedule,
            id: run.run.graph_run_id.clone(),
        })
}

fn seal_snapshot(
    schedule: GroupAgentGraphExecutionScheduleInspection,
    nodes: Vec<crate::runtime_domain::ScheduledGraphProgressNode>,
) -> Result<ScheduledGraphProgressSnapshot, HubStoreError> {
    let source = schedule.schedule;
    ScheduledGraphProgressSnapshot {
        v: SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: source.graph_run_id,
        graph_id: source.graph_id,
        schedule_id: source.schedule_id,
        schedule_sha256: source.schedule_sha256,
        node_count: source.node_count,
        execution_mode: source.execution_mode,
        max_in_flight_nodes: source.max_in_flight_nodes,
        progression_policy: source.progression_policy,
        attempt_policy: source.attempt_policy,
        failure_policy: source.failure_policy,
        nodes,
        snapshot_sha256: String::new(),
    }
    .seal()
    .map_err(|error| corrupt(&error.message))
}

fn validate_stored_counts(
    connection: &Connection,
    graph_run_id: &str,
    observed: ObservedProgressCounts,
) -> Result<(), HubStoreError> {
    let stored = stored_counts(connection, graph_run_id)?;
    (stored == observed)
        .then_some(())
        .ok_or_else(|| corrupt("scheduled Graph progress row counts disagree"))
}

fn stored_counts(
    connection: &Connection,
    graph_run_id: &str,
) -> Result<ObservedProgressCounts, HubStoreError> {
    let values: (i64, i64, i64) = connection
        .query_row(
            "SELECT
             (SELECT COUNT(*) FROM group_agent_graph_scheduled_node_contract_candidates
                WHERE graph_run_id=?1)
             +(SELECT COUNT(*) FROM group_agent_graph_scheduled_node_successor_candidates
                WHERE graph_run_id=?1),
             (SELECT COUNT(*) FROM group_agent_graph_scheduled_node_provider_requests
                WHERE graph_run_id=?1),
             (SELECT COUNT(*) FROM group_agent_graph_scheduled_node_dispatch_lifecycles
                WHERE graph_run_id=?1)",
            [graph_run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(read_error)?;
    ObservedProgressCounts::from_stored(values)
        .ok_or_else(|| corrupt("scheduled Graph progress stored row count is outside its bound"))
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
