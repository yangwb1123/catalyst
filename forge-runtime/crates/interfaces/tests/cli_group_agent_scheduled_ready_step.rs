use std::{
    fs,
    io::ErrorKind,
    net::TcpListener,
    path::Path,
    process::{Command, Output},
};

use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

#[allow(dead_code, clippy::duplicate_mod)]
mod cli_group_agent_scheduled_node_contract_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod cli_group_agent_scheduled_node_provider_request_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod group_agent_graph_run_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod group_agent_graph_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod scheduled_graph_reconcile_cli_support;

use cli_group_agent_scheduled_node_contract_support::{
    admit_candidate, admit_schedule, build_candidate, build_schedule, export_control, json,
    prepare_run,
};
use cli_group_agent_scheduled_node_provider_request_support::{
    all_hub_state, checkpoint, prepare_provider_request,
};
use group_agent_graph_run_support::{Fixture, TASK_SECRET, WORKSPACE_SECRET, command};
use group_agent_graph_support::{path_text, successful_json, text};
use scheduled_graph_reconcile_cli_support::{loopback_sentinel, shared_core};

const PRICING: &str = "48f3531a7d71015453dc27a71bd0f17efbaf68ddfcff04461bd5d01b52cade8d";
const OFFICIAL_ENDPOINT: &str = "https://api.openai.com/v1/responses";
const MODEL: &str = "private-scheduled-contract-model";
const PRICING_RATE: &str = "976561523";

#[test]
fn help_publishes_the_exact_ready_step_boundary() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .arg("--help")
        .output()
        .expect("run CLI help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 help");
    for expected in [
        "group graph run step GRAPH_RUN_ID",
        "--expected-provider-request-id ID --expected-ready-authorization-sha256 SHA256",
        "--confirm-off-machine [--confirm-predecessor-content] [--include-result]",
        "Re-entry never resends",
        "--include-result explicitly reveals stored provider output",
    ] {
        assert!(
            stdout.contains(expected),
            "missing help fragment: {expected}"
        );
    }
}

#[test]
fn process_parser_rejects_and_redacts_unowned_credentials() {
    let secret = "private-inline-credential-must-not-echo";
    let option = format!("--api-key={secret}");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "group",
            "graph",
            "run",
            "step",
            "graph-run-1",
            "--expected-provider-request-id",
            "provider-request-1",
            "--expected-ready-authorization-sha256",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--pricing",
            "pricing.json",
            "--core-bin",
            "/opt/forge/bin/forge",
            "--core-bin-sha256",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            &option,
        ])
        .output()
        .expect("run CLI parser");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(stderr.contains("unknown group graph run step option"));
    assert!(!stderr.contains(secret));
}

#[test]
fn missing_consent_fails_before_core_or_private_pricing_is_read() {
    let private_path = "private-pricing-path-must-not-echo";
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "group",
            "graph",
            "run",
            "step",
            "graph-run-1",
            "--expected-provider-request-id",
            "provider-request-1",
            "--expected-ready-authorization-sha256",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--pricing",
            private_path,
            "--core-bin",
            "/nonexistent/private-core-path",
            "--core-bin-sha256",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        ])
        .output()
        .expect("run CLI consent refusal");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 error");
    assert!(stderr.contains("--confirm-off-machine consent is required"));
    assert!(!stderr.contains(private_path));
    assert!(!stderr.contains("Core executable"));
}

#[test]
fn real_public_step_reaches_only_the_offline_credential_boundary() {
    let prepared = PreparedStep::new();
    checkpoint(prepared.fixture.state.path());
    let hub_before = all_hub_state(prepared.fixture.state.path());

    let output = prepared.invoke();
    assert_credential_boundary(&output);
    assert_private_source_not_emitted(&output);

    assert_eq!(all_hub_state(prepared.fixture.state.path()), hub_before);
    assert_no_owner_sidecars(prepared.fixture.state.path());
    prepared.fixture.assert_workspace_unchanged();
    assert_no_network(&prepared.listener);
}

