#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use forge_runtime_domain::MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES;
use serde_json::Value;
use tempfile::TempDir;

mod group_agent_graph_run_support;
mod group_agent_graph_support;
mod group_agent_node_dispatch_authorization_support;

use group_agent_graph_run_support::Fixture;
use group_agent_graph_support::successful_json;
use group_agent_node_dispatch_authorization_support::*;

const OFFICIAL_ENDPOINT: &str = "https://api.openai.com/v1/responses";

#[test]
fn real_go_pricing_and_authorization_reach_only_effect_free_rust_readiness() {
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture);
    let scheduler_control = export_scheduler_control(&fixture, &graph_run_id);
    let pricing = build_pricing_with_real_core();
    assert!(!pricing.ends_with(b"\n"));
    let pricing_value: Value = serde_json::from_slice(&pricing).expect("pricing JSON");
    let pricing_sha256 = pricing_value["pricing_snapshot_sha256"]
        .as_str()
        .expect("pricing digest");
    let contract =
        build_contract_with_pricing(&scheduler_control, OFFICIAL_ENDPOINT, pricing_sha256);
    admit_contract(&fixture, &graph_run_id, &contract);
    prepare_dispatch_request(&fixture, &graph_run_id);
    fixture.remove_member_workspaces();
    let state_before = state_file_bytes(&fixture);
    assert_no_sqlite_sidecars(fixture.state.path());

    let release_control = export_release_control(&fixture, &graph_run_id, true);
    let authorization = authorize_with_real_core(&release_control);
    let verified = verify_readiness(&fixture, &graph_run_id, &authorization, &pricing);

    assert_readiness_output(&verified, pricing_sha256);
    assert_eq!(state_file_bytes(&fixture), state_before);
    assert_no_sqlite_sidecars(fixture.state.path());
    assert_run_still_waits_for_authority(&fixture, &graph_run_id);
}

fn assert_readiness_output(verified: &Value, pricing_sha256: &str) {
    assert_eq!(
        verified["type"],
        "group_agent_node_dispatch_readiness_verified"
    );
    assert_eq!(verified["readiness_validated"], true);
    assert_eq!(verified["authorization_validated"], true);
    assert_eq!(verified["destination_registered"], true);
    assert_eq!(verified["pricing_snapshot_validated"], true);
    assert_eq!(verified["pricing_upper_bound_within_budget"], true);
    assert_eq!(verified["pricing_provenance"], "operator_asserted");
    assert_eq!(verified["vendor_attestation_present"], false);
    for field in [
        "final_effectful_preflight_performed",
        "dispatch_authority_released",
        "consent_obtained",
        "credential_read",
        "credential_preflight_performed",
        "provider_constructed",
        "provider_used",
        "network_accessed",
        "project_lane_claimed",
        "execution_performed",
        "result_produced",
        "result_persisted",
        "graph_advanced",
        "database_written",
    ] {
        assert_eq!(verified[field], false, "{field}");
    }
    let encoded = serde_json::to_string(&verified).unwrap();
    for private in [
        OFFICIAL_ENDPOINT,
        pricing_sha256,
        "private-release-authorization-model",
    ] {
        assert!(
            !encoded.contains(private),
            "readiness output leaked {private}"
        );
    }
}

#[test]
fn invalid_pricing_fails_before_database_construction() {
    let state = TempDir::new().expect("state directory");
    let cwd = TempDir::new().expect("current directory");
    fs::write(cwd.path().join("authorization.json"), b"{}").unwrap();
    let args = readiness_stdin_args();
    for (input, expected) in [
        (vec![0xff], "must be UTF-8"),
        (
            vec![b'x'; MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES + 1],
            "exceeds its byte limit",
        ),
    ] {
        let output = invoke_raw(state.path(), cwd.path(), &args, &input);
        assert_failure(&output);
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn readiness_rejects_idempotency_key_before_reading_state() {
    let state = TempDir::new().expect("state directory");
    let cwd = TempDir::new().expect("current directory");
    let mut args = vec!["--idempotency-key", "forbidden"];
    args.extend(readiness_stdin_args());
    let output = invoke_raw(state.path(), cwd.path(), &args, b"{}");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("only valid for mutating"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn readiness_stdin_args() -> Vec<&'static str> {
    vec![
        "group",
        "graph",
        "run",
        "dispatch",
        "readiness",
        "verify",
        "run-1",
        "--authorization",
        "authorization.json",
        "--pricing",
        "-",
    ]
}

fn build_pricing_with_real_core() -> Vec<u8> {
    let output = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "run",
            "./cmd/forge",
            "graph-node-pricing-snapshot",
            "--model",
            "private-release-authorization-model",
            "--input-usd-micros-per-token-unit",
            "2000000",
            "--output-usd-micros-per-token-unit",
            "10000000",
            "--max-input-tokens",
            "400000",
        ])
        .output()
        .expect("build pricing snapshot");
    assert_success(&output);
    output.stdout
}

fn verify_readiness(
    fixture: &Fixture,
    graph_run_id: &str,
    authorization: &[u8],
    pricing: &[u8],
) -> Value {
    let path = fixture.cwd.path().join("pricing.json");
    fs::write(&path, pricing).expect("write pricing fixture");
    let output = invoke_raw(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "readiness",
            "verify",
            graph_run_id,
            "--authorization",
            "-",
            "--pricing",
            path.to_str().expect("UTF-8 temporary path"),
        ],
        authorization,
    );
    successful_json(&output)
}

fn forge_core_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("forge-core")
}
