use super::super::{
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
    GroupAgentScheduledNodeLifecycleValidationError, GroupAgentScheduledNodeTerminalArtifact,
    GroupAgentScheduledNodeTerminalArtifactKind, GroupAgentScheduledNodeTerminalControl,
    GroupAgentScheduledNodeTerminalReceipt, MAX_GROUP_AGENT_SCHEDULED_NODE_ARTIFACT_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTROL_BYTES, MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES,
};
use crate::{GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome};

#[allow(clippy::too_many_lines)]
pub(crate) fn validate_artifact(
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
    let expected = artifact.expected_sha256()?;
    let canonical_len = artifact.canonical_json()?.len();
    let result_class = matches!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::Completed
    );
    let kind_valid = (artifact.artifact_kind
        == GroupAgentScheduledNodeTerminalArtifactKind::Result)
        == result_class;
    let valid = artifact_structure_valid(artifact, &expected, canonical_len)
        && kind_valid
        && (artifact_result_evidence(artifact) || artifact_uncertainty_evidence(artifact));
    valid
        .then_some(())
        .ok_or_else(|| super::invalid("invalid scheduled terminal artifact"))
}

/// Structural field checks: versions, identifiers, digests, byte accounting,
/// content-addressed IDs, and the canonical JSON round trip.
fn artifact_structure_valid(
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
    expected: &str,
    canonical_len: usize,
) -> bool {
    artifact.v == GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION
        && artifact.terminal_artifact_protocol_version
            == GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION
        && super::identifier(&artifact.graph_run_id)
        && super::identifier(&artifact.node_id)
        && artifact.attempt == 1
        && super::identifier(&artifact.dispatch_id)
        && super::identifier(&artifact.provider_request_id)
        && super::digest(&artifact.claim_event_sha256)
        && super::digest(&artifact.authorization_sha256)
        && super::digest(&artifact.provider_request_sha256)
        && super::digest(&artifact.request_body_sha256)
        && super::digest(&artifact.pricing_snapshot_sha256)
        && super::identifier(&artifact.lane_ownership_id)
        && super::digest(&artifact.project_lane_sha256)
        && (!artifact.output_text.is_empty()
            || artifact.classification != GroupAgentNodeTerminalClassification::Completed)
        && artifact.output_bytes == artifact.output_text.len()
        && artifact.output_bytes <= MAX_GROUP_AGENT_SCHEDULED_NODE_ARTIFACT_BYTES
        && artifact.output_sha256
            == super::super::group_agent_scheduled_node_terminal_output_sha256(
                &artifact.output_text,
            )
        && !artifact.retry_authorized
        && artifact.artifact_id
            == super::super::group_agent_scheduled_node_terminal_artifact_id(expected)
        && artifact.artifact_sha256 == expected
        && (1..=MAX_GROUP_AGENT_SCHEDULED_NODE_ARTIFACT_BYTES).contains(&artifact.artifact_bytes)
        && artifact.artifact_bytes == canonical_len
        && canonical_len <= MAX_GROUP_AGENT_SCHEDULED_NODE_ARTIFACT_BYTES
}

/// Usage bookkeeping is valid when observed usage is positive in both token
/// dimensions or, when unobserved, both dimensions are exactly zero.
fn artifact_usage_valid(artifact: &GroupAgentScheduledNodeTerminalArtifact) -> bool {
    if artifact.usage_observed {
        artifact.input_tokens >= 1 && artifact.output_tokens >= 1
    } else {
        artifact.input_tokens == 0 && artifact.output_tokens == 0
    }
}

/// A completed result must be fully observed: poll started, terminal seen,
/// true EOF, positive usage, cost never calculated, and non-empty output.
fn artifact_result_evidence(artifact: &GroupAgentScheduledNodeTerminalArtifact) -> bool {
    let result_class = matches!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::Completed
    );
    result_class
        && artifact.provider_poll_started
        && artifact.terminal_seen
        && artifact.stream_eof_seen
        && artifact_usage_valid(artifact)
        && artifact.usage_observed
        && !artifact.actual_cost_calculated
        && artifact.actual_cost_usd_micros == 0
        && (artifact.classification != GroupAgentNodeTerminalClassification::Completed
            || !artifact.output_text.is_empty())
}

