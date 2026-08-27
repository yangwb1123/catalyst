use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphInspection,
    GroupAgentGraphRunInspection, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeContractInspection, GroupAgentScheduledNodeLifecycleInspection,
    GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledNodePredecessorOutcome,
    GroupAgentScheduledNodePredecessorReceipt, GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalArtifactKind,
    GroupAgentScheduledNodeTerminalReceipt, HubEntity, HubStoreError,
    ScheduledGraphProgressSnapshot, ScheduledReadyNodeReleaseSource,
};

use super::super::{
    group_agent_graph, group_agent_graph_run, group_agent_scheduled_node_contract,
    group_agent_scheduled_node_lifecycle, group_agent_scheduled_node_provider_request,
    group_agent_scheduled_node_successor, read_error,
};

pub(super) fn inspect(
    connection: &mut Connection,
    graph_run_id: &str,
    expected_snapshot_sha256: &str,
    execution_ordinal: usize,
    node_id: &str,
) -> Result<ScheduledReadyNodeReleaseSource, HubStoreError> {
    inspect_after_progress(
        connection,
        graph_run_id,
        expected_snapshot_sha256,
        execution_ordinal,
        node_id,
        || Ok(()),
    )
}

fn inspect_after_progress(
    connection: &mut Connection,
    graph_run_id: &str,
    expected_snapshot_sha256: &str,
    execution_ordinal: usize,
    node_id: &str,
    after_progress: impl FnOnce() -> Result<(), HubStoreError>,
) -> Result<ScheduledReadyNodeReleaseSource, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let progress = super::read::build_in_snapshot(&transaction, graph_run_id)?;
    validate_expected(
        &progress,
        graph_run_id,
        expected_snapshot_sha256,
        execution_ordinal,
        node_id,
    )?;
    after_progress()?;
    let run = group_agent_graph_run::read::inspect_in_snapshot(&transaction, graph_run_id)?;
    let graph = group_agent_graph::read::inspect_in_snapshot(&transaction, &run.run.graph_id)?;
    let schedule = required_schedule(&transaction, &run, &graph)?;
    validate_schedule_binding(&progress, &schedule)?;
    let provider = load_selected(&transaction, &run, &graph, &schedule, execution_ordinal)?;
    validate_selected_binding(&progress.nodes[execution_ordinal], &provider)?;
    reject_selected_lifecycle(&transaction, &run, &provider)?;
    let (receipts, artifact) =
        load_direct_closure(&transaction, &progress, &run, &graph, &schedule, &provider)?;
    transaction.commit().map_err(read_error)?;
    Ok(ScheduledReadyNodeReleaseSource {
        progress_snapshot: progress,
        graph_run: run,
        graph,
        schedule,
        selected_provider_request: provider,
        direct_predecessor_receipts: receipts,
        predecessor_content_artifact: artifact,
    })
}

#[cfg(test)]
pub(super) fn inspect_with_concurrent_writer(
    connection: &mut Connection,
    graph_run_id: &str,
    expected_snapshot_sha256: &str,
    execution_ordinal: usize,
    node_id: &str,
    writer: impl FnOnce() -> Result<(), HubStoreError>,
) -> Result<ScheduledReadyNodeReleaseSource, HubStoreError> {
    inspect_after_progress(
        connection,
        graph_run_id,
        expected_snapshot_sha256,
        execution_ordinal,
        node_id,
        writer,
    )
}

fn validate_expected(
    progress: &ScheduledGraphProgressSnapshot,
    graph_run_id: &str,
    expected_sha256: &str,
    ordinal: usize,
    node_id: &str,
) -> Result<(), HubStoreError> {
    let selected = progress.nodes.get(ordinal);
    let matches = progress.snapshot_sha256 == expected_sha256
        && progress.graph_run_id == graph_run_id
        && selected.is_some_and(|node| {
            node.execution_ordinal == ordinal
                && node.node_id == node_id
                && node.candidate_id.is_some()
                && node.provider_request_id.is_some()
                && node.lifecycle_status.is_none()
                && node.terminal_outcome.is_none()
                && node.terminal_receipt_sha256.is_none()
        });
    matches
        .then_some(())
        .ok_or_else(|| conflict("scheduled ready-node source is stale or not release-ready"))
}

