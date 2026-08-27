use super::{
    GroupAgentScheduledReadyNodeDispatchReleaseControl,
    GroupAgentScheduledReadyNodeDispatchReleaseValidationError,
};
use crate::{
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeContractScope, GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeLifecycleStatus, GroupAgentScheduledNodePredecessorOutcome,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalArtifactKind,
    GroupAgentScheduledNodeTerminalReceipt, ScheduledGraphProgressNode,
    ScheduledGraphReconcileDisposition,
};

pub(super) fn validate_sources(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    validate_legacy_source(control)?;
    validate_progress_source(control)?;
    validate_selected_source(control)?;
    validate_direct_receipt_closure(control)?;
    validate_content_artifact(control)
}

fn validate_legacy_source(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let mut legacy = GroupAgentScheduledNodeDispatchReleaseControl {
        v: crate::GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        release_control_protocol_version:
            crate::GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: control.graph_run.clone(),
        journal_events: control.journal_events.clone(),
        control_snapshot: control.control_snapshot.clone(),
        schedule_record: control.schedule_record.clone(),
        schedule: control.schedule.clone(),
        scheduled_contract_record: control.scheduled_contract_record.clone(),
        scheduled_contract: control.scheduled_contract.clone(),
        provider_request: control.provider_request.clone(),
        provider_request_json: control.provider_request_json.clone(),
        snapshot_sha256: String::new(),
    };
    legacy.snapshot_sha256 = legacy
        .expected_sha256()
        .map_err(|_| invalid("legacy source invalid"))?;
    legacy.validate().map_err(|error| {
        invalid(&format!(
            "ready release exact source is invalid: {}",
            error.message
        ))
    })
}

fn validate_progress_source(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let progress = &control.progress_snapshot;
    let schedule = &control.schedule;
    progress
        .validate()
        .map_err(|_| invalid("ready release progress snapshot is invalid"))?;
    control
        .reconcile_decision
        .validate_against_snapshot(progress)
        .map_err(|_| invalid("ready release reconcile decision is invalid"))?;
    let valid = control.reconcile_decision.disposition == ScheduledGraphReconcileDisposition::Ready
        && progress.graph_run_id == control.graph_run.graph_run_id
        && progress.graph_id == control.graph_run.graph_id
        && progress.schedule_id == schedule.schedule_id
        && progress.schedule_sha256 == schedule.schedule_sha256
        && progress.node_count == schedule.node_count
        && progress.nodes.len() == schedule.nodes.len()
        && progress.execution_mode == schedule.execution_mode
        && progress.max_in_flight_nodes == schedule.max_in_flight_nodes
        && progress.progression_policy == schedule.progression_policy
        && progress.attempt_policy == schedule.attempt_policy
        && progress.failure_policy == schedule.failure_policy;
    valid
        .then_some(())
        .ok_or_else(|| invalid("ready release progress source disagrees"))?;
    validate_all_ordinals(control)
}

fn validate_all_ordinals(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let mapped = control
        .progress_snapshot
        .nodes
        .iter()
        .zip(&control.schedule.nodes)
        .all(|(progress, scheduled)| {
            progress.execution_ordinal == scheduled.execution_ordinal
                && progress.node_id == scheduled.node_id
                && progress.attempt == scheduled.attempt
        });
    mapped
        .then_some(())
        .ok_or_else(|| invalid("ready release progress ordinal mapping disagrees"))
}

fn validate_selected_source(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let ordinal = control
        .reconcile_decision
        .next_execution_ordinal
        .ok_or_else(|| invalid("ready decision has no selected ordinal"))?;
    let selected = control
        .progress_snapshot
        .nodes
        .get(ordinal)
        .ok_or_else(|| invalid("ready selected ordinal is outside progress"))?;
    let node_id = control
        .reconcile_decision
        .next_node_id
        .as_deref()
        .ok_or_else(|| invalid("ready decision has no selected node"))?;
    if !selected_matches(control, selected, ordinal, node_id) {
        return Err(invalid("ready selected source bindings disagree"));
    }
    validate_selected_scope(control, ordinal)
}

fn selected_matches(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    selected: &ScheduledGraphProgressNode,
    ordinal: usize,
    node_id: &str,
) -> bool {
    let candidate = &control.scheduled_contract;
    let request = &control.provider_request;
    selected.execution_ordinal == ordinal
        && selected.node_id == node_id
        && candidate.node.execution_ordinal == ordinal
        && candidate.node.node_id == node_id
        && selected.candidate_id.as_deref() == Some(candidate.contract_id.as_str())
        && selected.candidate_sha256.as_deref() == Some(candidate.contract_sha256.as_str())
        && selected.provider_request_id.as_deref() == Some(request.provider_request_id.as_str())
        && selected.prepared_request_sha256.as_deref()
            == Some(request.prepared_request_sha256.as_str())
        && selected.lifecycle_status.is_none()
        && selected.terminal_outcome.is_none()
        && selected.terminal_receipt_sha256.is_none()
}

fn validate_selected_scope(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ordinal: usize,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let candidate = &control.scheduled_contract;
    let initial = ordinal == 0
        && candidate.contract_scope
            == GroupAgentScheduledNodeContractScope::ScheduleInitialNodeOnly
        && control.direct_predecessor_receipts.is_empty()
        && control.predecessor_content_artifact.is_none();
    let successor = ordinal > 0
        && candidate.contract_scope == GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    (initial || successor)
        .then_some(())
        .ok_or_else(|| invalid("ready selected candidate scope disagrees"))
}

