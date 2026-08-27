use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_CONTROL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION, GroupAgentNodeTerminalClassification,
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchReleaseControl, GroupAgentScheduledNodeTerminalArtifact,
    GroupAgentScheduledNodeTerminalArtifactKind, GroupAgentScheduledNodeTerminalControl,
    GroupAgentScheduledNodeTerminalReceipt, TerminalizeGroupAgentScheduledNodeDispatch,
    group_agent_scheduled_node_terminal_artifact_id,
    group_agent_scheduled_node_terminal_output_sha256,
    group_agent_scheduled_node_terminal_receipt_id,
};

const TERMINALIZED_AT_MS: u64 = 80;

pub(super) fn terminal_request(
    release: &GroupAgentScheduledNodeDispatchReleaseControl,
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> TerminalizeGroupAgentScheduledNodeDispatch {
    let artifact = completed_artifact(claim);
    let artifact_json = artifact.canonical_json().expect("terminal artifact JSON");
    let control = terminal_control(release, claim, artifact);
    let control_json = control.canonical_json().expect("terminal control JSON");
    let receipt = terminal_receipt(&control);
    let receipt_json = receipt.canonical_json().expect("terminal receipt JSON");
    TerminalizeGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        control: Some(control),
        control_json: Some(control_json),
        artifact_json,
        receipt: Some(receipt),
        receipt_json: Some(receipt_json),
        terminalized_at_ms: TERMINALIZED_AT_MS,
    }
}

fn completed_artifact(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeTerminalArtifact {
    let output = "done".to_owned();
    let mut artifact = GroupAgentScheduledNodeTerminalArtifact {
        v: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION,
        terminal_artifact_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Result,
        graph_run_id: claim.graph_run_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        provider_request_sha256: claim.provider_request_sha256.clone(),
        request_body_sha256: claim.request_body_sha256.clone(),
        pricing_snapshot_sha256: claim.pricing_snapshot_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        provider_poll_started: true,
        terminal_seen: true,
        stream_eof_seen: true,
        classification: GroupAgentNodeTerminalClassification::Completed,
        output_bytes: output.len(),
        output_sha256: group_agent_scheduled_node_terminal_output_sha256(&output),
        output_text: output,
        usage_observed: true,
        input_tokens: 1,
        output_tokens: 1,
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms: TERMINALIZED_AT_MS,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    };
    seal_artifact(&mut artifact);
    artifact
}

fn seal_artifact(artifact: &mut GroupAgentScheduledNodeTerminalArtifact) {
    let sha256 = artifact
        .expected_sha256()
        .expect("terminal artifact digest");
    artifact.artifact_id = group_agent_scheduled_node_terminal_artifact_id(&sha256);
    artifact.artifact_sha256 = sha256;
    loop {
        let bytes = artifact
            .canonical_json()
            .expect("terminal artifact JSON")
            .len();
        if artifact.artifact_bytes == bytes {
            break;
        }
        artifact.artifact_bytes = bytes;
    }
    artifact.validate().expect("valid terminal artifact");
}

fn terminal_control(
    release: &GroupAgentScheduledNodeDispatchReleaseControl,
    claim: &GroupAgentScheduledNodeDispatchClaim,
    artifact: GroupAgentScheduledNodeTerminalArtifact,
) -> GroupAgentScheduledNodeTerminalControl {
    let mut control = GroupAgentScheduledNodeTerminalControl {
        v: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        terminal_control_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        release_control_snapshot_sha256: release.snapshot_sha256.clone(),
        graph_run_id: claim.graph_run_id.clone(),
        graph_id: release.graph_run.graph_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        provider_request_sha256: claim.provider_request_sha256.clone(),
        request_body_sha256: claim.request_body_sha256.clone(),
        expected_last_event_seq: claim.expected_last_event_seq,
        expected_last_event_sha256: claim.expected_last_event_sha256.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        artifact,
        snapshot_sha256: String::new(),
    };
    control.snapshot_sha256 = control.expected_sha256().expect("terminal control digest");
    control.validate().expect("valid terminal control");
    control
}

fn terminal_receipt(
    control: &GroupAgentScheduledNodeTerminalControl,
) -> GroupAgentScheduledNodeTerminalReceipt {
    let artifact = &control.artifact;
    let mut receipt = GroupAgentScheduledNodeTerminalReceipt {
        v: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        terminal_receipt_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        terminal_control_sha256: control.snapshot_sha256.clone(),
        graph_run_id: control.graph_run_id.clone(),
        graph_id: control.graph_id.clone(),
        node_id: control.node_id.clone(),
        attempt: control.attempt,
        dispatch_id: control.dispatch_id.clone(),
        provider_request_id: control.provider_request_id.clone(),
        project_lane_sha256: control.project_lane_sha256.clone(),
        artifact_kind: artifact.artifact_kind,
        artifact_id: artifact.artifact_id.clone(),
        artifact_sha256: artifact.artifact_sha256.clone(),
        node_outcome: GroupAgentNodeTerminalOutcome::Completed,
        retry_authorized: false,
        lane_release_authorized: true,
        successor_advance_authorized: false,
        receipt_id: String::new(),
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = receipt.expected_sha256().expect("terminal receipt digest");
    receipt.receipt_id = group_agent_scheduled_node_terminal_receipt_id(&receipt.receipt_sha256);
    receipt
        .validate_against_control(control)
        .expect("valid terminal receipt");
    receipt
}