#[test]
fn real_public_step_terminalizes_offline_transport_and_reentry_never_resends() {
    let prepared = PreparedStep::new();
    let first = prepared.invoke_network_denied(false, "offline-test-key");
    let first = successful_json(&first);
    assert_terminalized_effect_facts(&first);
    assert_eq!(first["node_id"], "frontend");
    assert_one_stored_lifecycle(prepared.fixture.state.path());
    assert!(
        Path::new(&prepared.network_guard_marker).is_file(),
        "the offline network guard did not intercept the provider path"
    );
    let blocked_calls = fs::read(&prepared.network_guard_marker).expect("read guard marker");
    assert_eq!(first["metadata_only"], true);
    assert!(first.get("result_text").is_none());

    let second = prepared.invoke_network_denied(true, CREDENTIAL_POISON);
    let second = successful_json(&second);
    assert_reentry_effect_facts(&second);
    assert_one_stored_lifecycle(prepared.fixture.state.path());
    assert_eq!(second["result_included"], true);
    assert_eq!(second["result_text"], "");
    assert_eq!(
        fs::read(&prepared.network_guard_marker).expect("reread guard marker"),
        blocked_calls,
        "re-entry attempted a second network operation"
    );
    assert_no_owner_sidecars(prepared.fixture.state.path());
    prepared.fixture.assert_workspace_unchanged();
    assert_no_network(&prepared.listener);
}

struct PreparedStep {
    fixture: Fixture,
    listener: TcpListener,
    proxy: String,
    graph_run_id: String,
    provider_request_id: String,
    authorization_sha256: String,
    pricing_path: String,
    _network_guard: TempDir,
    network_guard_path: String,
    network_guard_marker: String,
}

impl PreparedStep {
    fn new() -> Self {
        let (listener, sentinel) = loopback_sentinel();
        let fixture = Fixture::new();
        let (graph_run_id, contract_id) = prepare_initial_ready_source(&fixture);
        let provider_request_id = prepare_provider_request(
            &fixture,
            &contract_id,
            "scheduled-ready-step-provider-request",
        );
        let pricing_path = write_exact_pricing(&fixture);
        let authorization = authorize_with_public_ready_release(&fixture, &graph_run_id);
        let (network_guard, network_guard_path, network_guard_marker) = compile_network_guard();
        assert_eq!(authorization["endpoint"], OFFICIAL_ENDPOINT);
        assert_eq!(authorization["execution_ordinal"], 0);
        assert_eq!(
            authorization["scheduled_provider_request_id"],
            provider_request_id
        );
        Self {
            fixture,
            listener,
            proxy: sentinel.replacen("https://", "http://", 1),
            graph_run_id,
            provider_request_id,
            authorization_sha256: text(&authorization["authorization_sha256"]),
            pricing_path,
            _network_guard: network_guard,
            network_guard_path,
            network_guard_marker,
        }
    }

    fn invoke(&self) -> Output {
        let core = shared_core();
        command(
            self.fixture.state.path(),
            self.fixture.cwd.path(),
            &self.args(path_text(&core.path), &core.sha256),
        )
        .env_remove("OPENAI_API_KEY")
        .env_remove("OPENAI_BASE_URL")
        .env("HTTP_PROXY", &self.proxy)
        .env("HTTPS_PROXY", &self.proxy)
        .env("ALL_PROXY", &self.proxy)
        .env("NO_PROXY", "")
        .output()
        .expect("run public scheduled ready-node step")
    }

