use crate::runtime_domain::{
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodePredecessorOutcome, GroupAgentScheduledNodePredecessorReceipt,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalArtifactKind,
    GroupAgentScheduledNodeTerminalReceipt, group_agent_scheduled_node_terminal_artifact_id,
    group_agent_scheduled_node_terminal_output_sha256,
    group_agent_scheduled_node_user_prompt_with_output,
};

use super::{
    GroupAgentScheduledNodeContractServiceError,
    scheduled_contract_tests::{resign_candidate, successor_candidate},
    scheduled_contract_validation::receipt_binding_matches,
    scheduled_successor_service::stored_predecessor_content_matches,
};

#[test]
fn reentry_rejects_resigned_embedded_content_drift_from_durable_result() {
    let durable_output = "frontend produced: login flow verified";
    let candidate = successor_candidate(Some(durable_output));
    candidate.validate().expect("valid original successor");
    let artifact = durable_result_artifact(durable_output);
    artifact.validate().expect("valid durable Result artifact");
    stored_predecessor_content_matches(&candidate, Some(&artifact))
        .expect("unchanged embedded output matches durable evidence");

    let mut drifted = candidate.clone();
    drifted.request.user_prompt = group_agent_scheduled_node_user_prompt_with_output(
        &drifted.node.node_id,
        "backend task",
        "backend acceptance",
        "frontend produced: different bytes",
    )
    .expect("drifted Prompt");
    resign_candidate(&mut drifted);
    drifted
        .validate()
        .expect("self-consistent re-signed successor");
    assert_ne!(drifted.contract_id, candidate.contract_id);
    let drifted_json = drifted.canonical_json().expect("drifted canonical JSON");
    assert_eq!(
        crate::runtime_domain::GroupAgentScheduledNodeContractCandidate::decode_exact(
            &drifted_json,
        )
        .expect("exact drifted candidate"),
        drifted,
    );

    assert!(matches!(
        stored_predecessor_content_matches(&drifted, Some(&artifact)),
        Err(GroupAgentScheduledNodeContractServiceError::Corrupt { .. })
    ));
}

#[test]
fn durable_failed_outcomes_cannot_bind_successor_evidence() {
    let completed_stored = stored_receipt();
    let completed_candidate = candidate_receipt();
    assert!(receipt_binding_matches(
        &completed_stored,
        &completed_candidate
    ));

    let cases = [
        (
            GroupAgentNodeTerminalOutcome::Failed,
            GroupAgentScheduledNodePredecessorOutcome::Failed,
        ),
        (
            GroupAgentNodeTerminalOutcome::FailedUncertain,
            GroupAgentScheduledNodePredecessorOutcome::FailedUncertain,
        ),
    ];
    for (stored_outcome, candidate_outcome) in cases {
        let mut stored = completed_stored.clone();
        stored.node_outcome = stored_outcome;
        assert!(
            !receipt_binding_matches(&stored, &completed_candidate),
            "a durable terminal failure must not masquerade as completed"
        );
        let mut candidate = completed_candidate.clone();
        candidate.node_outcome = candidate_outcome;
        assert!(
            !receipt_binding_matches(&completed_stored, &candidate),
            "a candidate terminal failure must not advance a successor"
        );
    }
}

fn stored_receipt() -> GroupAgentScheduledNodeTerminalReceipt {
    let receipt_sha256 = "a".repeat(64);
    GroupAgentScheduledNodeTerminalReceipt {
        v: 1,
        scheduler_protocol_version: 1,
        terminal_receipt_protocol_version: 1,
        terminal_control_sha256: "b".repeat(64),
        graph_run_id: "graph-run".into(),
        graph_id: "graph".into(),
        node_id: "frontend".into(),
        attempt: 1,
        dispatch_id: "dispatch-frontend".into(),
        provider_request_id: "scheduled-node-provider-request-frontend".into(),
        project_lane_sha256: "c".repeat(64),
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Result,
        artifact_id: format!("scheduled-node-terminal-artifact-{}", "d".repeat(64)),
        artifact_sha256: "d".repeat(64),
        node_outcome: GroupAgentNodeTerminalOutcome::Completed,
        retry_authorized: false,
        lane_release_authorized: true,
        successor_advance_authorized: false,
        receipt_id: format!("scheduled-node-terminal-receipt-{receipt_sha256}"),
        receipt_sha256,
    }
}

fn candidate_receipt() -> GroupAgentScheduledNodePredecessorReceipt {
    GroupAgentScheduledNodePredecessorReceipt {
        predecessor_node_id: "frontend".into(),
        predecessor_attempt: 1,
        terminal_event_seq: 0,
        terminal_event_sha256: String::new(),
        terminal_receipt_id: format!("scheduled-node-terminal-receipt-{}", "a".repeat(64)),
        terminal_receipt_sha256: "a".repeat(64),
        node_outcome: GroupAgentScheduledNodePredecessorOutcome::Completed,
        provider_request_id: "scheduled-node-provider-request-frontend".into(),
        dispatch_id: "dispatch-frontend".into(),
    }
}

fn durable_result_artifact(output_text: &str) -> GroupAgentScheduledNodeTerminalArtifact {
    let mut artifact = GroupAgentScheduledNodeTerminalArtifact {
        v: 1,
        terminal_artifact_protocol_version: 1,
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Result,
        graph_run_id: "graph-run".into(),
        node_id: "frontend".into(),
        attempt: 1,
        dispatch_id: "dispatch-frontend".into(),
        provider_request_id: "scheduled-node-provider-request-frontend".into(),
        claim_event_sha256: "1".repeat(64),
        authorization_sha256: "2".repeat(64),
        provider_request_sha256: "3".repeat(64),
        request_body_sha256: "4".repeat(64),
        pricing_snapshot_sha256: "5".repeat(64),
        lane_ownership_id: "lane-frontend".into(),
        project_lane_sha256: "6".repeat(64),
        provider_poll_started: true,
        terminal_seen: true,
        stream_eof_seen: true,
        classification: GroupAgentNodeTerminalClassification::Completed,
        output_text: output_text.into(),
        output_bytes: output_text.len(),
        output_sha256: group_agent_scheduled_node_terminal_output_sha256(output_text),
        usage_observed: true,
        input_tokens: 1,
        output_tokens: 1,
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms: 100,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    };
    let digest = artifact.expected_sha256().expect("artifact digest");
    artifact.artifact_id = group_agent_scheduled_node_terminal_artifact_id(&digest);
    artifact.artifact_sha256 = digest;
    for _ in 0..2 {
        artifact.artifact_bytes = artifact.canonical_json().expect("artifact JSON").len();
    }
    artifact
}