fn validate_direct_receipt_closure(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let ordinal = control.scheduled_contract.node.execution_ordinal;
    let scheduled = control
        .schedule
        .nodes
        .get(ordinal)
        .ok_or_else(|| invalid("ready selected schedule node is absent"))?;
    let required = &scheduled.direct_predecessor_node_ids;
    let compact = &control
        .scheduled_contract
        .request
        .predecessor_terminal_receipts;
    if required.len() != compact.len()
        || required.len() != control.direct_predecessor_receipts.len()
    {
        return Err(invalid("ready direct predecessor receipt count disagrees"));
    }
    for (index, receipt) in control.direct_predecessor_receipts.iter().enumerate() {
        validate_direct_receipt(control, index, &required[index], receipt)?;
    }
    Ok(())
}

fn validate_direct_receipt(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    index: usize,
    predecessor_id: &str,
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    receipt
        .validate()
        .map_err(|_| invalid("ready direct predecessor receipt is invalid"))?;
    let scheduled = control
        .schedule
        .nodes
        .iter()
        .find(|node| node.node_id == predecessor_id)
        .ok_or_else(|| invalid("ready direct predecessor is outside schedule"))?;
    let progress = &control.progress_snapshot.nodes[scheduled.execution_ordinal];
    let compact = &control
        .scheduled_contract
        .request
        .predecessor_terminal_receipts[index];
    if receipt_matches_source(control, scheduled, progress, receipt)
        && compact_receipt_matches(compact, receipt, predecessor_id)
    {
        Ok(())
    } else {
        Err(invalid(
            "ready direct predecessor receipt closure disagrees",
        ))
    }
}

fn receipt_matches_source(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    scheduled: &crate::GroupAgentGraphExecutionScheduleNode,
    progress: &ScheduledGraphProgressNode,
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
) -> bool {
    receipt.graph_run_id == control.graph_run.graph_run_id
        && receipt.graph_id == control.graph_run.graph_id
        && receipt.node_id == scheduled.node_id
        && receipt.attempt == scheduled.attempt
        && receipt.project_lane_sha256 == scheduled.project_lane_sha256
        && receipt.artifact_kind == GroupAgentScheduledNodeTerminalArtifactKind::Result
        && receipt.node_outcome == GroupAgentNodeTerminalOutcome::Completed
        && !receipt.retry_authorized
        && receipt.lane_release_authorized
        && !receipt.successor_advance_authorized
        && progress.provider_request_id.as_deref() == Some(receipt.provider_request_id.as_str())
        && progress.lifecycle_status == Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized)
        && progress.terminal_outcome == Some(GroupAgentNodeTerminalOutcome::Completed)
        && progress.terminal_receipt_sha256.as_deref() == Some(receipt.receipt_sha256.as_str())
}

fn compact_receipt_matches(
    compact: &crate::GroupAgentScheduledNodePredecessorReceipt,
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
    predecessor_id: &str,
) -> bool {
    compact.predecessor_node_id == predecessor_id
        && compact.predecessor_attempt == receipt.attempt
        && compact.terminal_event_seq == 0
        && compact.terminal_event_sha256.is_empty()
        && compact.terminal_receipt_id == receipt.receipt_id
        && compact.terminal_receipt_sha256 == receipt.receipt_sha256
        && compact.node_outcome == GroupAgentScheduledNodePredecessorOutcome::Completed
        && compact.provider_request_id == receipt.provider_request_id
        && compact.dispatch_id == receipt.dispatch_id
}

fn validate_content_artifact(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    let content = crate::group_agent_scheduled_node_predecessor_output(
        &control.scheduled_contract.request.user_prompt,
    )
    .map_err(|_| invalid("ready predecessor content Prompt is invalid"))?;
    match (content, &control.predecessor_content_artifact) {
        (None, None) => Ok(()),
        (Some(content), Some(artifact)) => validate_bound_artifact(control, artifact, &content),
        _ => Err(invalid(
            "ready predecessor content artifact presence disagrees",
        )),
    }
}

fn validate_bound_artifact(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
    content: &str,
) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
    artifact
        .validate()
        .map_err(|_| invalid("ready predecessor content artifact is invalid"))?;
    let receipt = control
        .direct_predecessor_receipts
        .first()
        .ok_or_else(|| invalid("ready predecessor content has no direct receipt"))?;
    let valid = artifact.artifact_kind == GroupAgentScheduledNodeTerminalArtifactKind::Result
        && artifact.classification == GroupAgentNodeTerminalClassification::Completed
        && artifact.output_text == content
        && artifact.graph_run_id == receipt.graph_run_id
        && artifact.node_id == receipt.node_id
        && artifact.attempt == receipt.attempt
        && artifact.dispatch_id == receipt.dispatch_id
        && artifact.provider_request_id == receipt.provider_request_id
        && artifact.project_lane_sha256 == receipt.project_lane_sha256
        && artifact.artifact_id == receipt.artifact_id
        && artifact.artifact_sha256 == receipt.artifact_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("ready predecessor content artifact closure disagrees"))
}

fn invalid(message: &str) -> GroupAgentScheduledReadyNodeDispatchReleaseValidationError {
    GroupAgentScheduledReadyNodeDispatchReleaseValidationError {
        message: message.into(),
    }
}
