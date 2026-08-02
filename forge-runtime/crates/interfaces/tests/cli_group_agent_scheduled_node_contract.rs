use std::{io::ErrorKind, net::TcpListener};

use forge_runtime_domain::MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES;
use serde_json::Value;
use tempfile::TempDir;

mod cli_group_agent_scheduled_node_contract_support;
mod group_agent_graph_run_support;
mod group_agent_graph_support;
use cli_group_agent_scheduled_node_contract_support::*;
use group_agent_graph_run_support::{
    Fixture, TASK_SECRET, WORKSPACE_SECRET, human_command, invoke_with_stdin, run_json,
};
use group_agent_graph_support::text;

const CANDIDATE_KEY: &str = "real-go-rust-scheduled-contract-key";
const PRICING: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

#[test]
fn real_go_to_rust_initial_candidate_is_passive_private_and_replay_safe() {
    let (listener, endpoint) = loopback_sentinel();
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture, "scheduled-contract-source-run");
    let control = export_control(&fixture, &graph_run_id);
    let schedule = build_schedule(&control);
    let schedule_id = admit_schedule(&fixture, &graph_run_id, &schedule);
    let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
    let candidate = build_candidate_at(&control, &schedule_sha256, &endpoint);
    assert_candidate_shape(&candidate, &graph_run_id, &schedule_id);
    let hub_before = non_candidate_hub_state(fixture.state.path());
    fixture.assert_workspace_unchanged();
    fixture.remove_member_workspaces();

    let created = admit_candidate(&fixture, &graph_run_id, CANDIDATE_KEY, &candidate);
    let contract_id = assert_created_is_private(&created, &graph_run_id, &schedule_id, &endpoint);
    assert_views(&fixture, &contract_id, &candidate, &endpoint);
    assert_replay(&fixture, &graph_run_id, &candidate, &created, &endpoint);
    assert_different_key_conflicts(&fixture, &graph_run_id, &candidate);
    assert_legacy_contract_conflicts(&fixture, &graph_run_id, &control);
    assert_eq!(non_candidate_hub_state(fixture.state.path()), hub_before);
    let error = listener.accept().expect_err("no provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}

fn loopback_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback sentinel");
    listener
        .set_nonblocking(true)
        .expect("nonblocking loopback sentinel");
    let endpoint = format!(
        "https://127.0.0.1:{}/v1/responses",
        listener.local_addr().expect("listener address").port()
    );
    (listener, endpoint)
}

#[test]
fn existing_legacy_contract_rejects_scheduled_candidate() {
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture, "legacy-first-source-run");
    let control = export_control(&fixture, &graph_run_id);
    let schedule = build_schedule(&control);
    admit_schedule(&fixture, &graph_run_id, &schedule);
    let candidate = build_candidate(&control, &text(&json(&schedule)["schedule_sha256"]));
    let legacy = build_legacy_contract(&control);
    admit_legacy_contract(&fixture, &graph_run_id, &legacy);
    let output = invoke_candidate(&fixture, &graph_run_id, CANDIDATE_KEY, &candidate);
    assert_conflict(&output);
}

#[test]
fn malformed_candidate_fails_before_hub_creation() {
    reject_pre_hub(br#"{"v":"PRIVATE-CANDIDATE-INPUT"}"#, "noncanonical");
    reject_pre_hub(&[0xff, 0xfe], "noncanonical");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    ))
    .expect("shared candidate fixture");
    let canonical = text(&fixture["expected"]["canonical_contract_json"]);
    let mut noncanonical = canonical.clone().into_bytes();
    noncanonical.push(b'\n');
    reject_pre_hub(&noncanonical, "noncanonical");
    for drift in [
        canonical.replacen("{\"v\":2,", "{\"v\":2,\"v\":2,", 1),
        canonical.replacen("{\"v\":2,", "{\"v\":2,\"unknown\":false,", 1),
        canonical.replacen("\"contract_scope\":\"schedule_initial_node_only\",", "", 1),
        canonical.replacen(
            "\"required_predecessor_node_ids\":[]",
            "\"required_predecessor_node_ids\":null",
            1,
        ),
        canonical.replacen(
            "{\"v\":2,\"scheduler_protocol_version\":1",
            "{\"scheduler_protocol_version\":1,\"v\":2",
            1,
        ),
        canonical.replacen("graph-run-fixture-v1", r"\u0067raph-run-fixture-v1", 1),
        canonical.replacen(
            "\"contract_id\":\"scheduled-node-contract-",
            "\"contract_id\":\"scheduled-node-contract-0",
            1,
        ),
    ] {
        reject_pre_hub_failure(drift.as_bytes());
    }
    let oversized = vec![b' '; MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES + 1];
    reject_pre_hub(&oversized, "byte limit");
}

