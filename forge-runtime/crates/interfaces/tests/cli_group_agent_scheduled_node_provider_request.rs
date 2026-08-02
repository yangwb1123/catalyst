use std::{fs, io::ErrorKind, net::TcpListener, path::Path, process::Output};

use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

#[allow(dead_code)]
mod cli_group_agent_scheduled_node_contract_support;
mod cli_group_agent_scheduled_node_provider_request_support;
mod group_agent_graph_run_support;
mod group_agent_graph_support;

use cli_group_agent_scheduled_node_contract_support::*;
use cli_group_agent_scheduled_node_provider_request_support::*;
use group_agent_graph_run_support::{
    Fixture, TASK_SECRET, WORKSPACE_SECRET, command, human_command, invoke_with_stdin, run_json,
};
use group_agent_graph_support::{successful_json, text};

const PRICING: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const CANDIDATE_KEY: &str = "scheduled-provider-request-candidate-key";
const REQUEST_KEY: &str = "scheduled-provider-request-key";

#[test]
fn real_go_candidate_prepares_exact_passive_request_without_other_hub_changes() {
    let (listener, endpoint) = loopback_sentinel();
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture, "scheduled-provider-request-source-run");
    let control = export_control(&fixture, &graph_run_id);
    let schedule = build_schedule(&control);
    admit_schedule(&fixture, &graph_run_id, &schedule);
    let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
    let candidate = build_candidate_at(&control, &schedule_sha256, &endpoint);
    let admitted = admit_candidate(&fixture, &graph_run_id, CANDIDATE_KEY, &candidate);
    let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
    let old_hub_state = non_provider_request_hub_state(fixture.state.path());
    fixture.assert_workspace_unchanged();
    fixture.remove_member_workspaces();

    let created = prepare_provider_request(&fixture, &contract_id, REQUEST_KEY);
    let created = successful_json(&created);
    let request_id = assert_created(&created, &graph_run_id, &contract_id, &endpoint);
    assert_hidden_and_explicit_views(&fixture, &request_id, &endpoint);
    assert_list(&fixture, &graph_run_id, &request_id, &endpoint);
    assert_replay_and_conflict(&fixture, &contract_id, &created, &endpoint);
    assert_legacy_dispatch_stays_fenced(&fixture, &graph_run_id, &request_id);

    assert_eq!(
        non_provider_request_hub_state(fixture.state.path()),
        old_hub_state,
        "preexisting Hub tables changed during passive request preparation"
    );
    let error = listener.accept().expect_err("no provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}

#[test]
fn pure_provider_request_prepare_inputs_fail_before_hub_creation() {
    reject_pre_hub(
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "prepare",
            " ",
            "--idempotency-key",
            "key",
        ],
        "scheduled contract ID",
    );
    reject_pre_hub(
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "prepare",
            "contract-1",
            "--idempotency-key",
            "",
        ],
        "idempotency key",
    );
}

#[test]
fn pure_provider_request_read_inputs_fail_before_hub_creation() {
    reject_pre_hub(
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "show",
            " ",
        ],
        "provider request ID",
    );
    reject_pre_hub(
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "list",
            " ",
        ],
        "Graph Run ID",
    );
}

#[test]
fn valid_read_operations_do_not_create_or_migrate_a_missing_hub() {
    for tail in [
        vec!["show", "scheduled-node-provider-request-missing"],
        vec!["list"],
    ] {
        let state = TempDir::new().expect("isolated state");
        let cwd = TempDir::new().expect("isolated cwd");
        let mut args = vec![
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
        ];
        args.extend(tail);
        let output = invoke_with_stdin(state.path(), cwd.path(), &args, &[]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn read_operations_reject_exact_v14_without_migration() {
    let fixture = Fixture::new();
    prepare_run(&fixture, "provider-request-v14-read-source");
    let database = fixture.state.path().join("hub.sqlite3");
    downgrade_empty_v15_sidecar(&database);
    let before = fs::read(&database).expect("read exact v14 database");

    let output = request_command(&fixture, &["list"])
        .output()
        .expect("run read-only list against v14");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("current schema version 15"));
    assert_eq!(fs::read(&database).expect("reread v14 database"), before);

    let connection = Connection::open(database).expect("inspect unchanged v14 database");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("v14 user version");
    assert_eq!(version, 14);
    assert!(!provider_request_table_exists(&connection));
}

#[test]
fn help_and_empty_list_keep_the_passive_boundary_explicit() {
    let fixture = Fixture::new();
    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "list",
        ],
    );
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["returned_requests_present"], false);
    assert_eq!(listed["provider_request_sidecar_rows_returned"], false);
    assert_eq!(listed["current_run_state_included"], false);
    assert_eq!(listed["provider_request_preparation_validated"], false);

    let help = command(fixture.state.path(), fixture.cwd.path(), &["help"])
        .output()
        .expect("run help");
    assert_success(&help);
    let help = String::from_utf8(help.stdout).expect("UTF-8 help");
    for operation in [
        "scheduled-contract provider-request prepare CONTRACT_ID",
        "scheduled-contract provider-request show PROVIDER_REQUEST_ID",
        "scheduled-contract provider-request list",
    ] {
        assert!(help.contains(operation), "help omitted {operation}");
    }
    assert!(!help.contains("scheduled-contract provider-request send"));
    assert!(!help.contains("scheduled-contract provider-request execute"));
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

