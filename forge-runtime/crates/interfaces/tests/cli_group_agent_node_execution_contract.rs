use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use forge_runtime_domain::MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

mod group_agent_graph_run_support;
mod group_agent_graph_support;
use group_agent_graph_run_support::{
    Fixture, TASK_SECRET, WORKSPACE_SECRET, command, human_command, invoke_with_stdin, run_json,
};
use group_agent_graph_support::{path_text, successful_json, text};

const ENDPOINT: &str = "https://api.example.test/v1/responses";
const MODEL: &str = "passive-test-model";
const ADMISSION_KEY: &str = "real-go-rust-contract-key";

#[derive(Debug, Eq, PartialEq)]
struct DurableAdmission {
    contract_id: String,
    contract_blob: Vec<u8>,
    contract_created_at_ms: i64,
    event_blob: Vec<u8>,
    event_created_at_ms: i64,
    event_sha256: Vec<u8>,
    journal_bytes: i64,
}

#[test]
fn real_go_to_rust_cli_pipeline_is_passive_and_replay_safe() {
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture);
    let unrelated_before = unrelated_counts(fixture.state.path());
    let control = export_control(&fixture, &graph_run_id);
    let contract = build_contract_with_real_go(&control);
    assert_contract_shape(&contract, &graph_run_id);

    let contract_path = fixture.cwd.path().join("first-node-contract.json");
    fs::write(&contract_path, &contract).expect("write explicit contract");
    let created = admit_file(&fixture, &graph_run_id, &contract_path);
    let contract_id = assert_created_is_private(&created, &graph_run_id);
    let durable = durable_admission(fixture.state.path());

    assert_contract_views(&fixture, &contract_id, &contract);
    assert_run_transition(&fixture, &graph_run_id);
    assert_replay(&fixture, &graph_run_id, &contract, &created);
    assert_eq!(durable_admission(fixture.state.path()), durable);
    assert_export_is_closed(&fixture, &graph_run_id);
    assert_eq!(unrelated_counts(fixture.state.path()), unrelated_before);
    fixture.assert_workspace_unchanged();
}

#[test]
fn invalid_contract_input_fails_before_hub_creation() {
    reject_pre_hub(br#"{"v":"PRIVATE-CONTRACT-INPUT"}"#, "invalid");
    reject_pre_hub(&[0xff, 0xfe], "UTF-8");

    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    ))
    .expect("shared fixture JSON");
    let mut noncanonical = text(&fixture["expected"]["canonical_contract_json"]).into_bytes();
    noncanonical.push(b'\n');
    reject_pre_hub(&noncanonical, "not canonical");

    let oversized = vec![b' '; MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES + 1];
    reject_pre_hub(&oversized, "byte limit");
}

fn prepare_run(fixture: &Fixture) -> String {
    let prepared = successful_json(&fixture.prepare(&fixture.plan(), "contract-source-run"));
    assert_eq!(prepared["disposition"], "created");
    text(&prepared["inspection"]["run"]["graph_run_id"])
}

fn export_control(fixture: &Fixture, graph_run_id: &str) -> Vec<u8> {
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "control", "export", graph_run_id],
    )
    .output()
    .expect("export control");
    assert_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    let control: Value = serde_json::from_slice(&output.stdout).expect("control JSON");
    assert_eq!(control["graph_run_id"], graph_run_id);
    assert_eq!(control["last_event_seq"], 1);
    assert_eq!(control["execution_contract_present"], false);
    assert_eq!(control["dispatch_authority_released"], false);
    assert!(String::from_utf8_lossy(&output.stdout).contains(TASK_SECRET));
    output.stdout
}

fn build_contract_with_real_go(control: &[u8]) -> Vec<u8> {
    let mut child = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args(go_contract_args())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real forge-core");
    child
        .stdin
        .take()
        .expect("Go stdin")
        .write_all(control)
        .expect("write exact control");
    let output = child.wait_with_output().expect("wait for forge-core");
    assert_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    output.stdout
}