/// Uncertainty evidence: either the poll never started (no terminal/EOF
/// observed), or the uncertainty is a bounded `Length` outcome with the full
/// usage/cost evidence, or a `MissingUsage` outcome with a true EOF.
fn artifact_uncertainty_evidence(artifact: &GroupAgentScheduledNodeTerminalArtifact) -> bool {
    let uncertainty_class = matches!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::Length
            | GroupAgentNodeTerminalClassification::ProviderError
            | GroupAgentNodeTerminalClassification::HttpError
            | GroupAgentNodeTerminalClassification::TransportError
            | GroupAgentNodeTerminalClassification::Timeout
            | GroupAgentNodeTerminalClassification::Cancelled
            | GroupAgentNodeTerminalClassification::EofBeforeTerminal
            | GroupAgentNodeTerminalClassification::MissingUsage
            | GroupAgentNodeTerminalClassification::ToolCall
            | GroupAgentNodeTerminalClassification::ProtocolError
            | GroupAgentNodeTerminalClassification::TrailingData
            | GroupAgentNodeTerminalClassification::LocalLimit
    );
    let usage_valid = artifact_usage_valid(artifact);
    let cost_valid = !artifact.actual_cost_calculated && artifact.actual_cost_usd_micros == 0;
    uncertainty_class
        && (artifact.provider_poll_started
            || (!artifact.terminal_seen && !artifact.stream_eof_seen))
        && (artifact.classification != GroupAgentNodeTerminalClassification::Length
            || (artifact.terminal_seen
                && artifact.stream_eof_seen
                && artifact.usage_observed
                && usage_valid
                && cost_valid))
        && (artifact.classification == GroupAgentNodeTerminalClassification::Length
            || artifact.classification != GroupAgentNodeTerminalClassification::MissingUsage
            || (artifact.terminal_seen
                && artifact.stream_eof_seen
                && !artifact.usage_observed
                && cost_valid))
}

pub(crate) fn validate_control(
    control: &GroupAgentScheduledNodeTerminalControl,
) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
    control.artifact.validate()?;
    let expected = control.expected_sha256()?;
    let valid = control.v == super::super::GROUP_AGENT_SCHEDULED_NODE_TERMINAL_CONTROL_VERSION
        && control.scheduler_protocol_version
            == crate::GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && control.terminal_control_protocol_version
            == GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION
        && super::digest(&control.release_control_snapshot_sha256)
        && super::identifier(&control.graph_run_id)
        && super::identifier(&control.graph_id)
        && super::identifier(&control.node_id)
        && control.attempt == 1
        && control.dispatch_id == control.artifact.dispatch_id
        && control.provider_request_id == control.artifact.provider_request_id
        && control.authorization_sha256 == control.artifact.authorization_sha256
        && control.provider_request_sha256 == control.artifact.provider_request_sha256
        && control.request_body_sha256 == control.artifact.request_body_sha256
        && control.expected_last_event_seq == 1
        && super::digest(&control.expected_last_event_sha256)
        && control.claim_event_sha256 == control.artifact.claim_event_sha256
        && control.project_lane_sha256 == control.artifact.project_lane_sha256
        && control.snapshot_sha256 == expected;
    if !valid {
        return Err(super::invalid("invalid scheduled terminal control"));
    }
    (control.canonical_json()?.len() <= MAX_GROUP_AGENT_SCHEDULED_NODE_CONTROL_BYTES)
        .then_some(())
        .ok_or_else(|| super::invalid("scheduled terminal control exceeds its byte bound"))
}

pub(crate) fn decode_exact_control(
    bytes: &[u8],
) -> Result<GroupAgentScheduledNodeTerminalControl, GroupAgentScheduledNodeLifecycleValidationError>
{
    if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTROL_BYTES).contains(&bytes.len()) {
        return Err(super::invalid(
            "scheduled terminal control exceeds its byte bound",
        ));
    }
    let value: GroupAgentScheduledNodeTerminalControl = serde_json::from_slice(bytes)
        .map_err(|_| super::invalid("scheduled terminal control is invalid"))?;
    let canonical = value.canonical_json()?;
    if canonical.as_bytes() != bytes {
        return Err(super::invalid(
            "scheduled terminal control is not canonical",
        ));
    }
    value.validate()?;
    Ok(value)
}

pub(crate) fn validate_receipt(
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
    let expected = receipt.expected_sha256()?;
    let completed = receipt.node_outcome == GroupAgentNodeTerminalOutcome::Completed;
    let valid = receipt.v == GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION
        && receipt.scheduler_protocol_version
            == crate::GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && receipt.terminal_receipt_protocol_version
            == super::super::GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION
        && super::digest(&receipt.terminal_control_sha256)
        && super::identifier(&receipt.graph_run_id)
        && super::identifier(&receipt.graph_id)
        && super::identifier(&receipt.node_id)
        && receipt.attempt == 1
        && super::identifier(&receipt.dispatch_id)
        && super::identifier(&receipt.provider_request_id)
        && super::digest(&receipt.project_lane_sha256)
        && receipt.artifact_kind
            == if completed {
                GroupAgentScheduledNodeTerminalArtifactKind::Result
            } else {
                GroupAgentScheduledNodeTerminalArtifactKind::Uncertainty
            }
        && super::identifier(&receipt.artifact_id)
        && super::digest(&receipt.artifact_sha256)
        && !receipt.retry_authorized
        && receipt.lane_release_authorized
        && !receipt.successor_advance_authorized
        && receipt.receipt_id
            == super::super::group_agent_scheduled_node_terminal_receipt_id(&expected)
        && receipt.receipt_sha256 == expected;
    if !valid {
        return Err(super::invalid("invalid scheduled terminal receipt"));
    }
    (receipt.canonical_json()?.len() <= MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES)
        .then_some(())
        .ok_or_else(|| super::invalid("scheduled terminal receipt exceeds its byte bound"))
}

