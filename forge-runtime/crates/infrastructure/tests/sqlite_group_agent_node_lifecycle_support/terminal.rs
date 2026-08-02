use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GROUP_AGENT_NODE_LIFECYCLE_VERSION,
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION, GROUP_AGENT_NODE_TERMINAL_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_CONTROL_VERSION, GROUP_AGENT_NODE_TERMINAL_RECEIPT_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_RECEIPT_VERSION, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphRunStatus, GroupAgentNodeDispatchClaim,
    GroupAgentNodeLifecycleInspection, GroupAgentNodeTerminalArtifact,
    GroupAgentNodeTerminalArtifactKind, GroupAgentNodeTerminalClassification,
    GroupAgentNodeTerminalControl, GroupAgentNodeTerminalOutcome, GroupAgentNodeTerminalReceipt,
    TerminalizeGroupAgentNodeDispatch, group_agent_node_terminal_artifact_id,
    group_agent_node_terminal_output_sha256, group_agent_node_terminal_receipt_id,
};

use super::ClaimFixture;

pub fn terminal_request(
    source: &ClaimFixture,
    lifecycle: &GroupAgentNodeLifecycleInspection,
) -> TerminalizeGroupAgentNodeDispatch {
    let artifact = uncertainty_artifact(&lifecycle.claim);
    let control = terminal_control(source, lifecycle, artifact);
    let receipt = terminal_receipt(&control);
    let event = terminal_event(&control, &receipt);
    let request = TerminalizeGroupAgentNodeDispatch {
        v: GROUP_AGENT_NODE_LIFECYCLE_VERSION,
        control_json: control.canonical_json().unwrap(),
        artifact_json: control.artifact.canonical_json().unwrap(),
        receipt_json: receipt.canonical_json().unwrap(),
        event_json: event.canonical_json().unwrap(),
        control,
        receipt,
        event,
        terminalized_at_ms: 120,
    };
    request.validate().expect("valid terminal request");
    request
}

fn uncertainty_artifact(claim: &GroupAgentNodeDispatchClaim) -> GroupAgentNodeTerminalArtifact {
    let output = String::new();
    let mut artifact = GroupAgentNodeTerminalArtifact {
        v: GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION,
        terminal_artifact_protocol_version: GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION,
        artifact_kind: GroupAgentNodeTerminalArtifactKind::Uncertainty,
        graph_run_id: claim.graph_run_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        dispatch_request_sha256: claim.dispatch_request_sha256.clone(),
        logical_request_sha256: claim.logical_request_sha256.clone(),
        request_body_sha256: claim.request_body_sha256.clone(),
        pricing_snapshot_sha256: claim.pricing_snapshot_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        provider_poll_started: false,
        terminal_seen: false,
        stream_eof_seen: false,
        classification: GroupAgentNodeTerminalClassification::ProviderError,
        output_bytes: output.len(),
        output_sha256: group_agent_node_terminal_output_sha256(&output),
        output_text: output,
        usage_observed: false,
        input_tokens: 0,
        output_tokens: 0,
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms: 110,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    };
    artifact.artifact_bytes = artifact.canonical_payload_json().unwrap().len();
    artifact.artifact_sha256 = artifact.expected_sha256().unwrap();
    artifact.artifact_id = group_agent_node_terminal_artifact_id(&artifact.artifact_sha256);
    artifact.validate().expect("valid uncertainty artifact");
    artifact
}