fn prepare_provider_request(fixture: &Fixture, contract_id: &str, key: &str) -> Output {
    command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "prepare",
            contract_id,
            "--idempotency-key",
            key,
        ],
    )
    .env(
        "OPENAI_API_KEY",
        format!("{CREDENTIAL_MARKER}\r\nx-private-header: rejected"),
    )
    .env_remove("ANTHROPIC_API_KEY")
    .output()
    .expect("prepare scheduled provider request")
}

fn assert_created(created: &Value, run: &str, contract: &str, endpoint: &str) -> String {
    assert_eq!(created["disposition"], "created");
    let inspection = &created["inspection"];
    assert_eq!(inspection["record"]["graph_run_id"], run);
    assert_eq!(inspection["record"]["scheduled_contract_id"], contract);
    assert_eq!(inspection["passive_scheduled_provider_request"], true);
    assert_eq!(inspection["request_body_validated"], true);
    assert_effect_boundaries(inspection, true);
    assert_private(created, endpoint);
    text(&inspection["record"]["provider_request_id"])
}

fn assert_hidden_and_explicit_views(fixture: &Fixture, request_id: &str, endpoint: &str) {
    let hidden = request_json(fixture, &["show", request_id]);
    assert_eq!(hidden["inspection"]["request_included"], false);
    assert_private(&hidden, endpoint);

    let shown = request_json(fixture, &["show", request_id, "--include-request"]);
    assert_eq!(shown["inspection"]["request_included"], true);
    let body = text(&shown["inspection"]["provider_request_body"]);
    assert_eq!(
        body.as_bytes(),
        stored_body(fixture.state.path(), request_id)
    );
    assert!(!body.ends_with('\n'));
    let exact: Value = serde_json::from_str(&body).expect("exact provider request JSON");
    assert_eq!(exact["model"], "private-scheduled-contract-model");
    assert_eq!(exact["store"], false);
    assert_eq!(exact["stream"], true);
    assert_eq!(exact["tools"], serde_json::json!([]));
    assert_eq!(exact["max_output_tokens"], 1024);
    assert!(body.contains(TASK_SECRET));
    assert!(!body.contains(CREDENTIAL_MARKER));

    let human = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "show",
            request_id,
        ],
    );
    assert_success(&human);
    let human = String::from_utf8(human.stdout).expect("human output");
    assert!(human.contains("passive exact-byte sidecar only"));
    assert!(human.contains("provider request hidden"));
    assert!(!human.contains(endpoint));
    assert!(!human.contains(TASK_SECRET));
}

fn assert_list(fixture: &Fixture, run: &str, request_id: &str, endpoint: &str) {
    let listed = request_json(fixture, &["list", run, "--limit", "1"]);
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["returned_requests_present"], true);
    assert_eq!(listed["provider_request_sidecar_rows_returned"], true);
    assert_eq!(listed["current_run_state_included"], false);
    assert_eq!(listed["source_and_request_validated"], false);
    assert_eq!(listed["provider_request_preparation_validated"], false);
    assert_eq!(listed["requests"][0]["provider_request_id"], request_id);
    assert_private(&listed, endpoint);
    let human = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "list",
            run,
        ],
    );
    assert_success(&human);
    let human = String::from_utf8(human.stdout).expect("human list output");
    assert!(human.contains("current Run lifecycle and dispatch state were not inspected"));
    assert!(!human.contains("current Run lifecycle and dispatch request are unchanged"));
}

