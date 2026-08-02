use super::{
    GROUP_AGENT_NODE_LIFECYCLE_VERSION, GROUP_AGENT_NODE_TERMINAL_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_CONTROL_VERSION, GROUP_AGENT_NODE_TERMINAL_RECEIPT_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_RECEIPT_VERSION, GroupAgentNodeLifecycleValidationError,
    GroupAgentNodeTerminalArtifact, GroupAgentNodeTerminalArtifactKind,
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalControl,
    GroupAgentNodeTerminalOutcome, GroupAgentNodeTerminalReceipt,
    MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES, MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES,
    TerminalizeGroupAgentNodeDispatch, codec, group_agent_node_terminal_artifact_id,
    group_agent_node_terminal_receipt_id,
};
use crate::{
    GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION, GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchReleaseControl,
};

use super::validation::{
    invalid, is_digest, valid_identifier, validate_claim_against_sources, validate_exact_json,
    validate_single_node,
};

pub(super) fn validate_terminal_control(
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    validate_control_header(control)?;
    validate_run_inspection(control)?;
    validate_manifest_and_topology(control)?;
    let release = release_control(control)?;
    let claim_event = control
        .journal_events
        .get(3)
        .ok_or_else(|| invalid("terminal control has no seq-4 claim event"))?;
    validate_claim_against_sources(
        &control.claim,
        claim_event,
        &release,
        &control.authorization,
        &control.pricing,
    )?;
    control.active_lane.validate_against_claim(&control.claim)?;
    control.artifact.validate_against_claim(
        &control.claim,
        &control.active_lane,
        &control.authorization,
        &control.pricing,
        &control.contract,
    )?;
    validate_control_identity(control)
}

fn validate_control_header(
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let run = &control.graph_run;
    let valid = control.v == GROUP_AGENT_NODE_TERMINAL_CONTROL_VERSION
        && control.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && control.terminal_control_protocol_version
            == GROUP_AGENT_NODE_TERMINAL_CONTROL_PROTOCOL_VERSION
        && run.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION
        && run.status == GroupAgentGraphRunStatus::DispatchUnknown
        && run.execution_contract_present
        && run.dispatch_request_present
        && run.dispatch_authority_released
        && run.last_event_seq == 4
        && control.journal_events.len() == 4
        && is_digest(&control.snapshot_sha256);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Node terminal control header"))
}

fn validate_run_inspection(
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let plan_json = control
        .plan
        .canonical_json()
        .map_err(|error| invalid(&error.message))?;
    let event_jsons = canonical_event_jsons(&control.journal_events)?;
    GroupAgentGraphRunInspection {
        v: control.graph_run.v,
        run: control.graph_run.clone(),
        plan_json,
        plan: control.plan.clone(),
        event_jsons,
        events: control.journal_events.clone(),
    }
    .validate()
    .map_err(|error| invalid(&error.message))
}

fn validate_manifest_and_topology(
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    control
        .manifest
        .validate()
        .map_err(|error| invalid(&error.message))?;
    let authored = control
        .manifest
        .nodes
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<Vec<_>>();
    let planned = control
        .plan
        .authored_node_ids
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let exact = control.manifest.expected_sha256().as_deref()
        == Ok(control.graph_run.graph_manifest_sha256.as_str())
        && control.manifest.source.snapshot_sha256 == control.graph_run.source_snapshot_sha256
        && authored == planned
        && control.manifest.edges == control.plan.edges
        && control.manifest.waves == control.plan.waves;
    if !exact {
        return Err(invalid("terminal control manifest and Graph Run disagree"));
    }
    let release = release_control(control)?;
    validate_single_node(&release)
}

fn release_control(
    control: &GroupAgentNodeTerminalControl,
) -> Result<GroupAgentNodeDispatchReleaseControl, GroupAgentNodeLifecycleValidationError> {
    let events = control.journal_events[..3].to_vec();
    let event_jsons = canonical_event_jsons(&events)?;
    let mut run = control.graph_run.clone();
    run.v = GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION;
    run.status = GroupAgentGraphRunStatus::AwaitingDispatchAuthorization;
    run.dispatch_authority_released = false;
    run.last_event_seq = 3;
    run.journal_bytes = event_jsons.iter().map(String::len).sum();
    Ok(GroupAgentNodeDispatchReleaseControl {
        v: GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        release_control_protocol_version:
            GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: run,
        plan: control.plan.clone(),
        manifest: control.manifest.clone(),
        journal_events: events,
        contract_record: control.contract_record.clone(),
        contract: control.contract.clone(),
        dispatch_request: control.dispatch_request.clone(),
        provider_request_json: control.provider_request_json.clone(),
        snapshot_sha256: control
            .authorization
            .release_control_snapshot_sha256
            .clone(),
    })
}

fn validate_control_identity(
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let bytes = control.canonical_json()?.len();
    let valid = control.expected_sha256()? == control.snapshot_sha256
        && (1..=MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES).contains(&bytes);
    valid
        .then_some(())
        .ok_or_else(|| invalid("terminal control content identity disagrees"))
}