fn go_contract_args() -> [&'static str; 23] {
    [
        "run",
        "./cmd/forge",
        "graph-node-contract",
        "--control",
        "-",
        "--endpoint",
        ENDPOINT,
        "--model",
        MODEL,
        "--max-output-tokens",
        "1024",
        "--max-model-output-bytes",
        "8192",
        "--max-model-events",
        "128",
        "--timeout-ms",
        "30000",
        "--max-cost-usd-micros",
        "1000000",
        "--pricing-snapshot-sha256",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "--max-result-bytes",
        "16384",
    ]
}

fn assert_contract_shape(contract: &[u8], graph_run_id: &str) {
    let value: Value = serde_json::from_slice(contract).expect("contract JSON");
    assert_eq!(value["graph_run_id"], graph_run_id);
    assert_eq!(value["node"]["node_id"], "frontend");
    assert_eq!(value["node"]["topology_wave_index"], 0);
    assert_eq!(value["node"]["attempt"], 1);
    assert_eq!(value["workspace"]["mode"], "none");
    assert_eq!(value["provider"]["endpoint"], ENDPOINT);
    assert_eq!(value["provider"]["model"], MODEL);
    assert_eq!(value["request"]["tools"], serde_json::json!([]));
    assert_eq!(value["execution_contract_present"], true);
    assert_eq!(value["dispatch_authority_released"], false);
}

fn admit_file(fixture: &Fixture, graph_run_id: &str, contract_path: &Path) -> Value {
    run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "contract",
            "admit",
            graph_run_id,
            "--contract",
            path_text(contract_path),
            "--idempotency-key",
            ADMISSION_KEY,
        ],
    )
}

fn assert_created_is_private(created: &Value, graph_run_id: &str) -> String {
    assert_eq!(
        created["type"],
        "group_agent_node_execution_contract_admitted"
    );
    assert_eq!(created["disposition"], "created");
    let inspection = &created["inspection"];
    assert_eq!(inspection["record"]["graph_run_id"], graph_run_id);
    assert_eq!(inspection["graph_run"]["status"], "awaiting_core_dispatch");
    assert_eq!(inspection["explicit_contract_file_read"], true);
    assert_contract_boundaries(inspection);
    assert_private(created);
    text(&inspection["record"]["contract_id"])
}

fn assert_contract_views(fixture: &Fixture, contract_id: &str, contract: &[u8]) {
    let hidden = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "contract", "show", contract_id],
    );
    assert_eq!(hidden["inspection"]["contract_included"], false);
    assert_private(&hidden);

    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "contract",
            "show",
            contract_id,
            "--include-contract",
        ],
    );
    let expected: Value = serde_json::from_slice(contract).expect("expected contract");
    assert_eq!(shown["inspection"]["contract"], expected);
    assert_eq!(shown["inspection"]["contract_included"], true);
    assert!(
        serde_json::to_string(&shown)
            .expect("shown JSON")
            .contains(TASK_SECRET)
    );
    assert_contract_list(fixture, contract_id);
    assert_human_contract_is_terminal_safe(fixture, contract_id);
}

fn assert_contract_list(fixture: &Fixture, contract_id: &str) {
    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "contract", "list"],
    );
    assert_eq!(listed["type"], "group_agent_node_execution_contracts");
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["contract_included"], false);
    assert_eq!(listed["contracts"][0]["contract_id"], contract_id);
    assert_eq!(listed["contracts"].as_array().expect("contracts").len(), 1);
    assert_private(&listed);
}

fn assert_human_contract_is_terminal_safe(fixture: &Fixture, contract_id: &str) {
    let output = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "contract",
            "show",
            contract_id,
            "--include-contract",
        ],
    );
    assert_success(&output);
    let shown = String::from_utf8(output.stdout).expect("human output");
    assert!(shown.contains("contract: {"));
    assert!(shown.contains(r"\n\nManager instruction:\n"));
    assert!(shown.contains("dispatch authority not released"));
}

fn assert_run_transition(fixture: &Fixture, graph_run_id: &str) {
    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "show", graph_run_id],
    );
    let inspection = &shown["inspection"];
    assert_eq!(inspection["v"], 2);
    assert_eq!(inspection["run"]["v"], 2);
    assert_eq!(inspection["run"]["status"], "awaiting_core_dispatch");
    assert_eq!(inspection["run"]["last_event_seq"], 2);
    assert_eq!(inspection["events"].as_array().expect("events").len(), 2);
    assert_eq!(inspection["execution_contract_present"], true);
    assert_eq!(inspection["model_selected"], true);
    assert_run_effects_false(inspection);
}