    fn invoke_network_denied(&self, include_result: bool, credential: &str) -> Output {
        let core = shared_core();
        let mut args = self.args(path_text(&core.path), &core.sha256);
        if include_result {
            args.push("--include-result");
        }
        command(self.fixture.state.path(), self.fixture.cwd.path(), &args)
            .env("OPENAI_API_KEY", credential)
            .env("LD_PRELOAD", &self.network_guard_path)
            .env("FORGE_NETWORK_GUARD_MARKER", &self.network_guard_marker)
            .env_remove("OPENAI_BASE_URL")
            .env_remove("HTTP_PROXY")
            .env_remove("HTTPS_PROXY")
            .env_remove("ALL_PROXY")
            .env_remove("NO_PROXY")
            .output()
            .expect("run network-denied public scheduled ready-node step")
    }

    fn args<'a>(&'a self, core: &'a str, digest: &'a str) -> Vec<&'a str> {
        vec![
            "group",
            "graph",
            "run",
            "step",
            &self.graph_run_id,
            "--expected-provider-request-id",
            &self.provider_request_id,
            "--expected-ready-authorization-sha256",
            &self.authorization_sha256,
            "--pricing",
            &self.pricing_path,
            "--core-bin",
            core,
            "--core-bin-sha256",
            digest,
            "--confirm-off-machine",
        ]
    }
}

fn prepare_initial_ready_source(fixture: &Fixture) -> (String, String) {
    let graph_run_id = prepare_run(fixture, "scheduled-ready-step-source-run");
    let control = export_control(fixture, &graph_run_id);
    let schedule = build_schedule(&control);
    admit_schedule(fixture, &graph_run_id, &schedule);
    let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
    let candidate = build_candidate(&control, &schedule_sha256);
    let admitted = admit_candidate(
        fixture,
        &graph_run_id,
        "scheduled-ready-step-candidate",
        &candidate,
    );
    let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
    (graph_run_id, contract_id)
}

fn write_exact_pricing(fixture: &Fixture) -> String {
    let core = shared_core();
    let output = Command::new(&core.path)
        .args([
            "graph-node-pricing-snapshot",
            "--model",
            MODEL,
            "--input-usd-micros-per-token-unit",
            "1000000",
            "--output-usd-micros-per-token-unit",
            PRICING_RATE,
            "--max-input-tokens",
            "1",
        ])
        .env_clear()
        .output()
        .expect("build exact pricing with pinned Go Core");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let pricing = json(&output.stdout);
    assert_eq!(pricing["pricing_snapshot_sha256"], PRICING);
    let path = fixture.cwd.path().join("ready-step-pricing.json");
    fs::write(&path, output.stdout).expect("write exact pricing fixture");
    path_text(&path).to_owned()
}

fn authorize_with_public_ready_release(fixture: &Fixture, graph_run_id: &str) -> Value {
    let core = shared_core();
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "ready-release",
            graph_run_id,
            "--core-bin",
            path_text(&core.path),
            "--core-bin-sha256",
            &core.sha256,
        ],
    )
    .env_remove("OPENAI_API_KEY")
    .env_remove("OPENAI_BASE_URL")
    .output()
    .expect("authorize ready source through public CLI");
    successful_json(&output)["authorization"].clone()
}

fn assert_credential_boundary(output: &Output) {
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("scheduled ready-node credential is unavailable"),
        "stderr was {stderr:?}"
    );
}

fn assert_terminalized_effect_facts(value: &Value) {
    assert_eq!(
        value["invocation_disposition"], "terminalized",
        "unexpected public step output: {value}"
    );
    assert_eq!(value["lifecycle_status"], "terminalized");
    assert_eq!(
        value["core_trust_boundary"]["stored_terminal_receipt_validated"],
        true
    );
    assert_eq!(value["classification"], "transport_error");
    assert_eq!(value["lane_active"], false);
    let facts = &value["runtime_effect_facts"];
    assert_eq!(facts["preclaim_effects_observation"], "performed");
    assert_eq!(facts["project_lane_claimed_this_invocation"], true);
    assert_eq!(facts["provider_stream_polled_this_invocation"], true);
    assert_eq!(facts["logical_hub_mutated_this_invocation"], true);
    assert_eq!(facts["terminal_receipt_recorded_this_invocation"], true);
    assert_eq!(facts["remote_provider_request_observation"], "not_attested");
}