pub(super) fn validate_terminal_receipt(
    receipt: &GroupAgentNodeTerminalReceipt,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let valid = receipt.v == GROUP_AGENT_NODE_TERMINAL_RECEIPT_VERSION
        && receipt.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && receipt.terminal_receipt_protocol_version
            == GROUP_AGENT_NODE_TERMINAL_RECEIPT_PROTOCOL_VERSION
        && is_digest(&receipt.terminal_control_sha256)
        && receipt.expected_last_event_seq == 4
        && is_digest(&receipt.expected_last_event_sha256)
        && valid_identifier(&receipt.graph_run_id)
        && valid_identifier(&receipt.graph_id)
        && valid_identifier(&receipt.node_id)
        && receipt.attempt == 1
        && valid_identifier(&receipt.dispatch_id)
        && valid_identifier(&receipt.lane_ownership_id)
        && is_digest(&receipt.project_lane_sha256)
        && receipt.artifact_id == group_agent_node_terminal_artifact_id(&receipt.artifact_sha256)
        && is_digest(&receipt.artifact_sha256)
        && receipt.wave_index == 0
        && valid_outcomes(receipt)
        && !receipt.retry_authorized
        && receipt.lane_release_authorized;
    if !valid {
        return Err(invalid("invalid Group Agent Node Core terminal receipt"));
    }
    validate_receipt_identity(receipt)
}

fn valid_outcomes(receipt: &GroupAgentNodeTerminalReceipt) -> bool {
    receipt.node_outcome == receipt.wave_outcome
        && matches!(
            (
                receipt.artifact_kind,
                receipt.node_outcome,
                receipt.graph_status,
            ),
            (
                GroupAgentNodeTerminalArtifactKind::Result,
                GroupAgentNodeTerminalOutcome::Completed,
                GroupAgentGraphRunStatus::Completed,
            ) | (
                GroupAgentNodeTerminalArtifactKind::Result,
                GroupAgentNodeTerminalOutcome::Failed,
                GroupAgentGraphRunStatus::Failed,
            ) | (
                GroupAgentNodeTerminalArtifactKind::Uncertainty,
                GroupAgentNodeTerminalOutcome::FailedUncertain,
                GroupAgentGraphRunStatus::FailedUncertain,
            )
        )
}

fn validate_receipt_identity(
    receipt: &GroupAgentNodeTerminalReceipt,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let digest = codec::receipt_digest(receipt)?;
    let valid = receipt.receipt_sha256 == digest
        && receipt.receipt_id == group_agent_node_terminal_receipt_id(&digest)
        && receipt.canonical_json()?.len() <= MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES;
    valid
        .then_some(())
        .ok_or_else(|| invalid("Core terminal receipt content identity disagrees"))
}

pub(super) fn validate_receipt_against_control(
    receipt: &GroupAgentNodeTerminalReceipt,
    control: &GroupAgentNodeTerminalControl,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    control.validate()?;
    validate_receipt_against_terminal_evidence(
        receipt,
        &control.claim,
        &control.artifact,
        &control.graph_run.graph_id,
    )?;
    let claim_event = control
        .journal_events
        .get(3)
        .ok_or_else(|| invalid("terminal control has no claim head"))?;
    let claim_head = claim_event
        .expected_sha256()
        .map_err(|error| invalid(&error.message))?;
    let exact = receipt.terminal_control_sha256 == control.snapshot_sha256
        && receipt.expected_last_event_sha256 == claim_head;
    exact
        .then_some(())
        .ok_or_else(|| invalid("Core terminal receipt disagrees with terminal control"))
}

pub(super) fn validate_receipt_against_terminal_evidence(
    receipt: &GroupAgentNodeTerminalReceipt,
    claim: &super::GroupAgentNodeDispatchClaim,
    artifact: &GroupAgentNodeTerminalArtifact,
    graph_id: &str,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    receipt.validate()?;
    super::artifact_validation::validate_artifact_against_persisted_claim(artifact, claim)?;
    let exact = receipt.expected_last_event_sha256 == claim.claim_event_sha256
        && receipt.graph_run_id == claim.graph_run_id
        && receipt.graph_id == graph_id
        && receipt.node_id == claim.node_id
        && receipt.attempt == claim.attempt
        && receipt.dispatch_id == claim.dispatch_id
        && receipt.lane_ownership_id == claim.lane_ownership_id
        && receipt.project_lane_sha256 == claim.project_lane_sha256
        && receipt.artifact_kind == artifact.artifact_kind
        && receipt.artifact_id == artifact.artifact_id
        && receipt.artifact_sha256 == artifact.artifact_sha256
        && receipt_outcome_matches_artifact(receipt, artifact);
    exact
        .then_some(())
        .ok_or_else(|| invalid("Core terminal receipt disagrees with terminal evidence"))
}