fn terminal_control(
    source: &ClaimFixture,
    lifecycle: &GroupAgentNodeLifecycleInspection,
    artifact: GroupAgentNodeTerminalArtifact,
) -> GroupAgentNodeTerminalControl {
    let release = &source.release;
    let mut control = GroupAgentNodeTerminalControl {
        v: GROUP_AGENT_NODE_TERMINAL_CONTROL_VERSION,
        scheduler_protocol_version: release.scheduler_protocol_version,
        terminal_control_protocol_version: GROUP_AGENT_NODE_TERMINAL_CONTROL_PROTOCOL_VERSION,
        graph_run: lifecycle.graph_run.run.clone(),
        plan: release.plan.clone(),
        manifest: release.manifest.clone(),
        journal_events: lifecycle.graph_run.events.clone(),
        contract_record: release.contract_record.clone(),
        contract: release.contract.clone(),
        dispatch_request: release.dispatch_request.clone(),
        provider_request_json: release.provider_request_json.clone(),
        authorization: source.authorization.clone(),
        pricing: source.pricing.clone(),
        active_lane: lifecycle.active_lane.clone().unwrap(),
        claim: lifecycle.claim.clone(),
        artifact,
        snapshot_sha256: String::new(),
    };
    control.snapshot_sha256 = control.expected_sha256().unwrap();
    control.validate().expect("valid terminal control");
    control
}

fn terminal_receipt(control: &GroupAgentNodeTerminalControl) -> GroupAgentNodeTerminalReceipt {
    let mut receipt = GroupAgentNodeTerminalReceipt {
        v: GROUP_AGENT_NODE_TERMINAL_RECEIPT_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        terminal_receipt_protocol_version: GROUP_AGENT_NODE_TERMINAL_RECEIPT_PROTOCOL_VERSION,
        terminal_control_sha256: control.snapshot_sha256.clone(),
        expected_last_event_seq: 4,
        expected_last_event_sha256: control.claim.claim_event_sha256.clone(),
        graph_run_id: control.graph_run.graph_run_id.clone(),
        graph_id: control.graph_run.graph_id.clone(),
        node_id: control.claim.node_id.clone(),
        attempt: control.claim.attempt,
        dispatch_id: control.claim.dispatch_id.clone(),
        lane_ownership_id: control.claim.lane_ownership_id.clone(),
        project_lane_sha256: control.claim.project_lane_sha256.clone(),
        artifact_kind: GroupAgentNodeTerminalArtifactKind::Uncertainty,
        artifact_id: control.artifact.artifact_id.clone(),
        artifact_sha256: control.artifact.artifact_sha256.clone(),
        node_outcome: GroupAgentNodeTerminalOutcome::FailedUncertain,
        wave_index: 0,
        wave_outcome: GroupAgentNodeTerminalOutcome::FailedUncertain,
        graph_status: GroupAgentGraphRunStatus::FailedUncertain,
        retry_authorized: false,
        lane_release_authorized: true,
        receipt_id: String::new(),
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = receipt.expected_sha256().unwrap();
    receipt.receipt_id = group_agent_node_terminal_receipt_id(&receipt.receipt_sha256);
    receipt
        .validate_against_control(control)
        .expect("valid terminal receipt");
    receipt
}

fn terminal_event(
    control: &GroupAgentNodeTerminalControl,
    receipt: &GroupAgentNodeTerminalReceipt,
) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION,
        graph_run_id: control.graph_run.graph_run_id.clone(),
        seq: 5,
        kind: GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
            previous_event_sha256: control.claim.claim_event_sha256.clone(),
            dispatch_id: control.claim.dispatch_id.clone(),
            lane_ownership_id: control.claim.lane_ownership_id.clone(),
            project_lane_sha256: control.claim.project_lane_sha256.clone(),
            artifact_id: control.artifact.artifact_id.clone(),
            artifact_sha256: control.artifact.artifact_sha256.clone(),
            terminal_receipt_id: receipt.receipt_id.clone(),
            terminal_receipt_sha256: receipt.receipt_sha256.clone(),
            node_id: receipt.node_id.clone(),
            attempt: receipt.attempt,
            node_outcome: receipt.node_outcome,
            wave_index: 0,
            wave_outcome: receipt.wave_outcome,
            graph_status: receipt.graph_status,
            retry_authorized: false,
            lane_released: true,
            terminalized_at_ms: 120,
        },
    }
}
