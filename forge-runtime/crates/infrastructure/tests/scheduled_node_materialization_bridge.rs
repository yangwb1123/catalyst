#![cfg(unix)]

#[allow(clippy::duplicate_mod, dead_code)]
mod core_terminal_bridge_support;

use std::fs;

use forge_runtime_domain::{
    GroupAgentGraphControlSnapshot, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractScope,
    GroupAgentScheduledNodeTerminalArtifactKind, GroupAgentScheduledNodeTerminalReceipt,
    ScheduledGraphControllerExecutionProfile, ScheduledGraphNodeMaterializationInput,
    ScheduledGraphNodeMaterializationPort, ScheduledGraphNodeMaterializationPortError,
    group_agent_project_lane_sha256, group_agent_scheduled_node_terminal_artifact_id,
    group_agent_scheduled_node_terminal_receipt_id,
};
use forge_runtime_infrastructure::PinnedScheduledNodeMaterializationBridge;
use serde::Deserialize;
use tempfile::tempdir;

use core_terminal_bridge_support::{build_go_forge, script_digest, write_script};

#[derive(Deserialize)]
struct MaterializationFixture {
    input: FixtureInput,
    expected: FixtureExpected,
}

#[derive(Deserialize)]
struct FixtureInput {
    canonical_control_snapshot_json: String,
    schedule_sha256: String,
    execution_options: FixtureExecutionOptions,
}

#[derive(Deserialize)]
struct FixtureExecutionOptions {
    endpoint: String,
    model: String,
    max_output_tokens: u64,
    max_model_output_bytes: u64,
    max_model_events: u64,
    timeout_ms: u64,
    max_cost_usd_micros: u64,
    pricing_snapshot_sha256: String,
    max_result_bytes: u64,
}

#[derive(Deserialize)]
struct FixtureExpected {
    selected_node_id: String,
    canonical_contract_json: String,
}

#[test]
fn compiled_go_core_materializes_exact_initial_and_same_wave_successor() {
    let fixture = fixture();
    let input = materialization_input(&fixture);
    let expected = GroupAgentScheduledNodeContractCandidate::decode_exact(
        &fixture.expected.canonical_contract_json,
    )
    .expect("strict shared candidate");
    let directory = tempdir().expect("temporary Go build directory");
    let path = build_go_forge(directory.path());
    let bridge = PinnedScheduledNodeMaterializationBridge::new(path.clone(), script_digest(&path))
        .expect("materialization protocol v2 handshake");

    let output = bridge.materialize(&input).expect("initial materialization");

    assert_eq!(output.candidate, expected);
    assert_eq!(
        output.candidate_json,
        fixture.expected.canonical_contract_json
    );

    assert_same_wave_successor(&bridge, input, &fixture.expected.selected_node_id);
}

fn assert_same_wave_successor(
    bridge: &PinnedScheduledNodeMaterializationBridge,
    mut successor_input: ScheduledGraphNodeMaterializationInput,
    initial_node_id: &str,
) {
    successor_input.execution_ordinal = 1;
    successor_input.node_id = successor_input.control_snapshot.plan.authored_node_ids[1].clone();
    successor_input.predecessor_receipts = vec![completed_receipt(
        &successor_input.control_snapshot,
        initial_node_id,
    )];
    let successor = bridge
        .materialize(&successor_input)
        .expect("same-wave successor materialization");
    assert_eq!(
        successor.candidate.contract_scope,
        GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly
    );
    assert_eq!(successor.candidate.node.execution_ordinal, 1);
    assert_eq!(successor.candidate.node.node_id, "backend");
    assert!(
        successor
            .candidate
            .request
            .required_predecessor_node_ids
            .is_empty()
    );
    assert!(
        successor
            .candidate
            .request
            .predecessor_terminal_receipts
            .is_empty(),
        "consumed same-wave sibling is not a direct predecessor"
    );
    assert_eq!(
        successor
            .candidate
            .canonical_json()
            .expect("canonical successor"),
        successor.candidate_json
    );
}

#[test]
fn constructor_requires_the_exact_materialization_protocol() {
    for (name, protocol) in [("legacy", "1"), ("suffixed", "2x"), ("line", "2\n")] {
        let directory = tempdir().expect("temporary Core directory");
        let script = materializer_script(protocol, "printf '%s' '{}'");
        let path = write_script(directory.path(), name, &script, true);

        assert!(
            PinnedScheduledNodeMaterializationBridge::new(path.clone(), script_digest(&path))
                .is_err(),
            "accepted {name} protocol"
        );
    }
}

