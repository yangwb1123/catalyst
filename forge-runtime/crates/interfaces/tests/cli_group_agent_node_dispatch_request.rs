use std::{
    io::{ErrorKind, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use rusqlite::Connection;
use serde_json::Value;

mod group_agent_graph_run_support;
mod group_agent_graph_support;
use group_agent_graph_run_support::{
    Fixture, TASK_SECRET, WORKSPACE_SECRET, command, human_command, invoke_with_stdin, run_json,
};
use group_agent_graph_support::{successful_json, text};

const MODEL: &str = "private-passive-request-model";
const PREPARE_KEY: &str = "private-dispatch-prepare-key";
const CREDENTIAL_MARKER: &str = "credential-must-never-be-read-or-printed";
const CREDENTIAL_SENTINEL: &str =
    "credential-must-never-be-read-or-printed\r\nx-private-header: rejected";
const PRICING_IDENTITY: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn prepare_show_list_and_replay_are_local_private_and_effect_free() {
    let (listener, endpoint) = loopback_sentinel();
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture);
    let control = export_control(&fixture, &graph_run_id);
    let contract = build_contract_with_real_go(&control, &endpoint);
    let contract_id = admit_contract(&fixture, &graph_run_id, &contract);
    let unrelated_before = unrelated_counts(fixture.state.path());
    fixture.assert_workspace_unchanged();
    fixture.remove_member_workspaces();

    let created = dispatch_json_without_credentials(
        &fixture,
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "prepare",
            &graph_run_id,
            "--idempotency-key",
            PREPARE_KEY,
        ],
    );
    assert_eq!(
        created["type"],
        "group_agent_node_dispatch_request_prepared"
    );
    assert_eq!(created["disposition"], "created");
    let inspection = &created["inspection"];
    assert_eq!(inspection["record"]["graph_run_id"], graph_run_id);
    assert_eq!(inspection["record"]["contract_id"], contract_id);
    assert_boundaries(inspection);
    assert_private(&created, &endpoint);
    let request_id = text(&inspection["record"]["dispatch_request_id"]);

    assert_hidden_and_explicit_views(&fixture, &request_id, &endpoint);
    assert_list(&fixture, &graph_run_id, &request_id, &endpoint);
    assert_replay(&fixture, &graph_run_id, &created, &endpoint);
    assert_run_transition(&fixture, &graph_run_id);
    assert_eq!(unrelated_counts(fixture.state.path()), unrelated_before);

    let error = listener.accept().expect_err("no provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}

fn loopback_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback sentinel");
    listener
        .set_nonblocking(true)
        .expect("nonblocking sentinel");
    let endpoint = format!(
        "https://127.0.0.1:{}/v1/responses",
        listener.local_addr().expect("listener address").port()
    );
    (listener, endpoint)
}

#[test]
fn help_exposes_prepare_show_list_but_no_effectful_dispatch_command() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .arg("help")
        .output()
        .expect("run help");
    assert_success(&output);
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    for command in [
        "group graph run dispatch prepare GRAPH_RUN_ID",
        "group graph run dispatch show DISPATCH_REQUEST_ID",
        "group graph run dispatch list [GRAPH_RUN_ID]",
    ] {
        assert!(help.contains(command), "help omitted {command}");
    }
    assert!(!help.contains("group graph run dispatch claim"));
    assert!(!help.contains("group graph run dispatch send"));
}

#[test]
fn empty_metadata_list_does_not_claim_that_a_request_or_pricing_was_validated() {
    let fixture = Fixture::new();
    let listed = dispatch_json(&fixture, &["group", "graph", "run", "dispatch", "list"]);
    assert_eq!(listed["returned_requests_present"], false);
    assert_eq!(listed["request_preparation_validated"], false);
    assert_eq!(listed["pricing_snapshot_identity_validated"], false);

    let human = dispatch_human(&fixture, &["group", "graph", "run", "dispatch", "list"]);
    assert_success(&human);
    let human = String::from_utf8(human.stdout).expect("human output");
    assert!(human.contains("no request metadata returned; preparation was not inferred"));
    assert!(!human.contains("exact provider request prepared locally"));
    assert!(!human.contains("pricing identity pinned"));
}

fn prepare_run(fixture: &Fixture) -> String {
    let prepared = successful_json(&fixture.prepare(&fixture.plan(), "dispatch-source-run"));
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
    output.stdout
}

