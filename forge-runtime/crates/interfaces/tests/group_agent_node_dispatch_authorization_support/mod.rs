use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use serde_json::Value;

use super::{
    group_agent_graph_run_support::{
        Fixture, TASK_SECRET, WORKSPACE_SECRET, command, human_command, invoke_with_stdin, run_json,
    },
    group_agent_graph_support::{successful_json, text},
};

const MODEL: &str = "private-release-authorization-model";
const PREPARE_KEY: &str = "private-release-dispatch-prepare-key";
const CREDENTIAL_MARKER: &str = "credential-must-not-be-read-during-authorization-verification";
const CREDENTIAL_SENTINEL: &str =
    "credential-must-not-be-read-during-authorization-verification\r\nx-private-header: rejected";
const PRICING: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
pub(super) fn prepare_run(fixture: &Fixture) -> String {
    let prepared = successful_json(&fixture.prepare(&fixture.plan(), "authorization-source-run"));
    text(&prepared["inspection"]["run"]["graph_run_id"])
}

pub(super) fn export_scheduler_control(fixture: &Fixture, graph_run_id: &str) -> Vec<u8> {
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "control", "export", graph_run_id],
    )
    .output()
    .expect("export scheduler control");
    assert_success(&output);
    output.stdout
}

pub(super) fn build_contract_with_real_core(control: &[u8], endpoint: &str) -> Vec<u8> {
    build_contract_with_pricing(control, endpoint, PRICING)
}

pub(super) fn build_contract_with_pricing(
    control: &[u8],
    endpoint: &str,
    pricing_sha256: &str,
) -> Vec<u8> {
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
            pricing_sha256,
            "--max-result-bytes",
            "16384",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-core contract builder");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(control)
        .expect("write control");
    let output = child.wait_with_output().expect("wait for contract");
    assert_success(&output);
    output.stdout
}

pub(super) fn admit_contract(fixture: &Fixture, graph_run_id: &str, contract: &[u8]) {
    let output = invoke_with_stdin(
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
            "authorization-contract-admission",
        ],
        contract,
    );
    successful_json(&output);
}

pub(super) fn prepare_dispatch_request(fixture: &Fixture, graph_run_id: &str) {
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
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
    )
    .output()
    .expect("prepare request");
    successful_json(&output);
}

pub(super) fn export_release_control(fixture: &Fixture, graph_run_id: &str, json: bool) -> Vec<u8> {
    let output = if json {
        command(
            fixture.state.path(),
            fixture.cwd.path(),
            &[
                "group",
                "graph",
                "run",
                "dispatch",
                "release-control",
                "export",
                graph_run_id,
            ],
        )
        .env("OPENAI_API_KEY", CREDENTIAL_SENTINEL)
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("export release control")
    } else {
        human_command(
            fixture.state.path(),
            fixture.cwd.path(),
            &[
                "group",
                "graph",
                "run",
                "dispatch",
                "release-control",
                "export",
                graph_run_id,
            ],
        )
    };
    assert_success(&output);
    output.stdout
}

pub(super) fn authorize_with_real_core(control: &[u8]) -> Vec<u8> {
    let mut child = Command::new("go")
        .current_dir(forge_core_dir())
        .env("GOTOOLCHAIN", "local")
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .args([
            "run",
            "./cmd/forge",
            "graph-node-dispatch-authorize",
            "--control",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-core authorization builder");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(control)
        .expect("write release control");
    let output = child.wait_with_output().expect("wait for authorization");
    assert_success(&output);
    output.stdout
}

pub(super) fn verify_stdin(fixture: &Fixture, graph_run_id: &str, authorization: &[u8]) -> Value {
    let output = invoke_raw(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "authorization",
            "verify",
            graph_run_id,
            "--authorization",
            "-",
        ],
        authorization,
    );
    successful_json(&output)
}

pub(super) fn verify_file(fixture: &Fixture, graph_run_id: &str, authorization: &[u8]) -> Value {
    let path = fixture.cwd.path().join("authorization.json");
    fs::write(&path, authorization).expect("write authorization fixture");
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "authorization",
            "verify",
            graph_run_id,
            "--authorization",
            path_text(&path),
        ],
    )
    .env("OPENAI_API_KEY", CREDENTIAL_SENTINEL)
    .output()
    .expect("verify authorization file");
    successful_json(&output)
}

pub(super) fn assert_release_control_is_explicitly_private(bytes: &[u8], endpoint: &str) {
    let control: Value = serde_json::from_slice(bytes).expect("release control JSON");
    assert_eq!(control["graph_run"]["v"], 3);
    assert_eq!(
        control["graph_run"]["status"],
        "awaiting_dispatch_authorization"
    );
    let text = String::from_utf8_lossy(bytes);
    for private in [TASK_SECRET, endpoint, MODEL, PRICING] {
        assert!(text.contains(private), "explicit export omitted {private}");
    }
    assert!(!text.contains(WORKSPACE_SECRET));
    assert!(!text.contains(CREDENTIAL_MARKER));
}

