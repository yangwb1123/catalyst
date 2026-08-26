use std::{io::ErrorKind, net::TcpListener, process::Output};

use serde_json::Value;

#[allow(clippy::duplicate_mod)]
mod group_agent_graph_run_support;
#[allow(clippy::duplicate_mod)]
mod group_agent_graph_support;
mod scheduled_graph_reconcile_cli_support;

use group_agent_graph_support::successful_json;
use scheduled_graph_reconcile_cli_support::{
    CREDENTIAL_POISON, ReconcileFixture, TASK_SECRET, WORKSPACE_SECRET, loopback_sentinel,
    shared_core,
};

#[test]
fn real_pinned_core_reconciles_ready_without_effects_or_private_access() {
    let core = shared_core();
    let fixture = ReconcileFixture::new(core);
    let (listener, endpoint) = loopback_sentinel();
    let before = fixture.hub_state();

    let output = fixture.reconcile(core, &core.sha256, &endpoint);
    let value = successful_json(&output);
    assert_ready_output(&value, &fixture.graph_run_id);
    assert_private(&output, &endpoint);

    assert_eq!(fixture.hub_state(), before, "logical Hub state changed");
    fixture.assert_workspace_unchanged();
    assert_no_network(&listener);
}

#[test]
fn mismatched_core_digest_fails_closed_without_state_or_network_effects() {
    let core = shared_core();
    let fixture = ReconcileFixture::new(core);
    let (listener, endpoint) = loopback_sentinel();
    let before = fixture.hub_state();

    let output = fixture.reconcile(core, &"0".repeat(64), &endpoint);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("scheduled Graph Core reconcile is unavailable"),
        "{stderr}"
    );
    assert_private(&output, &endpoint);

    assert_eq!(fixture.hub_state(), before, "failed command changed Hub");
    fixture.assert_workspace_unchanged();
    assert_no_network(&listener);
}

fn assert_ready_output(value: &Value, graph_run_id: &str) {
    assert_reconcile_contract(value);
    assert_core_trust_boundary(&value["core_trust_boundary"]);
    assert_runtime_effect_facts(&value["runtime_effect_facts"]);
    let decision = &value["decision"];
    assert_eq!(decision["graph_run_id"], graph_run_id);
    assert_eq!(decision["disposition"], "ready");
    assert_eq!(decision["next_execution_ordinal"], 0);
    assert_eq!(decision["next_node_id"], "build");
}

fn assert_reconcile_contract(value: &Value) {
    assert_eq!(value["type"], "scheduled_graph_reconcile");
    assert_eq!(value["v"], 1);
    assert_eq!(value["metadata_only"], true);
    assert_eq!(value["progress_snapshot_validated"], true);
    assert_eq!(value["core_decision_validated"], true);
    assert_eq!(value["progress_observed"], true);
    assert_eq!(value["effect_facts_scope"], "forge_runtime");
    assert_eq!(value["sqlite_live_reader_coordination_possible"], true);
    assert_eq!(value["content_included"], false);
}

fn assert_core_trust_boundary(trust: &Value) {
    for field in [
        "same_user_code",
        "operator_trust_required",
        "binary_identity_validated",
        "protocol_handshake_validated",
        "empty_environment",
    ] {
        assert_eq!(trust[field], true, "{field} must be true");
    }
    for field in [
        "filesystem_isolation_enforced",
        "network_isolation_enforced",
        "effect_containment_enforced",
        "effect_attestation_present",
    ] {
        assert_eq!(trust[field], false, "{field} must be false");
    }
}

fn assert_runtime_effect_facts(runtime: &Value) {
    for field in [
        "logical_hub_mutated",
        "scheduled_candidate_materialized",
        "provider_request_prepared",
        "consent_consumed",
        "credential_read",
        "provider_constructed",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "project_lane_claimed",
        "provider_request_sent",
        "execution_authority_released",
        "dispatch_authority_released",
        "node_execution_performed",
        "recovery_performed",
        "retry_performed",
        "resend_performed",
        "terminal_receipt_recorded",
        "successor_authority_granted",
        "result_produced_or_persisted",
        "conversation_prompt_memory_or_writeback_written",
    ] {
        assert_eq!(runtime[field], false, "{field} must be false");
    }
}

fn assert_private(output: &Output, endpoint: &str) {
    let bytes = [&output.stdout[..], &output.stderr[..]].concat();
    let text = String::from_utf8_lossy(&bytes);
    for private in [CREDENTIAL_POISON, TASK_SECRET, WORKSPACE_SECRET, endpoint] {
        assert!(!text.contains(private), "output leaked private input");
    }
}

fn assert_no_network(listener: &TcpListener) {
    let error = listener
        .accept()
        .expect_err("unexpected provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}