fn receipt_outcome_matches_artifact(
    receipt: &GroupAgentNodeTerminalReceipt,
    artifact: &GroupAgentNodeTerminalArtifact,
) -> bool {
    let expected = match (artifact.artifact_kind, artifact.classification) {
        (
            GroupAgentNodeTerminalArtifactKind::Result,
            GroupAgentNodeTerminalClassification::Completed,
        ) => GroupAgentNodeTerminalOutcome::Completed,
        (
            GroupAgentNodeTerminalArtifactKind::Result,
            GroupAgentNodeTerminalClassification::Length,
        ) => GroupAgentNodeTerminalOutcome::Failed,
        (GroupAgentNodeTerminalArtifactKind::Uncertainty, _) => {
            GroupAgentNodeTerminalOutcome::FailedUncertain
        }
        _ => return false,
    };
    receipt.node_outcome == expected
}

pub(super) fn validate_terminalize_request(
    request: &TerminalizeGroupAgentNodeDispatch,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    if request.v != GROUP_AGENT_NODE_LIFECYCLE_VERSION
        || request.terminalized_at_ms > i64::MAX as u64
        || request.terminalized_at_ms < request.control.artifact.created_at_ms
    {
        return Err(invalid(
            "invalid Group Agent Node terminalization request header",
        ));
    }
    request.control.validate()?;
    validate_exact_json(&request.control, &request.control_json)?;
    validate_exact_json(&request.control.artifact, &request.artifact_json)?;
    request.receipt.validate_against_control(&request.control)?;
    validate_exact_json(&request.receipt, &request.receipt_json)?;
    validate_exact_json(&request.event, &request.event_json)?;
    validate_terminal_event(
        &request.event,
        &request.control.claim,
        &request.control.artifact,
        &request.receipt,
    )?;
    let event_time = terminalized_at_ms(&request.event)?;
    (event_time == request.terminalized_at_ms)
        .then_some(())
        .ok_or_else(|| invalid("terminalization time disagrees with seq-5 event"))
}

pub(super) fn validate_terminal_event(
    event: &GroupAgentGraphRunEvent,
    claim: &super::GroupAgentNodeDispatchClaim,
    artifact: &GroupAgentNodeTerminalArtifact,
    receipt: &GroupAgentNodeTerminalReceipt,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    event.validate().map_err(|error| invalid(&error.message))?;
    let envelope = event.v == GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION
        && event.graph_run_id == claim.graph_run_id
        && event.seq == 5;
    (envelope && terminal_event_fields_match(&event.kind, claim, artifact, receipt))
        .then_some(())
        .ok_or_else(|| invalid("seq-5 event disagrees with terminal evidence"))
}

fn terminal_event_fields_match(
    kind: &GroupAgentGraphRunEventKind,
    claim: &super::GroupAgentNodeDispatchClaim,
    artifact: &GroupAgentNodeTerminalArtifact,
    receipt: &GroupAgentNodeTerminalReceipt,
) -> bool {
    let GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
        previous_event_sha256,
        dispatch_id,
        lane_ownership_id,
        project_lane_sha256,
        artifact_id,
        artifact_sha256,
        terminal_receipt_id,
        terminal_receipt_sha256,
        node_id,
        attempt,
        node_outcome,
        wave_index,
        wave_outcome,
        graph_status,
        retry_authorized,
        lane_released,
        terminalized_at_ms,
    } = kind
    else {
        return false;
    };
    previous_event_sha256 == &claim.claim_event_sha256
        && dispatch_id == &claim.dispatch_id
        && lane_ownership_id == &claim.lane_ownership_id
        && project_lane_sha256 == &claim.project_lane_sha256
        && artifact_id == &artifact.artifact_id
        && artifact_sha256 == &artifact.artifact_sha256
        && terminal_receipt_id == &receipt.receipt_id
        && terminal_receipt_sha256 == &receipt.receipt_sha256
        && node_id == &claim.node_id
        && attempt == &claim.attempt
        && node_outcome == &receipt.node_outcome
        && wave_index == &receipt.wave_index
        && wave_outcome == &receipt.wave_outcome
        && graph_status == &receipt.graph_status
        && retry_authorized == &receipt.retry_authorized
        && lane_released == &receipt.lane_release_authorized
        && terminalized_at_ms >= &artifact.created_at_ms
}

fn terminalized_at_ms(
    event: &GroupAgentGraphRunEvent,
) -> Result<u64, GroupAgentNodeLifecycleValidationError> {
    let GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
        terminalized_at_ms, ..
    } = &event.kind
    else {
        return Err(invalid("terminalization event kind is invalid"));
    };
    Ok(*terminalized_at_ms)
}

fn canonical_event_jsons(
    events: &[GroupAgentGraphRunEvent],
) -> Result<Vec<String>, GroupAgentNodeLifecycleValidationError> {
    events
        .iter()
        .map(|event| {
            event
                .canonical_json()
                .map_err(|error| invalid(&error.message))
        })
        .collect()
}