fn build_contract_with_real_go(control: &[u8], endpoint: &str) -> Vec<u8> {
    let mut child = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "run",
            "./cmd/forge",
            "graph-node-contract",
            "--control",
            "-",
            "--endpoint",
            endpoint,
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
            PRICING_IDENTITY,
            "--max-result-bytes",
            "16384",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-core");
    child
        .stdin
        .take()
        .expect("Go stdin")
        .write_all(control)
        .expect("write control");
    let output = child.wait_with_output().expect("wait for forge-core");
    assert_success(&output);
    output.stdout
}

fn admit_contract(fixture: &Fixture, graph_run_id: &str, contract: &[u8]) -> String {
    let admitted = successful_json(&invoke_with_stdin(
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
            "dispatch-contract-admission",
        ],
        contract,
    ));
    text(&admitted["inspection"]["record"]["contract_id"])
}

fn assert_hidden_and_explicit_views(fixture: &Fixture, request_id: &str, endpoint: &str) {
    assert_hidden_view(fixture, request_id, endpoint);
    assert_explicit_view(fixture, request_id);
}

fn assert_hidden_view(fixture: &Fixture, request_id: &str, endpoint: &str) {
    let hidden = dispatch_json(
        fixture,
        &["group", "graph", "run", "dispatch", "show", request_id],
    );
    assert_eq!(hidden["inspection"]["request_included"], false);
    assert_private(&hidden, endpoint);
    let hidden_human = dispatch_human(
        fixture,
        &["group", "graph", "run", "dispatch", "show", request_id],
    );
    assert_success(&hidden_human);
    let hidden_human = String::from_utf8(hidden_human.stdout).expect("human output");
    assert!(hidden_human.contains("provider request hidden"));
    assert_private_text(&hidden_human, endpoint);
}

fn assert_explicit_view(fixture: &Fixture, request_id: &str) {
    let shown = dispatch_json(
        fixture,
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "show",
            request_id,
            "--include-request",
        ],
    );
    let body = text(&shown["inspection"]["provider_request_body"]);
    assert!(
        !serde_json::to_string(&shown)
            .expect("shown JSON")
            .contains(CREDENTIAL_MARKER)
    );
    assert_exact_body(fixture, request_id, &body);
    assert_explicit_human(fixture, request_id);
}

fn assert_exact_body(fixture: &Fixture, request_id: &str, body: &str) {
    let stored = stored_body(fixture.state.path(), request_id);
    assert_eq!(body.as_bytes(), stored);
    assert!(!body.ends_with('\n'));
    let request: Value = serde_json::from_str(body).expect("exact request JSON");
    assert_eq!(request["model"], MODEL);
    assert_eq!(request["store"], false);
    assert_eq!(request["stream"], true);
    assert_eq!(request["tools"], serde_json::json!([]));
    assert!(body.contains(TASK_SECRET));
}

fn assert_explicit_human(fixture: &Fixture, request_id: &str) {
    let human = human_command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "show",
            request_id,
            "--include-request",
        ],
    );
    assert_success(&human);
    let human = String::from_utf8(human.stdout).expect("human output");
    assert!(human.contains("provider request: {"));
    assert!(human.contains("dispatch authority not released"));
    assert!(!human.contains(CREDENTIAL_MARKER));
}

fn assert_list(fixture: &Fixture, graph_run_id: &str, request_id: &str, endpoint: &str) {
    let listed = dispatch_json(
        fixture,
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "list",
            graph_run_id,
            "--limit",
            "1",
        ],
    );
    assert_eq!(listed["type"], "group_agent_node_dispatch_requests");
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["source_contract_and_request_validated"], false);
    assert_eq!(listed["request_preparation_validated"], false);
    assert_eq!(listed["pricing_snapshot_identity_validated"], false);
    assert_eq!(listed["requests"][0]["dispatch_request_id"], request_id);
    assert_eq!(listed["requests"].as_array().expect("requests").len(), 1);
    assert_private(&listed, endpoint);
    let human = dispatch_human(
        fixture,
        &["group", "graph", "run", "dispatch", "list", graph_run_id],
    );
    assert_success(&human);
    let human = String::from_utf8(human.stdout).expect("human output");
    assert_private_text(&human, endpoint);
    assert!(human.contains("metadata reports stored request rows"));
    assert!(!human.contains("exact provider request prepared locally"));
    assert!(!human.contains("pricing identity pinned"));
}

