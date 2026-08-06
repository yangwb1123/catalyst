use std::{
    fs,
    io::{ErrorKind, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use forge_runtime_domain::{
    MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES,
};
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
use cli_group_agent_scheduled_node_provider_request_support::assert_provider_request_success as assert_cli_success;
use cli_group_agent_scheduled_node_provider_request_support::*;
use group_agent_graph_run_support::{Fixture, TASK_SECRET, invoke_with_stdin};
use group_agent_graph_support::{path_text, successful_json, text};

const PRICING: &str = "48f3531a7d71015453dc27a71bd0f17efbaf68ddfcff04461bd5d01b52cade8d";
const OFFICIAL_ENDPOINT: &str = "https://api.openai.com/v1/responses";
const MODEL: &str = "private-scheduled-contract-model";
const EXACT_BUDGET: u64 = 1_000_000;
const POISON: &str = "scheduled-readiness-secret-must-not-be-read-or-emitted";

#[test]
fn real_go_artifacts_reach_only_effect_free_scheduled_rust_readiness() {
    let prepared = PreparedReadiness::new();
    prepared.fixture.remove_member_workspaces();
    checkpoint(prepared.fixture.state.path());
    let files_before = state_file_bytes(prepared.fixture.state.path());
    let rows_before = all_hub_state(prepared.fixture.state.path());

    let file_file = verify_files(&prepared);
    let auth_stdin = verify_auth_stdin(&prepared);
    let pricing_stdin = verify_pricing_stdin(&prepared);
    assert_eq!(file_file, auth_stdin);
    assert_eq!(file_file, pricing_stdin);
    assert_readiness_output(&file_file, &prepared);
    assert_exact_budget(&prepared.authorization, &prepared.pricing);
    assert_human_output(&prepared);

    assert_eq!(
        state_file_bytes(prepared.fixture.state.path()),
        files_before
    );
    assert_eq!(all_hub_state(prepared.fixture.state.path()), rows_before);
    let error = prepared.listener.accept().expect_err("zero loopback calls");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}

#[test]
fn malformed_noncanonical_oversize_and_options_fail_before_hub() {
    let state = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    let authorization = golden_scheduled_authorization();
    let pricing = build_pricing_with_real_core();
    fs::write(cwd.path().join("authorization.json"), &authorization).unwrap();
    fs::write(cwd.path().join("pricing.json"), &pricing).unwrap();

    reject_parser_options(state.path(), cwd.path());
    reject_artifact(
        state.path(),
        cwd.path(),
        "-",
        "pricing.json",
        b"{}",
        "invalid JSON",
    );
    reject_artifact(
        state.path(),
        cwd.path(),
        "-",
        "pricing.json",
        &[0xff],
        "must be UTF-8",
    );
    let mut noncanonical_authorization = authorization.clone();
    noncanonical_authorization.push(b'\n');
    reject_artifact(
        state.path(),
        cwd.path(),
        "-",
        "pricing.json",
        &noncanonical_authorization,
        "canonical",
    );
    let oversized_authorization =
        vec![b'x'; MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES + 1];
    reject_artifact(
        state.path(),
        cwd.path(),
        "-",
        "pricing.json",
        &oversized_authorization,
        "byte limit",
    );

    reject_pricing_inputs(state.path(), cwd.path(), &pricing);
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn missing_and_v14_hubs_are_not_created_or_migrated() {
    let state = TempDir::new().unwrap();
    fs::set_permissions(state.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let cwd = TempDir::new().unwrap();
    fs::write(
        cwd.path().join("authorization.json"),
        golden_scheduled_authorization(),
    )
    .unwrap();
    fs::write(
        cwd.path().join("pricing.json"),
        build_pricing_with_real_core(),
    )
    .unwrap();
    let output = invoke_with_stdin(
        state.path(),
        cwd.path(),
        &readiness_tail("request-missing", "authorization.json", "pricing.json"),
        &[],
    );
    assert_failure(&output, "require an existing database");
    assert!(!state.path().join("hub.sqlite3").exists());

    let prepared = PreparedReadiness::new();
    write_artifacts(&prepared);
    let database = prepared.fixture.state.path().join("hub.sqlite3");
    downgrade_empty_v15_sidecar(&database);
    checkpoint(prepared.fixture.state.path());
    let before = state_file_bytes(prepared.fixture.state.path());
    let output = invoke_readiness(&prepared, "authorization.json", "pricing.json", &[]);
    assert_failure(&output, "current schema version 18");
    assert_eq!(state_file_bytes(prepared.fixture.state.path()), before);
    let connection = Connection::open(database).unwrap();
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 14);
}

struct PreparedReadiness {
    fixture: Fixture,
    listener: TcpListener,
    poison_url: String,
    graph_run_id: String,
    request_id: String,
    authorization: Vec<u8>,
    pricing: Vec<u8>,
}

impl PreparedReadiness {
    fn new() -> Self {
        let (listener, poison_url) = loopback_sentinel();
        let pricing = build_pricing_with_real_core();
        assert_eq!(text(&json(&pricing)["pricing_snapshot_sha256"]), PRICING);
        let fixture = Fixture::new();
        let graph_run_id = prepare_run(&fixture, "scheduled-readiness-source-run");
        let control = export_control(&fixture, &graph_run_id);
        let schedule = build_schedule(&control);
        admit_schedule(&fixture, &graph_run_id, &schedule);
        let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
        let candidate = build_candidate(&control, &schedule_sha256);
        let admitted = admit_candidate(
            &fixture,
            &graph_run_id,
            "scheduled-readiness-candidate",
            &candidate,
        );
        let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
        let request_id = prepare_provider_request(
            &fixture,
            &contract_id,
            "scheduled-readiness-provider-request",
        );
        let release = export_scheduled_release_control(&fixture, &request_id);
        let authorization = authorize_scheduled_with_core(&release);
        Self {
            fixture,
            listener,
            poison_url,
            graph_run_id,
            request_id,
            authorization,
            pricing,
        }
    }
}

fn build_pricing_with_real_core() -> Vec<u8> {
    let output = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .args([
            "run",
            "./cmd/forge",
            "graph-node-pricing-snapshot",
            "--model",
            MODEL,
            "--input-usd-micros-per-token-unit",
            "1000000",
            "--output-usd-micros-per-token-unit",
            "976561523",
            "--max-input-tokens",
            "1",
        ])
        .output()
        .expect("build pricing with real Core");
    assert_cli_success(&output);
    assert!(!output.stdout.ends_with(b"\n"));
    output.stdout
}

fn verify_files(prepared: &PreparedReadiness) -> Value {
    write_artifacts(prepared);
    successful_json(&invoke_readiness(
        prepared,
        "authorization.json",
        "pricing.json",
        &[],
    ))
}

fn verify_auth_stdin(prepared: &PreparedReadiness) -> Value {
    successful_json(&invoke_readiness(
        prepared,
        "-",
        "pricing.json",
        &prepared.authorization,
    ))
}

fn verify_pricing_stdin(prepared: &PreparedReadiness) -> Value {
    successful_json(&invoke_readiness(
        prepared,
        "authorization.json",
        "-",
        &prepared.pricing,
    ))
}

fn invoke_readiness(
    prepared: &PreparedReadiness,
    authorization_source: &str,
    pricing_source: &str,
    stdin: &[u8],
) -> Output {
    let tail = [
        "readiness",
        "verify",
        &prepared.request_id,
        "--authorization",
        authorization_source,
        "--pricing",
        pricing_source,
    ];
    let mut child = poisoned_command(prepared, &tail)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn readiness");
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().expect("wait for readiness")
}

fn poisoned_command(prepared: &PreparedReadiness, tail: &[&str]) -> Command {
    let mut command = scheduled_provider_request_command(&prepared.fixture, tail);
    command
        .env("OPENAI_API_KEY", POISON)
        .env("OPENAI_BASE_URL", &prepared.poison_url)
        .env("HTTP_PROXY", &prepared.poison_url)
        .env("HTTPS_PROXY", &prepared.poison_url)
        .env("ALL_PROXY", &prepared.poison_url)
        .env("NO_PROXY", "");
    command
}

fn assert_readiness_output(value: &Value, prepared: &PreparedReadiness) {
    assert_eq!(
        value["type"],
        "group_agent_scheduled_node_dispatch_readiness_verified"
    );
    assert_eq!(value["readiness_validated_against_current_state"], true);
    assert_eq!(value["authorization_validated_against_current_state"], true);
    assert_eq!(value["exact_registered_destination_validated"], true);
    assert_eq!(value["exact_pricing_snapshot_validated"], true);
    assert_eq!(value["pricing_upper_bound_within_frozen_budget"], true);
    assert_eq!(value["pricing_provenance"], "operator_asserted");
    assert_eq!(value["vendor_attestation_present"], false);
    assert_future_only_scheduled_authorization_decisions(value);
    assert_eq!(value["readiness"]["graph_run_id"], prepared.graph_run_id);
    assert_eq!(
        value["readiness"]["scheduled_provider_request_id"],
        prepared.request_id
    );
    assert_false_scheduled_readiness_effects(value);
    assert_redacted(&serde_json::to_string(value).unwrap());
}

fn assert_human_output(prepared: &PreparedReadiness) {
    let output = human_readiness(prepared);
    assert_cli_success(&output);
    let output = String::from_utf8(output.stdout).unwrap();
    assert!(output.contains("exact pricing artifact validated"));
    assert!(output.contains("operator-asserted, not vendor-attested"));
    assert!(output.contains("readiness only"));
    assert_redacted(&output);
}

fn human_readiness(prepared: &PreparedReadiness) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    command
        .current_dir(prepared.fixture.cwd.path())
        .args(["--state-dir", path_text(prepared.fixture.state.path())])
        .args(readiness_prefix())
        .args([
            "verify",
            &prepared.request_id,
            "--authorization",
            "authorization.json",
            "--pricing",
            "pricing.json",
        ])
        .env("OPENAI_API_KEY", POISON)
        .env("OPENAI_BASE_URL", &prepared.poison_url)
        .env("HTTPS_PROXY", &prepared.poison_url)
        .env("NO_PROXY", "");
    command.output().expect("run human readiness")
}

fn assert_exact_budget(authorization: &[u8], pricing: &[u8]) {
    let authorization = json(authorization);
    let pricing = json(pricing);
    let input = ceiling_cost(1, 1_000_000);
    let output = ceiling_cost(1_024, 976_561_523);
    assert_eq!(input + output, EXACT_BUDGET);
    assert_eq!(authorization["budgets"]["max_output_tokens"], 1_024);
    assert_eq!(
        authorization["budgets"]["max_cost_usd_micros"],
        EXACT_BUDGET
    );
    assert_eq!(pricing["max_input_tokens"], 1);
    assert_eq!(pricing["pricing_snapshot_sha256"], PRICING);
}

fn ceiling_cost(tokens: u64, rate: u64) -> u64 {
    tokens.saturating_mul(rate).div_ceil(1_000_000)
}

fn reject_parser_options(state: &Path, cwd: &Path) {
    let prefix = readiness_prefix();
    prehub(state, cwd, &prefix, &[], "command is required");
    let mut missing = prefix.clone();
    missing.extend(["verify", "request-1", "--authorization", "-"]);
    prehub(state, cwd, &missing, &[], "requires --pricing");
    let mut duplicate = readiness_tail("request-1", "authorization.json", "pricing.json");
    duplicate.extend(["--pricing", "again.json"]);
    prehub(state, cwd, &duplicate, &[], "more than once");
    let mut unknown = readiness_tail("request-1", "authorization.json", "pricing.json");
    unknown.push("--send");
    prehub(state, cwd, &unknown, &[], "unknown");
    prehub(
        state,
        cwd,
        &readiness_tail("request-1", "-", "-"),
        &[],
        "cannot both read from stdin",
    );
    let mut keyed = vec!["--idempotency-key", "forbidden"];
    keyed.extend(readiness_tail(
        "request-1",
        "authorization.json",
        "pricing.json",
    ));
    prehub(state, cwd, &keyed, &[], "only valid for mutating commands");
    for selector in [["-C", "."], ["--group", "group-1"]] {
        let mut args = selector.to_vec();
        args.extend(readiness_tail(
            "request-1",
            "authorization.json",
            "pricing.json",
        ));
        prehub(state, cwd, &args, &[], "selectors are not valid");
    }
}

fn reject_pricing_inputs(state: &Path, cwd: &Path, pricing: &[u8]) {
    reject_artifact(state, cwd, "authorization.json", "-", b"{}", "invalid JSON");
    reject_artifact(
        state,
        cwd,
        "authorization.json",
        "-",
        &[0xff],
        "must be UTF-8",
    );
    let mut noncanonical = pricing.to_vec();
    noncanonical.push(b'\n');
    reject_artifact(
        state,
        cwd,
        "authorization.json",
        "-",
        &noncanonical,
        "canonical",
    );
    let oversized = vec![b'x'; MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES + 1];
    reject_artifact(
        state,
        cwd,
        "authorization.json",
        "-",
        &oversized,
        "byte limit",
    );
}

fn reject_artifact(
    state: &Path,
    cwd: &Path,
    authorization_source: &str,
    pricing_source: &str,
    stdin: &[u8],
    expected: &str,
) {
    let args = readiness_tail("request-1", authorization_source, pricing_source);
    prehub(state, cwd, &args, stdin, expected);
}

fn prehub(state: &Path, cwd: &Path, args: &[&str], stdin: &[u8], expected: &str) {
    let output = invoke_with_stdin(state, cwd, args, stdin);
    assert_failure(&output, expected);
    assert!(!state.join("hub.sqlite3").exists());
}

fn assert_redacted(output: &str) {
    for private in [
        TASK_SECRET,
        POISON,
        OFFICIAL_ENDPOINT,
        MODEL,
        PRICING,
        "976561523",
        "1000000",
    ] {
        assert!(!output.contains(private), "output leaked {private}");
    }
}

fn write_artifacts(prepared: &PreparedReadiness) {
    fs::write(
        prepared.fixture.cwd.path().join("authorization.json"),
        &prepared.authorization,
    )
    .unwrap();
    fs::write(
        prepared.fixture.cwd.path().join("pricing.json"),
        &prepared.pricing,
    )
    .unwrap();
}

fn loopback_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback sentinel");
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    (listener, format!("http://{address}"))
}

fn json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("canonical JSON")
}

fn forge_core_dir() -> PathBuf {
    repository_root().join("forge-core")
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}
