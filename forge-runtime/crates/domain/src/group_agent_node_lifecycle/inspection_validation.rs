use super::{
    GROUP_AGENT_NODE_LIFECYCLE_VERSION, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeLifecycleValidationError, GroupAgentNodeTerminalArtifact,
    GroupAgentNodeTerminalReceipt, artifact_validation, terminal_validation,
};
use crate::{
    GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION, GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION,
    GroupAgentGraphRunStatus,
};

use super::validation::{invalid, validate_claim_event, validate_exact_json};

pub(super) fn validate_lifecycle_inspection(
    inspection: &GroupAgentNodeLifecycleInspection,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    if inspection.v != GROUP_AGENT_NODE_LIFECYCLE_VERSION {
        return Err(invalid("unsupported lifecycle inspection version"));
    }
    inspection
        .graph_run
        .validate()
        .map_err(|error| invalid(&error.message))?;
    inspection.claim.validate()?;
    validate_exact_json(&inspection.claim, &inspection.claim_json)?;
    let claim_event = inspection
        .graph_run
        .events
        .get(3)
        .ok_or_else(|| invalid("lifecycle inspection has no seq-4 claim event"))?;
    validate_claim_event(&inspection.claim, claim_event)?;
    validate_single_node_plan(inspection)?;
    match inspection.graph_run.run.status {
        GroupAgentGraphRunStatus::DispatchUnknown => validate_claimed(inspection),
        GroupAgentGraphRunStatus::Completed
        | GroupAgentGraphRunStatus::Failed
        | GroupAgentGraphRunStatus::FailedUncertain => validate_terminal(inspection),
        _ => Err(invalid(
            "lifecycle inspection has a non-lifecycle Run state",
        )),
    }
}

fn validate_claimed(
    inspection: &GroupAgentNodeLifecycleInspection,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let lane = inspection
        .active_lane
        .as_ref()
        .ok_or_else(|| invalid("claimed lifecycle has no active Project lane"))?;
    lane.validate_against_claim(&inspection.claim)?;
    let lane_json = inspection
        .active_lane_json
        .as_deref()
        .ok_or_else(|| invalid("claimed lifecycle has no canonical lane bytes"))?;
    validate_exact_json(lane, lane_json)?;
    let valid = inspection.graph_run.run.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION
        && inspection.artifact.is_none()
        && inspection.artifact_json.is_none()
        && inspection.terminal_receipt.is_none()
        && inspection.terminal_receipt_json.is_none();
    valid
        .then_some(())
        .ok_or_else(|| invalid("claimed lifecycle contains terminal evidence"))
}

fn validate_terminal(
    inspection: &GroupAgentNodeLifecycleInspection,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let artifact = inspection
        .artifact
        .as_ref()
        .ok_or_else(|| invalid("terminal lifecycle has no artifact"))?;
    let receipt = inspection
        .terminal_receipt
        .as_ref()
        .ok_or_else(|| invalid("terminal lifecycle has no Core receipt"))?;
    artifact_validation::validate_artifact_against_persisted_claim(artifact, &inspection.claim)?;
    terminal_validation::validate_receipt_against_terminal_evidence(
        receipt,
        &inspection.claim,
        artifact,
        &inspection.graph_run.run.graph_id,
    )?;
    validate_terminal_json(inspection, artifact, receipt)?;
    let event = inspection
        .graph_run
        .events
        .get(4)
        .ok_or_else(|| invalid("terminal lifecycle has no seq-5 event"))?;
    terminal_validation::validate_terminal_event(event, &inspection.claim, artifact, receipt)?;
    let valid = inspection.graph_run.run.v == GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION
        && inspection.graph_run.run.status == receipt.graph_status
        && inspection.active_lane.is_none()
        && inspection.active_lane_json.is_none();
    valid
        .then_some(())
        .ok_or_else(|| invalid("terminal lifecycle durable state disagrees"))
}

fn validate_single_node_plan(
    inspection: &GroupAgentNodeLifecycleInspection,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let node_id = inspection.claim.node_id.as_str();
    let plan = &inspection.graph_run.plan;
    let exact = plan.authored_node_ids.as_slice() == [node_id]
        && plan.edges.is_empty()
        && plan.waves.as_slice() == [vec![node_id.to_owned()]]
        && inspection.graph_run.run.node_count == 1
        && inspection.graph_run.run.wave_count == 1;
    exact
        .then_some(())
        .ok_or_else(|| invalid("lifecycle inspection is not an exact single-node topology"))
}

fn validate_terminal_json(
    inspection: &GroupAgentNodeLifecycleInspection,
    artifact: &GroupAgentNodeTerminalArtifact,
    receipt: &GroupAgentNodeTerminalReceipt,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let artifact_json = inspection
        .artifact_json
        .as_deref()
        .ok_or_else(|| invalid("terminal lifecycle has no canonical artifact bytes"))?;
    let receipt_json = inspection
        .terminal_receipt_json
        .as_deref()
        .ok_or_else(|| invalid("terminal lifecycle has no canonical receipt bytes"))?;
    validate_exact_json(artifact, artifact_json)?;
    validate_exact_json(receipt, receipt_json)
}