#[test]
fn successful_process_with_invalid_candidate_is_distinguished_from_unavailable_core() {
    let fixture = fixture();
    let input = materialization_input(&fixture);
    let directory = tempdir().expect("temporary Core directory");
    let script = materializer_script("2", "printf '%s' '{}'");
    let path = write_script(directory.path(), "invalid-output", &script, true);
    let bridge = PinnedScheduledNodeMaterializationBridge::new(path.clone(), script_digest(&path))
        .expect("valid materialization handshake");

    assert_eq!(
        bridge.materialize(&input),
        Err(ScheduledGraphNodeMaterializationPortError::InvalidCandidate)
    );
}

#[test]
fn executable_bytes_changed_after_handshake_are_rejected_before_materialization() {
    let fixture = fixture();
    let input = materialization_input(&fixture);
    let directory = tempdir().expect("temporary Core directory");
    let original = materializer_script("2", "printf '%s' '{}'");
    let path = write_script(directory.path(), "changed-core", &original, true);
    let bridge = PinnedScheduledNodeMaterializationBridge::new(path.clone(), script_digest(&path))
        .expect("valid materialization handshake");
    fs::write(&path, format!("{original}\n# changed after handshake\n"))
        .expect("replace pinned executable bytes");

    assert_eq!(
        bridge.materialize(&input),
        Err(ScheduledGraphNodeMaterializationPortError::Unavailable)
    );
}

fn fixture() -> MaterializationFixture {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .expect("shared materialization fixture")
}

fn materialization_input(
    fixture: &MaterializationFixture,
) -> ScheduledGraphNodeMaterializationInput {
    let control: GroupAgentGraphControlSnapshot =
        serde_json::from_str(&fixture.input.canonical_control_snapshot_json)
            .expect("shared control snapshot");
    control.validate().expect("valid control snapshot");
    assert_eq!(
        control
            .canonical_json()
            .expect("canonical control snapshot"),
        fixture.input.canonical_control_snapshot_json
    );
    let options = &fixture.input.execution_options;
    let execution_profile = ScheduledGraphControllerExecutionProfile {
        endpoint: options.endpoint.clone(),
        model: options.model.clone(),
        max_output_tokens: options.max_output_tokens,
        max_model_output_bytes: options.max_model_output_bytes,
        max_model_events: options.max_model_events,
        timeout_ms: options.timeout_ms,
        max_cost_usd_micros: options.max_cost_usd_micros,
        pricing_snapshot_sha256: options.pricing_snapshot_sha256.clone(),
        max_result_bytes: options.max_result_bytes,
        profile_sha256: String::new(),
    }
    .seal()
    .expect("sealed execution profile");
    ScheduledGraphNodeMaterializationInput {
        control_snapshot: control,
        schedule_sha256: fixture.input.schedule_sha256.clone(),
        execution_ordinal: 0,
        node_id: fixture.expected.selected_node_id.clone(),
        predecessor_receipts: Vec::new(),
        execution_profile,
    }
}

fn materializer_script(protocol: &str, decision: &str) -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" != \"graph-scheduled-node-contract\" ]; then exit 90; fi\n\
         if [ \"$2\" = \"--protocol-version\" ]; then printf '%s' '{protocol}'; exit 0; fi\n\
         if [ \"$2\" != \"--control\" ] || [ \"$3\" != \"-\" ]; then exit 91; fi\n\
         {decision}\n"
    )
}

fn completed_receipt(
    control: &GroupAgentGraphControlSnapshot,
    node_id: &str,
) -> GroupAgentScheduledNodeTerminalReceipt {
    let project_id = control
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .expect("receipt source node")
        .project_id
        .as_str();
    let artifact_sha256 = "c".repeat(64);
    let mut receipt = GroupAgentScheduledNodeTerminalReceipt {
        v: 1,
        scheduler_protocol_version: 1,
        terminal_receipt_protocol_version: 1,
        terminal_control_sha256: "a".repeat(64),
        graph_run_id: control.graph_run_id.clone(),
        graph_id: control.graph_id.clone(),
        node_id: node_id.into(),
        attempt: 1,
        dispatch_id: "scheduled-node-dispatch-materializer-test".into(),
        provider_request_id: "scheduled-node-provider-request-materializer-test".into(),
        project_lane_sha256: group_agent_project_lane_sha256(project_id),
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Result,
        artifact_id: group_agent_scheduled_node_terminal_artifact_id(&artifact_sha256),
        artifact_sha256,
        node_outcome: GroupAgentNodeTerminalOutcome::Completed,
        retry_authorized: false,
        lane_release_authorized: true,
        successor_advance_authorized: false,
        receipt_id: String::new(),
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = receipt.expected_sha256().expect("terminal receipt digest");
    receipt.receipt_id = group_agent_scheduled_node_terminal_receipt_id(&receipt.receipt_sha256);
    receipt.validate().expect("valid terminal receipt");
    receipt
}