fn assert_replay_and_conflict(
    fixture: &Fixture,
    contract_id: &str,
    created: &Value,
    endpoint: &str,
) {
    let replayed = successful_json(&prepare_provider_request(fixture, contract_id, REQUEST_KEY));
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(
        replayed["inspection"]["record"],
        created["inspection"]["record"]
    );
    assert_private(&replayed, endpoint);

    let conflict = prepare_provider_request(fixture, contract_id, "different-request-key");
    assert_cli_conflict(&conflict);
}

fn assert_legacy_dispatch_stays_fenced(fixture: &Fixture, run: &str, request_id: &str) {
    assert_legacy_prepare_rejects(fixture, run);
    assert_legacy_show_and_list_cannot_discover(fixture, run, request_id);
    assert_legacy_release_export_rejects(fixture, run);
}

fn assert_legacy_prepare_rejects(fixture: &Fixture, run: &str) {
    let mut output = legacy_command(
        fixture,
        fixture.state.path(),
        &[
            "prepare",
            run,
            "--idempotency-key",
            "legacy-dispatch-after-scheduled-request",
        ],
    );
    let output = output.output().expect("legacy prepare remains fenced");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("has no admitted execution contract"),
        "stderr was {stderr:?}"
    );
}

fn assert_legacy_show_and_list_cannot_discover(fixture: &Fixture, run: &str, request_id: &str) {
    let shown = legacy_command(fixture, fixture.state.path(), &["show", request_id])
        .output()
        .expect("legacy show remains isolated");
    assert!(!shown.status.success());
    assert!(shown.stdout.is_empty());

    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "dispatch", "list", run],
    );
    assert_eq!(listed["returned_requests_present"], false);
    assert_eq!(listed["requests"], serde_json::json!([]));
}

fn assert_legacy_release_export_rejects(fixture: &Fixture, run: &str) {
    let output = legacy_command(
        fixture,
        fixture.state.path(),
        &["release-control", "export", run],
    )
    .output()
    .expect("legacy release export remains isolated");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}

fn legacy_command(fixture: &Fixture, state: &Path, tail: &[&str]) -> std::process::Command {
    let mut args = vec!["group", "graph", "run", "dispatch"];
    args.extend_from_slice(tail);
    let mut command = command(state, fixture.cwd.path(), &args);
    command.env(
        "OPENAI_API_KEY",
        format!("{CREDENTIAL_MARKER}\r\nx-private-header: rejected"),
    );
    command
}

fn request_json(fixture: &Fixture, tail: &[&str]) -> Value {
    successful_json(
        &request_command(fixture, tail)
            .output()
            .expect("run scheduled provider-request command"),
    )
}

fn request_command(fixture: &Fixture, tail: &[&str]) -> std::process::Command {
    let mut args = vec![
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "provider-request",
    ];
    args.extend_from_slice(tail);
    command(fixture.state.path(), fixture.cwd.path(), &args)
}

fn assert_effect_boundaries(value: &Value, request_present: bool) {
    assert_eq!(value["candidate_provider_request_present"], false);
    assert_eq!(value["provider_request_sidecar_present"], request_present);
    for field in [
        "current_run_dispatch_request_present",
        "current_run_lifecycle_included",
        "provider_request_sent",
        "lifecycle_contract_admitted",
        "execution_authority_released",
        "dispatch_authority_released",
        "project_lane_claimed",
        "progress_observed",
        "successor_advance_authorized",
        "fresh_off_machine_consent_obtained",
        "credential_read",
        "provider_constructed",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "result_or_receipt_produced",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
    ] {
        assert_eq!(value[field], false, "{field} must be false");
    }
}

fn assert_private(value: &Value, endpoint: &str) {
    let encoded = serde_json::to_string(value).expect("output JSON");
    for secret in [
        TASK_SECRET,
        WORKSPACE_SECRET,
        CREDENTIAL_MARKER,
        REQUEST_KEY,
        endpoint,
        "private-scheduled-contract-model",
        "project_lane_sha256",
        "pricing_snapshot_sha256",
        "logical_request_sha256",
        "provider_request_sha256",
        "prepared_request_sha256",
        "system_prompt",
        "user_prompt",
    ] {
        assert!(!encoded.contains(secret), "default output leaked {secret}");
    }
    assert!(!encoded.contains("provider_request_body"));
}