fn assert_reentry_effect_facts(value: &Value) {
    assert_eq!(value["invocation_disposition"], "already_claimed");
    assert_eq!(value["lifecycle_status"], "terminalized");
    assert_eq!(
        value["core_trust_boundary"]["stored_terminal_receipt_validated"],
        true
    );
    let facts = &value["runtime_effect_facts"];
    assert_eq!(facts["preclaim_effects_observation"], "not_performed");
    assert_eq!(facts["credential_read_this_invocation"], false);
    assert_eq!(facts["provider_constructed_this_invocation"], false);
    assert_eq!(facts["owner_sidecar_created_this_invocation"], false);
    assert_eq!(facts["project_lane_claimed_this_invocation"], false);
    assert_eq!(facts["provider_stream_polled_this_invocation"], false);
    assert_eq!(facts["logical_hub_mutated_this_invocation"], false);
    assert_eq!(facts["terminal_receipt_recorded_this_invocation"], false);
}

fn compile_network_guard() -> (TempDir, String, String) {
    let directory = tempfile::tempdir().expect("network guard directory");
    let source = directory.path().join("deny-network.c");
    let library = directory.path().join("deny-network.so");
    let marker = directory.path().join("network-blocked");
    fs::write(&source, NETWORK_GUARD_SOURCE).expect("write network guard source");
    let output = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-o"])
        .arg(&library)
        .arg(&source)
        .output()
        .expect("compile network guard");
    assert!(
        output.status.success(),
        "network guard compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        directory,
        path_text(&library).to_owned(),
        path_text(&marker).to_owned(),
    )
}

const CREDENTIAL_POISON: &str = "reentry-must-not-read-this\r\nx-private: rejected";
const NETWORK_GUARD_SOURCE: &[u8] = br#"
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <unistd.h>

static void mark_blocked(void) {
    const char *path = getenv("FORGE_NETWORK_GUARD_MARKER");
    if (path == NULL) return;
    int descriptor = open(path, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (descriptor < 0) return;
    (void)write(descriptor, "blocked\n", 8);
    (void)close(descriptor);
}

int getaddrinfo(const char *node, const char *service,
                const struct addrinfo *hints, struct addrinfo **result) {
    (void)node; (void)service; (void)hints; (void)result;
    mark_blocked();
    return EAI_AGAIN;
}

int connect(int socket, const struct sockaddr *address, socklen_t length) {
    (void)socket; (void)address; (void)length;
    mark_blocked();
    errno = ENETUNREACH;
    return -1;
}
"#;

fn assert_private_source_not_emitted(output: &Output) {
    let combined = [&output.stdout[..], &output.stderr[..]].concat();
    let rendered = String::from_utf8_lossy(&combined);
    for private in [
        TASK_SECRET,
        WORKSPACE_SECRET,
        OFFICIAL_ENDPOINT,
        MODEL,
        PRICING,
        PRICING_RATE,
    ] {
        assert!(!rendered.contains(private), "output leaked private source");
    }
}

fn assert_no_owner_sidecars(state: &Path) {
    let directory = state.join("scheduled-executor-owners");
    if !directory.exists() {
        return;
    }
    assert!(
        fs::read_dir(directory)
            .expect("read scheduled executor owner directory")
            .next()
            .is_none(),
        "credential refusal left an owner sidecar"
    );
}

fn assert_one_stored_lifecycle(state: &Path) {
    let counts = Connection::open(state.join("hub.sqlite3"))
        .expect("open Hub for lifecycle count")
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(status='terminalized' AND terminal_receipt_json IS NOT NULL), 0) FROM group_agent_graph_scheduled_node_dispatch_lifecycles",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("count stored lifecycles and terminal receipts");
    assert_eq!(counts, (1, 1));
}

fn assert_no_network(listener: &TcpListener) {
    let error = listener
        .accept()
        .expect_err("unexpected network connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}