pub(crate) fn decode_exact_receipt(
    bytes: &[u8],
) -> Result<GroupAgentScheduledNodeTerminalReceipt, GroupAgentScheduledNodeLifecycleValidationError>
{
    if !(1..=MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES).contains(&bytes.len()) {
        return Err(super::invalid(
            "scheduled terminal receipt exceeds its byte bound",
        ));
    }
    let value: GroupAgentScheduledNodeTerminalReceipt = serde_json::from_slice(bytes)
        .map_err(|_| super::invalid("scheduled terminal receipt is invalid"))?;
    let canonical = value.canonical_json()?;
    if canonical.as_bytes() != bytes {
        return Err(super::invalid(
            "scheduled terminal receipt is not canonical",
        ));
    }
    value.validate()?;
    Ok(value)
}

pub(crate) fn validate_receipt_against_control(
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
    control: &GroupAgentScheduledNodeTerminalControl,
) -> Result<(), GroupAgentScheduledNodeLifecycleValidationError> {
    receipt.validate()?;
    let valid = receipt.terminal_control_sha256 == control.snapshot_sha256
        && receipt.graph_run_id == control.graph_run_id
        && receipt.graph_id == control.graph_id
        && receipt.node_id == control.node_id
        && receipt.attempt == control.attempt
        && receipt.dispatch_id == control.dispatch_id
        && receipt.provider_request_id == control.provider_request_id
        && receipt.project_lane_sha256 == control.project_lane_sha256
        && receipt.artifact_kind == control.artifact.artifact_kind
        && (receipt.node_outcome == GroupAgentNodeTerminalOutcome::Completed)
            == (control.artifact.classification == GroupAgentNodeTerminalClassification::Completed)
        && receipt.artifact_id == control.artifact.artifact_id
        && receipt.artifact_sha256 == control.artifact.artifact_sha256;
    valid
        .then_some(())
        .ok_or_else(|| super::invalid("scheduled receipt disagrees with control"))
}

#[cfg(test)]
mod tests {
    use super::super::GroupAgentScheduledNodeTerminalArtifact;
    use super::*;
    use crate::GroupAgentScheduledNodeTerminalArtifactKind;

    /// The all-false no-evidence shape the v4 family accepts for a hard crash
    /// (`provider_poll_started ∨ (¬terminal_seen ∧ ¬stream_eof_seen)`).
    fn no_evidence_artifact(
        class: GroupAgentNodeTerminalClassification,
    ) -> GroupAgentScheduledNodeTerminalArtifact {
        GroupAgentScheduledNodeTerminalArtifact {
            v: 1,
            terminal_artifact_protocol_version: 1,
            artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Uncertainty,
            graph_run_id: String::new(),
            node_id: String::new(),
            attempt: 1,
            dispatch_id: String::new(),
            provider_request_id: String::new(),
            claim_event_sha256: String::new(),
            authorization_sha256: String::new(),
            provider_request_sha256: String::new(),
            request_body_sha256: String::new(),
            pricing_snapshot_sha256: String::new(),
            lane_ownership_id: String::new(),
            project_lane_sha256: String::new(),
            provider_poll_started: false,
            terminal_seen: false,
            stream_eof_seen: false,
            classification: class,
            output_text: String::new(),
            output_bytes: 0,
            output_sha256: String::new(),
            usage_observed: false,
            input_tokens: 0,
            output_tokens: 0,
            actual_cost_calculated: false,
            actual_cost_usd_micros: 0,
            retry_authorized: false,
            created_at_ms: 0,
            artifact_id: String::new(),
            artifact_bytes: 0,
            artifact_sha256: String::new(),
        }
    }

    /// ADR-0034's pid-sidecar adjudication owns the scheduled family; its
    /// closed-world uncertainty list must keep rejecting `HardCrash` even in
    /// the all-false no-evidence shape the v4 family accepts (A-scheduled-fence).
    #[test]
    fn hard_crash_is_not_a_scheduled_family_uncertainty_class() {
        let hard_crash = no_evidence_artifact(GroupAgentNodeTerminalClassification::HardCrash);
        assert!(!artifact_uncertainty_evidence(&hard_crash));
        assert!(!artifact_result_evidence(&hard_crash));
        assert!(validate_artifact(&hard_crash).is_err());

        let provider_error =
            no_evidence_artifact(GroupAgentNodeTerminalClassification::ProviderError);
        assert!(
            artifact_uncertainty_evidence(&provider_error),
            "the no-evidence shape must stay valid for scheduled uncertainty classes"
        );
    }
}