pub(super) fn assert_verified(verified: &Value, authorization: &Value, endpoint: &str) {
    assert_eq!(
        verified["type"],
        "group_agent_node_dispatch_authorization_verified"
    );
    assert_eq!(verified["metadata_only"], true);
    assert_eq!(verified["authorization_validated"], true);
    assert_eq!(verified["dispatch_authority_release_authorized"], true);
    assert_eq!(verified["dispatch_authority_released"], false);
    assert_eq!(
        verified["authorization"]["authorization_id"],
        authorization["authorization_id"]
    );
    for field in effect_fields() {
        assert_eq!(verified[field], false, "{field} must remain false");
    }
    assert_private_verification_output(verified, endpoint);
}

fn assert_private_verification_output(verified: &Value, endpoint: &str) {
    let encoded = serde_json::to_string(verified).expect("verified JSON");
    for private in [
        TASK_SECRET,
        WORKSPACE_SECRET,
        endpoint,
        MODEL,
        PRICING,
        PREPARE_KEY,
        CREDENTIAL_MARKER,
    ] {
        assert!(
            !encoded.contains(private),
            "verification output leaked {private}"
        );
    }
    for field in [
        "endpoint",
        "model",
        "pricing_snapshot_sha256",
        "provider_request_json",
        "project_id",
        "project_lane_sha256",
        "destination_sha256",
        "request_body_sha256",
    ] {
        assert!(verified.get(field).is_none());
        assert!(verified["authorization"].get(field).is_none());
    }
}

pub(super) fn reject_noncanonical_without_writes(
    fixture: &Fixture,
    graph_run_id: &str,
    authorization: &[u8],
    database_before: &[u8],
) {
    let mut noncanonical = authorization.to_vec();
    noncanonical.push(b'\n');
    let output = invoke_raw(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "authorization",
            "verify",
            graph_run_id,
            "--authorization",
            "-",
        ],
        &noncanonical,
    );
    assert_failure(&output);
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains(MODEL));
    assert_eq!(database_bytes(fixture), database_before);
}

pub(super) fn assert_run_still_waits_for_authority(fixture: &Fixture, graph_run_id: &str) {
    let shown = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &["group", "graph", "run", "show", graph_run_id],
    );
    let run = &shown["inspection"]["run"];
    assert_eq!(run["v"], 3);
    assert_eq!(run["status"], "awaiting_dispatch_authorization");
    assert_eq!(run["last_event_seq"], 3);
    assert_eq!(run["dispatch_authority_released"], false);
}

pub(super) fn invoke_raw(state: &Path, cwd: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .current_dir(cwd)
        .env("OPENAI_API_KEY", CREDENTIAL_SENTINEL)
        .env_remove("ANTHROPIC_API_KEY")
        .args(["--state-dir", path_text(state), "--json"])
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait for CLI")
}

fn effect_fields() -> [&'static str; 19] {
    [
        "consent_obtained",
        "fresh_off_machine_consent_obtained",
        "credential_read",
        "credential_preflight_performed",
        "execution_performed",
        "model_used",
        "provider_constructed",
        "provider_used",
        "network_invoked",
        "network_accessed",
        "project_lane_claimed",
        "graph_advanced",
        "workspace_accessed",
        "tools_used",
        "result_produced",
        "result_persisted",
        "conversation_or_prompt_written",
        "memory_written",
        "writeback_performed",
    ]
}

pub(super) fn database_bytes(fixture: &Fixture) -> Vec<u8> {
    fs::read(fixture.state.path().join("hub.sqlite3")).expect("read Hub database")
}

pub(super) fn state_file_bytes(fixture: &Fixture) -> BTreeMap<String, Vec<u8>> {
    state_directory_file_bytes(fixture.state.path())
}

pub(super) fn state_directory_file_bytes(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .expect("read state directory")
        .map(|entry| {
            let entry = entry.expect("state entry");
            let name = entry
                .file_name()
                .into_string()
                .expect("UTF-8 state filename");
            let bytes = fs::read(entry.path()).expect("read state file");
            (name, bytes)
        })
        .collect()
}

pub(super) fn assert_no_sqlite_sidecars(state: &Path) {
    for name in ["hub.sqlite3-wal", "hub.sqlite3-shm", "hub.sqlite3-journal"] {
        assert!(
            !state.join(name).exists(),
            "unexpected SQLite sidecar {name}"
        );
    }
}

pub(super) fn loopback_sentinel() -> (TcpListener, String) {
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

fn forge_core_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("forge-core")
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temporary path")
}

pub(super) fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

pub(super) fn assert_failure(output: &Output) {
    assert!(!output.status.success(), "command unexpectedly succeeded");
}