fn required_schedule(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError> {
    group_agent_graph_run::schedule::read::validate_graph_run_binding(connection, run, graph)?
        .ok_or_else(|| corrupt("scheduled ready-node source has no exact schedule"))
}

fn validate_schedule_binding(
    progress: &ScheduledGraphProgressSnapshot,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
) -> Result<(), HubStoreError> {
    let exact = progress.graph_run_id == schedule.schedule.graph_run_id
        && progress.graph_id == schedule.schedule.graph_id
        && progress.schedule_id == schedule.schedule.schedule_id
        && progress.schedule_sha256 == schedule.schedule.schedule_sha256
        && progress.nodes.len() == schedule.schedule.nodes.len();
    exact
        .then_some(())
        .ok_or_else(|| corrupt("scheduled ready-node schedule binding disagrees"))
}

fn load_selected(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
    ordinal: usize,
) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError> {
    let candidate = load_candidate(connection, run, graph, schedule, ordinal)?
        .ok_or_else(|| corrupt("scheduled ready node has no candidate"))?;
    group_agent_scheduled_node_provider_request::read::inspect_ordinal_in_snapshot(
        connection,
        &schedule.record.schedule_id,
        ordinal,
        Some(&candidate),
    )?
    .ok_or_else(|| corrupt("scheduled ready node has no provider request"))
}

fn validate_selected_binding(
    selected: &crate::runtime_domain::ScheduledGraphProgressNode,
    provider: &GroupAgentScheduledNodeProviderRequestInspection,
) -> Result<(), HubStoreError> {
    let candidate = &provider.scheduled_contract.candidate;
    let exact = selected.candidate_id.as_deref() == Some(candidate.contract_id.as_str())
        && selected.candidate_sha256.as_deref() == Some(candidate.contract_sha256.as_str())
        && selected.provider_request_id.as_deref()
            == Some(provider.record.provider_request_id.as_str())
        && selected.prepared_request_sha256.as_deref()
            == Some(provider.record.prepared_request_sha256.as_str());
    exact
        .then_some(())
        .ok_or_else(|| corrupt("scheduled ready-node selected evidence disagrees"))
}

fn reject_selected_lifecycle(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    provider: &GroupAgentScheduledNodeProviderRequestInspection,
) -> Result<(), HubStoreError> {
    let lifecycle = group_agent_scheduled_node_lifecycle::read::inspect_for_progress_in_snapshot(
        connection, run, provider,
    )?;
    lifecycle
        .is_none()
        .then_some(())
        .ok_or_else(|| conflict("scheduled ready node already has a lifecycle"))
}

fn load_direct_closure(
    connection: &Connection,
    progress: &ScheduledGraphProgressSnapshot,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
    selected: &GroupAgentScheduledNodeProviderRequestInspection,
) -> Result<
    (
        Vec<GroupAgentScheduledNodeTerminalReceipt>,
        Option<GroupAgentScheduledNodeTerminalArtifact>,
    ),
    HubStoreError,
> {
    let candidate = &selected.scheduled_contract.candidate;
    let scheduled = schedule
        .schedule
        .nodes
        .get(candidate.node.execution_ordinal)
        .ok_or_else(|| corrupt("scheduled ready node ordinal is outside schedule"))?;
    if scheduled.direct_predecessor_node_ids != candidate.request.required_predecessor_node_ids
        || scheduled.direct_predecessor_node_ids.len()
            != candidate.request.predecessor_terminal_receipts.len()
    {
        return Err(corrupt(
            "scheduled ready-node direct predecessor order disagrees",
        ));
    }
    let mut receipts = Vec::with_capacity(scheduled.direct_predecessor_node_ids.len());
    let mut first_artifact = None;
    for (index, predecessor_id) in scheduled.direct_predecessor_node_ids.iter().enumerate() {
        let lifecycle = load_predecessor(connection, run, graph, schedule, predecessor_id)?;
        let receipt = terminal_receipt(&lifecycle)?;
        validate_predecessor(
            progress,
            &candidate.request.predecessor_terminal_receipts[index],
            &lifecycle,
            receipt,
        )?;
        if index == 0 {
            first_artifact.clone_from(&lifecycle.artifact);
        }
        receipts.push(receipt.clone());
    }
    let artifact = content_artifact(candidate, first_artifact)?;
    Ok((receipts, artifact))
}

fn load_predecessor(
    connection: &Connection,
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
    schedule: &GroupAgentGraphExecutionScheduleInspection,
    node_id: &str,
) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError> {
    let source = schedule
        .schedule
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .ok_or_else(|| corrupt("scheduled direct predecessor is outside schedule"))?;
    let candidate = load_candidate(connection, run, graph, schedule, source.execution_ordinal)?
        .ok_or_else(|| corrupt("scheduled direct predecessor has no candidate"))?;
    let provider = group_agent_scheduled_node_provider_request::read::inspect_ordinal_in_snapshot(
        connection,
        &schedule.record.schedule_id,
        source.execution_ordinal,
        Some(&candidate),
    )?
    .ok_or_else(|| corrupt("scheduled direct predecessor has no provider request"))?;
    group_agent_scheduled_node_lifecycle::read::inspect_for_progress_in_snapshot(
        connection, run, &provider,
    )?
    .ok_or_else(|| corrupt("scheduled direct predecessor has no lifecycle"))
}

fn terminal_receipt(
    lifecycle: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<&GroupAgentScheduledNodeTerminalReceipt, HubStoreError> {
    if lifecycle.status != GroupAgentScheduledNodeLifecycleStatus::Terminalized {
        return Err(corrupt("scheduled direct predecessor is not terminalized"));
    }
    lifecycle
        .terminal_receipt
        .as_ref()
        .filter(|receipt| {
            receipt.node_outcome == GroupAgentNodeTerminalOutcome::Completed
                && receipt.artifact_kind == GroupAgentScheduledNodeTerminalArtifactKind::Result
        })
        .ok_or_else(|| corrupt("scheduled direct predecessor has no completed result receipt"))
}

fn validate_predecessor(
    progress: &ScheduledGraphProgressSnapshot,
    compact: &GroupAgentScheduledNodePredecessorReceipt,
    lifecycle: &GroupAgentScheduledNodeLifecycleInspection,
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
) -> Result<(), HubStoreError> {
    let ordinal = lifecycle.provider_request.execution_ordinal;
    let node = progress
        .nodes
        .get(ordinal)
        .ok_or_else(|| corrupt("scheduled predecessor progress ordinal is absent"))?;
    let exact = node.node_id == receipt.node_id
        && node.provider_request_id.as_deref() == Some(receipt.provider_request_id.as_str())
        && node.lifecycle_status == Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
        && node.terminal_outcome == Some(GroupAgentNodeTerminalOutcome::Completed)
        && node.terminal_receipt_sha256.as_deref() == Some(receipt.receipt_sha256.as_str())
        && compact.predecessor_node_id == receipt.node_id
        && compact.predecessor_attempt == receipt.attempt
        && compact.terminal_receipt_id == receipt.receipt_id
        && compact.terminal_receipt_sha256 == receipt.receipt_sha256
        && compact.terminal_event_seq == 0
        && compact.terminal_event_sha256.is_empty()
        && compact.node_outcome == GroupAgentScheduledNodePredecessorOutcome::Completed
        && compact.provider_request_id == receipt.provider_request_id
        && compact.dispatch_id == receipt.dispatch_id;
    exact
        .then_some(())
        .ok_or_else(|| corrupt("scheduled predecessor receipt closure disagrees"))
}

fn content_artifact(
    candidate: &crate::runtime_domain::GroupAgentScheduledNodeContractCandidate,
    first: Option<GroupAgentScheduledNodeTerminalArtifact>,
) -> Result<Option<GroupAgentScheduledNodeTerminalArtifact>, HubStoreError> {
    if !candidate.request.predecessor_content_included {
        return Ok(None);
    }
    let artifact = first
        .filter(|value| value.artifact_kind == GroupAgentScheduledNodeTerminalArtifactKind::Result)
        .ok_or_else(|| corrupt("scheduled predecessor content has no durable result artifact"))?;
    let content = crate::runtime_domain::group_agent_scheduled_node_predecessor_output(
        &candidate.request.user_prompt,
    )
    .map_err(|_| corrupt("scheduled predecessor content Prompt is invalid"))?;
    (content.as_deref() == Some(artifact.output_text.as_str()))
        .then_some(Some(artifact))
        .ok_or_else(|| corrupt("scheduled predecessor content disagrees with durable artifact"))
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

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupAgentGraphRun,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
