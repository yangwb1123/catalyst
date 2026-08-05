use std::{
    fs,
    io::{ErrorKind, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use forge_runtime_domain::MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

#[allow(dead_code)]
mod cli_group_agent_scheduled_node_contract_support;
#[allow(dead_code)]
mod cli_group_agent_scheduled_node_provider_request_support;
#[allow(dead_code)]
mod group_agent_graph_run_support;
mod group_agent_graph_support;

use cli_group_agent_scheduled_node_contract_support::*;
use cli_group_agent_scheduled_node_provider_request_support::*;
use group_agent_graph_run_support::{
    Fixture, TASK_SECRET, command, human_command, invoke_with_stdin,
};
use group_agent_graph_support::{successful_json, text};

const PRICING: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const CANDIDATE_KEY: &str = "scheduled-release-candidate-key";
const REQUEST_KEY: &str = "scheduled-release-provider-request-key";
const POISON_CREDENTIAL: &str =
    "scheduled-release-credential-must-not-be-read\r\nx-private-header: rejected";

#[test]
fn real_rust_go_rust_round_trip_is_exact_redacted_and_zero_effect() {
    let prepared = PreparedRelease::new();
    prepared.fixture.remove_member_workspaces();
    checkpoint(prepared.fixture.state.path());
    let state_before = all_hub_state(prepared.fixture.state.path());
    let database_before = database_bytes(&prepared.fixture);

    let control = export_release_control(&prepared);
    assert!(!control.ends_with(b"\n"));
    assert_private_control(&control, &prepared.endpoint);
    let authorization = authorize_with_core(&control);
    let verified = verify_stdin(&prepared, &authorization);
    assert_verified_output(&verified, &prepared);
    assert_eq!(verify_file(&prepared, &authorization), verified);
    assert_human_output_is_redacted(&prepared, &authorization);

    assert_eq!(all_hub_state(prepared.fixture.state.path()), state_before);
    assert_eq!(database_bytes(&prepared.fixture), database_before);
    let error = prepared
        .listener
        .accept()
        .expect_err("no provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}

#[test]
fn malformed_noncanonical_utf8_oversize_and_options_fail_before_hub_creation() {
    let canonical = golden_authorization();
    reject_pre_hub(&release_tail(" "), &[], "provider request ID");
    reject_pre_hub(
        &verify_tail("request-1", None),
        &[],
        "requires --authorization",
    );
    reject_pre_hub(&verify_tail("request-1", Some("-")), b"{}", "invalid JSON");
    reject_pre_hub(&verify_tail("request-1", Some("-")), &[0xff], "UTF-8");

    let mut noncanonical = canonical.clone();
    noncanonical.push(b'\n');
    reject_pre_hub(
        &verify_tail("request-1", Some("-")),
        &noncanonical,
        "canonical",
    );
    let oversize = vec![b'x'; MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES + 1];
    reject_pre_hub(
        &verify_tail("request-1", Some("-")),
        &oversize,
        "byte limit",
    );
    reject_unknown_and_selectors_before_hub(&canonical);
}

#[test]
fn unknown_and_wrong_request_ids_fail_without_mutating_current_hub() {
    let prepared = PreparedRelease::new();
    checkpoint(prepared.fixture.state.path());
    let state_before = all_hub_state(prepared.fixture.state.path());
    let database_before = database_bytes(&prepared.fixture);
    let authorization = authorize_with_core(&export_release_control(&prepared));

    assert_failure(
        &export_command(&prepared, "scheduled-provider-request-missing"),
        "not found",
    );
    let wrong = invoke_verify(
        &prepared,
        "scheduled-provider-request-wrong-binding",
        "-",
        &authorization,
    );
    assert_failure(&wrong, "binding disagrees");
    assert_eq!(all_hub_state(prepared.fixture.state.path()), state_before);
    assert_eq!(database_bytes(&prepared.fixture), database_before);
}

#[test]
fn missing_and_v14_hubs_are_never_created_or_migrated() {
    let authorization = golden_authorization();
    reject_missing_hub(&release_tail("request-missing"), &[]);
    reject_missing_hub(&verify_tail("request-missing", Some("-")), &authorization);

    let prepared = PreparedRelease::new();
    let current_authorization = authorize_with_core(&export_release_control(&prepared));
    let database = prepared.fixture.state.path().join("hub.sqlite3");
    downgrade_empty_v15_sidecar(&database);
    checkpoint(prepared.fixture.state.path());
    let before = fs::read(&database).expect("read exact v14 database");
    assert_schema_failure(&export_command(&prepared, &prepared.request_id));
    assert_schema_failure(&invoke_verify(
        &prepared,
        &prepared.request_id,
        "-",
        &current_authorization,
    ));
    assert_eq!(fs::read(&database).expect("reread v14 database"), before);
    let connection = Connection::open(database).expect("inspect v14 Hub");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 14);
    assert!(!provider_request_table_exists(&connection));
}

struct PreparedRelease {
    fixture: Fixture,
    listener: TcpListener,
    endpoint: String,
    graph_run_id: String,
    request_id: String,
}

impl PreparedRelease {
    fn new() -> Self {
        let (listener, endpoint) = loopback_sentinel();
        let fixture = Fixture::new();
        let graph_run_id = prepare_run(&fixture, "scheduled-release-source-run");
        let control = export_control(&fixture, &graph_run_id);
        let schedule = build_schedule(&control);
        admit_schedule(&fixture, &graph_run_id, &schedule);
        let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
        let candidate = build_candidate_at(&control, &schedule_sha256, &endpoint);
        let admitted = admit_candidate(&fixture, &graph_run_id, CANDIDATE_KEY, &candidate);
        let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
        let request_id = prepare_request(&fixture, &contract_id);
        Self {
            fixture,
            listener,
            endpoint,
            graph_run_id,
            request_id,
        }
    }
}

fn prepare_request(fixture: &Fixture, contract_id: &str) -> String {
    let output = scheduled_command_for(
        fixture,
        &["prepare", contract_id, "--idempotency-key", REQUEST_KEY],
    )
    .env("OPENAI_API_KEY", POISON_CREDENTIAL)
    .output()
    .expect("prepare scheduled provider request");
    let value = successful_json(&output);
    text(&value["inspection"]["record"]["provider_request_id"])
}

fn export_release_control(prepared: &PreparedRelease) -> Vec<u8> {
    let output = export_command(prepared, &prepared.request_id);
    assert_success(&output);
    output.stdout
}

fn export_command(prepared: &PreparedRelease, request_id: &str) -> Output {
    scheduled_command(prepared, &["release-control", "export", request_id])
        .output()
        .expect("export scheduled release control")
}

fn authorize_with_core(control: &[u8]) -> Vec<u8> {
    let mut child = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .args([
            "run",
            "./cmd/forge",
            "graph-scheduled-node-dispatch-authorize",
            "--control",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn scheduled authorization Core");
    child.stdin.take().unwrap().write_all(control).unwrap();
    let output = child.wait_with_output().expect("wait for Core");
    assert_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    output.stdout
}

fn verify_stdin(prepared: &PreparedRelease, authorization: &[u8]) -> Value {
    successful_json(&invoke_verify(
        prepared,
        &prepared.request_id,
        "-",
        authorization,
    ))
}

fn verify_file(prepared: &PreparedRelease, authorization: &[u8]) -> Value {
    let path = prepared
        .fixture
        .cwd
        .path()
        .join("scheduled-authorization.json");
    fs::write(&path, authorization).expect("write authorization fixture");
    successful_json(&invoke_verify(
        prepared,
        &prepared.request_id,
        "scheduled-authorization.json",
        &[],
    ))
}

fn invoke_verify(
    prepared: &PreparedRelease,
    request_id: &str,
    source: &str,
    stdin: &[u8],
) -> Output {
    let args = [
        "authorization",
        "verify",
        request_id,
        "--authorization",
        source,
    ];
    let mut child = scheduled_command(prepared, &args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn scheduled authorization verify");
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().expect("wait for verification")
}

fn assert_verified_output(value: &Value, prepared: &PreparedRelease) {
    assert_eq!(
        value["type"],
        "group_agent_scheduled_node_dispatch_authorization_verified"
    );
    assert_eq!(value["authorization_validated_against_current_state"], true);
    assert_eq!(value["all_current_effect_facts_false"], true);
    for decision in value["authorization_decisions"]
        .as_object()
        .unwrap()
        .values()
    {
        assert_eq!(decision, true);
    }
    for fact in value["current_effect_facts"].as_object().unwrap().values() {
        assert_eq!(fact, false);
    }
    assert_eq!(
        value["authorization"]["graph_run_id"],
        prepared.graph_run_id
    );
    assert_eq!(
        value["authorization"]["scheduled_provider_request_id"],
        prepared.request_id
    );
    assert_redacted(&serde_json::to_string(value).unwrap(), prepared);
}

fn assert_human_output_is_redacted(prepared: &PreparedRelease, authorization: &[u8]) {
    let path = prepared.fixture.cwd.path().join("human-authorization.json");
    fs::write(&path, authorization).unwrap();
    let mut args = release_prefix();
    args.extend([
        "authorization",
        "verify",
        &prepared.request_id,
        "--authorization",
        "human-authorization.json",
    ]);
    let output = human_command(
        prepared.fixture.state.path(),
        prepared.fixture.cwd.path(),
        &args,
    );
    assert_success(&output);
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("authorization decisions"));
    assert!(text.contains("current effect facts: all false"));
    assert_redacted(&text, prepared);
}

fn assert_private_control(control: &[u8], endpoint: &str) {
    let value: Value = serde_json::from_slice(control).expect("release-control JSON");
    assert_eq!(value["provider_request"]["endpoint"], endpoint);
    assert!(String::from_utf8_lossy(control).contains(TASK_SECRET));
    assert!(!String::from_utf8_lossy(control).contains(POISON_CREDENTIAL));
}

fn assert_redacted(output: &str, prepared: &PreparedRelease) {
    for private in [
        TASK_SECRET,
        POISON_CREDENTIAL,
        &prepared.endpoint,
        "private-scheduled-contract-model",
        PRICING,
        REQUEST_KEY,
        CANDIDATE_KEY,
    ] {
        assert!(!output.contains(private), "output leaked {private}");
    }
    for field in [
        "project_id",
        "request_body_sha256",
        "pricing_snapshot_sha256",
    ] {
        assert!(!output.contains(field), "output exposed {field}");
    }
}

fn reject_unknown_and_selectors_before_hub(authorization: &[u8]) {
    let mut unknown = verify_tail("request-1", Some("-"));
    unknown.push("--execute");
    reject_pre_hub(&unknown, authorization, "unknown");
    for selector in [["-C", "."], ["--group", "group-1"]] {
        let state = TempDir::new().unwrap();
        let cwd = TempDir::new().unwrap();
        let mut args = selector.to_vec();
        args.extend(release_tail("request-1"));
        let output = invoke_with_stdin(state.path(), cwd.path(), &args, &[]);
        assert_failure(&output, "selectors are not valid");
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

fn reject_pre_hub(args: &[&str], stdin: &[u8], expected: &str) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(state.path(), cwd.path(), args, stdin);
    assert_failure(&output, expected);
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn reject_missing_hub(args: &[&str], stdin: &[u8]) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(state.path(), cwd.path(), args, stdin);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn scheduled_command(prepared: &PreparedRelease, tail: &[&str]) -> Command {
    let mut command = scheduled_command_for(&prepared.fixture, tail);
    command.env("OPENAI_API_KEY", POISON_CREDENTIAL);
    command.env("OPENAI_BASE_URL", &prepared.endpoint);
    command
}

fn scheduled_command_for(fixture: &Fixture, tail: &[&str]) -> Command {
    let mut args = release_prefix();
    args.extend_from_slice(tail);
    command(fixture.state.path(), fixture.cwd.path(), &args)
}

fn release_prefix() -> Vec<&'static str> {
    vec![
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "provider-request",
    ]
}

fn release_tail(request_id: &str) -> Vec<&str> {
    let mut args = release_prefix();
    args.extend(["release-control", "export", request_id]);
    args
}

fn verify_tail<'a>(request_id: &'a str, source: Option<&'a str>) -> Vec<&'a str> {
    let mut args = release_prefix();
    args.extend(["authorization", "verify", request_id]);
    if let Some(source) = source {
        args.extend(["--authorization", source]);
    }
    args
}

fn loopback_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback sentinel");
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    (listener, format!("https://127.0.0.1:{port}/v1/responses"))
}

fn checkpoint(state: &Path) {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub for checkpoint");
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .expect("checkpoint Hub");
}

fn database_bytes(fixture: &Fixture) -> Vec<u8> {
    fs::read(fixture.state.path().join("hub.sqlite3")).expect("read Hub database")
}

fn golden_authorization() -> Vec<u8> {
    let fixture =
        fs::read(repository_root().join(
            "docs/contracts/fixtures/group-agent-scheduled-node-dispatch-authorization-v1.json",
        ))
        .expect("read shared authorization golden");
    let value: Value = serde_json::from_slice(&fixture).expect("shared release fixture JSON");
    text(&value["canonical_authorization_json"]).into_bytes()
}

fn forge_core_dir() -> PathBuf {
    repository_root().join("forge-core")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn assert_schema_failure(output: &Output) {
    assert_failure(output, "current schema version 16");
}

fn assert_failure(output: &Output, expected: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "stderr was {stderr:?}");
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}
