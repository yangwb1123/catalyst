use crate::runtime_domain::GroupAgentScheduledNodeTerminalArtifactKind;

use super::{
    GroupAgentNodeTerminalClassification, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchExecutionCliOutput, GroupAgentScheduledNodeLifecycleStatus,
    GroupAgentScheduledNodeTerminalArtifact,
};

#[test]
fn adjudication_reports_database_write_without_dispatch() {
    let output = GroupAgentScheduledNodeDispatchExecutionCliOutput::from_parts(
        2,
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated,
        &claim(),
        false,
        None,
        None,
        false,
        true,
        false,
    );
    let value = serde_json::to_value(output).expect("serialize adjudication output");
    assert_eq!(value["dispatch_performed_this_invocation"], false);
    assert_eq!(value["database_written_this_invocation"], true);
    assert_eq!(value["status"], "adjudicated");
}

#[test]
fn adjudication_cleanup_failure_preserves_write_facts_and_reports_unknown_presence() {
    let output = GroupAgentScheduledNodeDispatchExecutionCliOutput::from_parts(
        2,
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated,
        &claim(),
        false,
        None,
        None,
        false,
        true,
        false,
    )
    .with_owner_cleanup(super::ScheduledExecutorOwnerCleanup::Failed);
    let value = serde_json::to_value(output).expect("serialize adjudication output");
    assert_eq!(value["dispatch_performed_this_invocation"], false);
    assert_eq!(value["database_written_this_invocation"], true);
    assert_eq!(value["owner_sidecar_cleanup_observation"], "failed");
    assert!(value["owner_sidecar_left_active_by_this_invocation"].is_null());
}

#[test]
fn quarantine_cleanup_failure_preserves_poll_write_and_no_receipt_facts() {
    let output = GroupAgentScheduledNodeDispatchExecutionCliOutput::from_parts(
        1,
        GroupAgentScheduledNodeLifecycleStatus::Quarantined,
        &claim(),
        false,
        Some(&quarantine_artifact()),
        None,
        true,
        true,
        false,
    )
    .with_owner_cleanup(super::ScheduledExecutorOwnerCleanup::Failed);
    let value = serde_json::to_value(output).expect("serialize quarantine output");
    assert_eq!(value["status"], "quarantined");
    assert_eq!(value["provider_poll_started"], true);
    assert_eq!(value["remote_provider_request_observation"], "not_attested");
    assert_eq!(value["dispatch_performed_this_invocation"], true);
    assert_eq!(value["database_written_this_invocation"], true);
    assert_eq!(value["retry_authorized"], false);
    assert!(value["outcome"].is_null());
    assert_eq!(value["owner_sidecar_cleanup_observation"], "failed");
    assert!(value["owner_sidecar_left_active_by_this_invocation"].is_null());
}

fn quarantine_artifact() -> GroupAgentScheduledNodeTerminalArtifact {
    GroupAgentScheduledNodeTerminalArtifact {
        v: 1,
        terminal_artifact_protocol_version: 1,
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Uncertainty,
        graph_run_id: "graph-run-1".into(),
        node_id: "node-1".into(),
        attempt: 1,
        dispatch_id: "dispatch-1".into(),
        provider_request_id: "provider-request-1".into(),
        claim_event_sha256: digest('1'),
        authorization_sha256: digest('a'),
        provider_request_sha256: digest('b'),
        request_body_sha256: digest('c'),
        pricing_snapshot_sha256: digest('d'),
        lane_ownership_id: "lane-owner-1".into(),
        project_lane_sha256: digest('e'),
        provider_poll_started: true,
        terminal_seen: false,
        stream_eof_seen: false,
        classification: GroupAgentNodeTerminalClassification::TransportError,
        output_text: String::new(),
        output_bytes: 0,
        output_sha256: digest('2'),
        usage_observed: false,
        input_tokens: 0,
        output_tokens: 0,
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms: 2,
        artifact_id: "artifact-1".into(),
        artifact_bytes: 1,
        artifact_sha256: digest('3'),
    }
}

fn claim() -> GroupAgentScheduledNodeDispatchClaim {
    GroupAgentScheduledNodeDispatchClaim {
        v: 1,
        graph_run_id: "graph-run-1".into(),
        provider_request_id: "provider-request-1".into(),
        dispatch_id: "dispatch-1".into(),
        authorization_id: "authorization-1".into(),
        authorization_sha256: digest('a'),
        provider_request_sha256: digest('b'),
        request_body_sha256: digest('c'),
        request_body_bytes: 1,
        pricing_snapshot_sha256: digest('d'),
        node_id: "node-1".into(),
        attempt: 1,
        max_cost_usd_micros: 1,
        lane_ownership_id: "lane-owner-1".into(),
        project_lane_sha256: digest('e'),
        expected_last_event_seq: 1,
        expected_last_event_sha256: digest('f'),
        claim_event_sha256: digest('1'),
        released_at_ms: 1,
    }
}

fn digest(character: char) -> String {
    std::iter::repeat_n(character, 64).collect()
}