#[test]
fn pure_binding_key_and_identifier_errors_fail_before_hub_creation() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    ))
    .expect("shared candidate fixture");
    let candidate = text(&fixture["expected"]["canonical_contract_json"]);
    reject_command_pre_hub(
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "admit",
            "different-run",
            "--contract",
            "-",
        ],
        candidate.as_bytes(),
        "Graph Run binding",
    );
    reject_command_pre_hub(
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "admit",
            "graph-run-fixture-v1",
            "--contract",
            "-",
            "--idempotency-key",
            "",
        ],
        candidate.as_bytes(),
        "idempotency key",
    );
    reject_command_pre_hub(
        &["group", "graph", "run", "scheduled-contract", "show", " "],
        &[],
        "scheduled contract ID",
    );
    reject_command_pre_hub(
        &["group", "graph", "run", "scheduled-contract", "list", " "],
        &[],
        "Graph Run ID",
    );
}

fn assert_candidate_shape(candidate: &[u8], run: &str, schedule_id: &str) {
    let value = json(candidate);
    assert_eq!(value["v"], 2);
    assert_eq!(value["contract_scope"], "schedule_initial_node_only");
    assert_eq!(value["graph_run_id"], run);
    assert_eq!(value["schedule_id"], schedule_id);
    assert_eq!(value["node"]["execution_ordinal"], 0);
    assert_eq!(value["node"]["node_id"], "frontend");
    assert_eq!(
        value["request"]["required_predecessor_node_ids"],
        Value::Array(vec![])
    );
    assert_eq!(
        value["request"]["predecessor_terminal_receipts"],
        Value::Array(vec![])
    );
    assert_eq!(value["request"]["predecessor_content_included"], false);
    for field in false_candidate_fields() {
        assert_eq!(value[field], false, "{field} must be false");
    }
}

fn assert_created_is_private(
    created: &Value,
    run: &str,
    schedule_id: &str,
    endpoint: &str,
) -> String {
    assert_eq!(created["disposition"], "created");
    let inspection = &created["inspection"];
    assert_eq!(inspection["record"]["graph_run_id"], run);
    assert_eq!(inspection["record"]["schedule_id"], schedule_id);
    assert_eq!(inspection["passive_initial_candidate_only"], true);
    assert_eq!(inspection["stored_schedule_validated"], true);
    assert_boundaries(inspection);
    assert_private(created);
    assert_endpoint_hidden(created, endpoint);
    text(&inspection["record"]["contract_id"])
}

fn assert_views(fixture: &Fixture, contract_id: &str, candidate: &[u8], endpoint: &str) {
    let hidden = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "show",
            contract_id,
        ],
    );
    assert_private(&hidden);
    assert_endpoint_hidden(&hidden, endpoint);
    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "show",
            contract_id,
            "--include-contract",
        ],
    );
    assert_eq!(shown["inspection"]["contract"], json(candidate));
    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "scheduled-contract", "list"],
    );
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["passive_initial_candidate_only"], true);
    assert_eq!(listed["current_run_lifecycle_included"], false);
    for field in false_candidate_fields() {
        assert_eq!(listed[field], false, "list {field} must be false");
    }
    for field in effect_fields() {
        assert_eq!(listed[field], false, "list {field} must be false");
    }
    assert_private(&listed);
    assert_endpoint_hidden(&listed, endpoint);
    assert_human_view(fixture, contract_id);
}