fn assert_replay(fixture: &Fixture, graph_run_id: &str, created: &Value, endpoint: &str) {
    let replayed = dispatch_json(
        fixture,
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "prepare",
            graph_run_id,
            "--idempotency-key",
            PREPARE_KEY,
        ],
    );
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(
        replayed["inspection"]["record"],
        created["inspection"]["record"]
    );
    assert_private(&replayed, endpoint);
    let human = dispatch_human(
        fixture,
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "prepare",
            graph_run_id,
            "--idempotency-key",
            PREPARE_KEY,
        ],
    );
    assert_success(&human);
    let human = String::from_utf8(human.stdout).expect("human output");
    assert!(human.contains("replayed"));
    assert_private_text(&human, endpoint);
}

fn assert_run_transition(fixture: &Fixture, graph_run_id: &str) {
    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "show", graph_run_id],
    );
    let run = &shown["inspection"]["run"];
    assert_eq!(run["v"], 3);
    assert_eq!(run["status"], "awaiting_dispatch_authorization");
    assert_eq!(run["dispatch_request_present"], true);
    assert_eq!(run["dispatch_authority_released"], false);
    assert_eq!(run["last_event_seq"], 3);
}

fn assert_boundaries(inspection: &Value) {
    assert_eq!(inspection["request_prepared"], true);
    assert_eq!(inspection["request_body_validated"], true);
    assert_eq!(inspection["pricing_snapshot_identity_pinned"], true);
    assert_eq!(inspection["pricing_policy_enforced"], false);
    for field in effect_fields() {
        assert_eq!(inspection[field], false, "{field} must remain false");
    }
}

fn effect_fields() -> [&'static str; 13] {
    [
        "dispatch_authority_released",
        "fresh_off_machine_consent_obtained",
        "credential_read",
        "execution_performed",
        "model_used",
        "provider_used",
        "network_accessed",
        "workspace_accessed",
        "tools_used",
        "result_produced",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
    ]
}

fn assert_private(value: &Value, endpoint: &str) {
    let encoded = serde_json::to_string(value).expect("output JSON");
    assert_private_text(&encoded, endpoint);
    assert!(value.get("provider_request_body").is_none());
    assert!(value["inspection"].get("provider_request_body").is_none());
}

fn assert_private_text(encoded: &str, endpoint: &str) {
    for secret in [
        TASK_SECRET,
        WORKSPACE_SECRET,
        endpoint,
        MODEL,
        PRICING_IDENTITY,
        PREPARE_KEY,
        CREDENTIAL_MARKER,
    ] {
        assert!(!encoded.contains(secret), "default output leaked {secret}");
    }
}

fn dispatch_json(fixture: &Fixture, args: &[&str]) -> Value {
    let mut command = command(fixture.state.path(), fixture.cwd.path(), args);
    command.env("OPENAI_API_KEY", CREDENTIAL_SENTINEL);
    successful_json(&command.output().expect("run dispatch CLI"))
}

fn dispatch_json_without_credentials(fixture: &Fixture, args: &[&str]) -> Value {
    let mut command = command(fixture.state.path(), fixture.cwd.path(), args);
    command
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY");
    successful_json(&command.output().expect("run credential-free dispatch CLI"))
}

fn dispatch_human(fixture: &Fixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .current_dir(fixture.cwd.path())
        .env("OPENAI_API_KEY", CREDENTIAL_SENTINEL)
        .env_remove("ANTHROPIC_API_KEY")
        .args(["--state-dir", path_text(fixture.state.path())])
        .args(args)
        .output()
        .expect("run human dispatch CLI")
}

fn stored_body(state: &Path, request_id: &str) -> Vec<u8> {
    Connection::open(state.join("hub.sqlite3"))
        .expect("open Hub")
        .query_row(
            "SELECT provider_request_blob FROM group_agent_graph_node_dispatch_requests WHERE id=?1",
            [request_id],
            |row| row.get(0),
        )
        .expect("stored exact body")
}

fn unrelated_counts(state: &Path) -> Vec<(String, i64)> {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT IN ('group_agent_graph_runs','group_agent_graph_run_events','group_agent_graph_node_dispatch_requests') ORDER BY name")
        .expect("list effect-unrelated tables");
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query effect-unrelated tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("read effect-unrelated table names");
    names
        .into_iter()
        .map(|name| {
            let count = table_count(&connection, &name);
            (name, count)
        })
        .collect()
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

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temporary path")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