fn assert_replay(fixture: &Fixture, graph_run_id: &str, contract: &[u8], created: &Value) {
    let replayed = successful_json(&invoke_with_stdin(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "contract",
            "admit",
            graph_run_id,
            "--contract",
            "-",
            "--idempotency-key",
            ADMISSION_KEY,
        ],
        contract,
    ));
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(
        replayed["inspection"]["record"],
        created["inspection"]["record"]
    );
    assert_eq!(
        replayed["inspection"]["graph_run"],
        created["inspection"]["graph_run"]
    );
    assert_eq!(replayed["inspection"]["explicit_contract_file_read"], false);
    assert_private(&replayed);
}

fn assert_export_is_closed(fixture: &Fixture, graph_run_id: &str) {
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "control", "export", graph_run_id],
    )
    .output()
    .expect("repeat control export");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exact v1"));
}

fn durable_admission(state: &Path) -> DurableAdmission {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    let contract = connection
        .query_row(
            "SELECT id,contract_blob,created_at_ms
             FROM group_agent_graph_node_execution_contracts",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("durable contract");
    let event = connection
        .query_row(
            "SELECT event_blob,created_at_ms,event_sha256
             FROM group_agent_graph_run_events WHERE seq = 2",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("durable event");
    let journal_bytes = connection
        .query_row(
            "SELECT journal_bytes FROM group_agent_graph_runs",
            [],
            |row| row.get(0),
        )
        .expect("journal bytes");
    DurableAdmission {
        contract_id: contract.0,
        contract_blob: contract.1,
        contract_created_at_ms: contract.2,
        event_blob: event.0,
        event_created_at_ms: event.1,
        event_sha256: event.2,
        journal_bytes,
    }
}

fn reject_pre_hub(input: &[u8], expected_error: &str) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(
        state.path(),
        cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "contract",
            "admit",
            "missing-run",
            "--contract",
            "-",
        ],
        input,
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_error),
        "expected {expected_error:?}, stderr was {stderr:?}"
    );
    assert!(!stderr.contains("PRIVATE-CONTRACT-INPUT"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn assert_contract_boundaries(inspection: &Value) {
    assert_eq!(inspection["contract_admitted"], true);
    assert_eq!(inspection["source_graph_validated"], true);
    assert_eq!(inspection["control_snapshot_validated"], true);
    assert_eq!(inspection["contract_and_journal_validated"], true);
    assert_eq!(inspection["execution_contract_present"], true);
    assert_eq!(inspection["dispatch_authority_released"], false);
    assert_eq!(inspection["provider_configuration_present"], true);
    assert_eq!(inspection["model_selected"], true);
    for field in contract_effect_fields() {
        assert_eq!(inspection[field], false, "{field} must remain false");
    }
}

fn assert_run_effects_false(inspection: &Value) {
    for field in [
        "execution_performed",
        "manager_execution_performed",
        "node_execution_performed",
        "model_used",
        "capabilities_granted",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "task_results_produced",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
    ] {
        assert_eq!(inspection[field], false, "{field} must remain false");
    }
}

fn contract_effect_fields() -> [&'static str; 11] {
    [
        "credential_read",
        "model_used",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "task_results_produced",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
        "dispatch_authority_released",
    ]
}

fn assert_private(value: &Value) {
    let encoded = serde_json::to_string(value).expect("output JSON");
    for secret in [
        TASK_SECRET,
        WORKSPACE_SECRET,
        ENDPOINT,
        MODEL,
        ADMISSION_KEY,
    ] {
        assert!(!encoded.contains(secret), "default output leaked {secret}");
    }
    assert!(!encoded.contains("\"request\""));
    assert!(!encoded.contains("\"provider\""));
}

fn unrelated_counts(state: &Path) -> (i64, i64, i64) {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    (
        table_count(&connection, "runs"),
        table_count(&connection, "run_events"),
        table_count(&connection, "prompts"),
    )
}

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("count table")
}

fn forge_core_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("forge-core")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