fn assert_human_view(fixture: &Fixture, contract_id: &str) {
    let human = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "show",
            contract_id,
        ],
    );
    assert_success(&human);
    let text = String::from_utf8(human.stdout).expect("human output");
    assert!(text.contains("passive initial-node candidate only"));
    assert!(text.contains("current Run lifecycle is not reported"));
}

fn assert_replay(fixture: &Fixture, run: &str, candidate: &[u8], created: &Value, endpoint: &str) {
    let replayed = admit_candidate(fixture, run, CANDIDATE_KEY, candidate);
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(
        replayed["inspection"]["record"],
        created["inspection"]["record"]
    );
    assert_private(&replayed);
    assert_endpoint_hidden(&replayed, endpoint);
}

fn assert_different_key_conflicts(fixture: &Fixture, run: &str, candidate: &[u8]) {
    assert_conflict(&invoke_candidate(
        fixture,
        run,
        "different-scheduled-contract-key",
        candidate,
    ));
}

fn assert_legacy_contract_conflicts(fixture: &Fixture, run: &str, control: &[u8]) {
    let legacy = build_legacy_contract(control);
    let output = invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "contract",
            "admit",
            run,
            "--contract",
            "-",
            "--idempotency-key",
            "legacy-after-candidate",
        ],
        &legacy,
    );
    assert_conflict(&output);
}

fn assert_boundaries(inspection: &Value) {
    assert_eq!(inspection["current_run_lifecycle_included"], false);
    assert_eq!(inspection["predecessor_receipts_present"], false);
    assert_eq!(inspection["predecessor_content_included"], false);
    for field in false_candidate_fields() {
        assert_eq!(inspection[field], false, "{field} must be false");
    }
    for field in effect_fields() {
        assert_eq!(inspection[field], false, "{field} must be false");
    }
}

fn false_candidate_fields() -> [&'static str; 6] {
    [
        "lifecycle_contract_admitted",
        "provider_request_present",
        "execution_authority_released",
        "dispatch_authority_released",
        "progress_observed",
        "successor_advance_authorized",
    ]
}

fn effect_fields() -> [&'static str; 7] {
    [
        "credential_read",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "result_or_receipt_produced",
        "writeback_performed",
    ]
}

fn assert_private(value: &Value) {
    let encoded = serde_json::to_string(value).expect("output JSON");
    for secret in [
        TASK_SECRET,
        WORKSPACE_SECRET,
        CANDIDATE_KEY,
        CREDENTIAL_MARKER,
        "project_lane_sha256",
        "request_sha256",
        "system_prompt",
        "user_prompt",
    ] {
        assert!(!encoded.contains(secret), "default output leaked {secret}");
    }
    for node_id in ["frontend", "backend", "sso"] {
        assert!(!encoded.contains(&format!("\"{node_id}\"")));
    }
    assert!(!encoded.contains("\"contract\":"));
}

fn assert_endpoint_hidden(value: &Value, endpoint: &str) {
    let encoded = serde_json::to_string(value).expect("output JSON");
    assert!(
        !encoded.contains(endpoint),
        "default output leaked endpoint"
    );
}

fn reject_pre_hub(input: &[u8], expected: &str) {
    reject_command_pre_hub(
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "admit",
            "missing-run",
            "--contract",
            "-",
        ],
        input,
        expected,
    );
}

fn reject_pre_hub_failure(input: &[u8]) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(
        state.path(),
        cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "admit",
            "missing-run",
            "--contract",
            "-",
        ],
        input,
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn reject_command_pre_hub(args: &[&str], input: &[u8], expected: &str) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(state.path(), cwd.path(), args, input);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "stderr was {stderr:?}");
    assert!(!state.path().join("hub.sqlite3").exists());
}
