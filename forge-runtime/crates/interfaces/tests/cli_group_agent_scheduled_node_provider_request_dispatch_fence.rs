#![cfg(target_os = "linux")]

use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

use forge_runtime_domain::{
    GroupAgentNodeDispatchAuthorization, group_agent_node_dispatch_authorization_id,
};

#[allow(dead_code)]
mod cli_group_agent_scheduled_node_contract_support;
#[allow(dead_code)]
mod group_agent_graph_run_support;
mod group_agent_graph_support;
#[allow(dead_code)]
mod group_agent_node_dispatch_authorization_support;

use cli_group_agent_scheduled_node_contract_support::{
    CREDENTIAL_MARKER, admit_candidate, admit_schedule, build_candidate_at, build_schedule,
    export_control, json, prepare_run,
};
use group_agent_graph_run_support::{Fixture, command};
use group_agent_graph_support::{path_text, successful_json, text};

const PRICING: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

#[test]
fn legacy_execute_rejects_scheduled_sidecar_before_starting_core() {
    let (listener, endpoint) = loopback_sentinel();
    let fixture = Fixture::new();
    let graph_run_id = prepare_scheduled_request(&fixture, &endpoint);
    let authorization = fixture.cwd.path().join("irrelevant-authorization.json");
    let pricing = fixture.cwd.path().join("irrelevant-pricing.json");
    fs::write(&authorization, b"{}").expect("authorization fixture");
    fs::write(&pricing, b"{}").expect("pricing fixture");
    let (core, core_sha256, marker) = sentinel_core(fixture.cwd.path());
    let hub_before = state_bytes(fixture.state.path());

    let output = legacy_execute(
        &fixture,
        &graph_run_id,
        &authorization,
        &pricing,
        &core,
        &core_sha256,
    );

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("has no prepared dispatch request"),
        "{error}"
    );
    assert!(!marker.exists(), "legacy execute started the Core sentinel");
    assert_eq!(state_bytes(fixture.state.path()), hub_before);
    fixture.assert_workspace_unchanged();
    assert_eq!(
        listener
            .accept()
            .expect_err("no provider connection")
            .kind(),
        ErrorKind::WouldBlock
    );
}

#[test]
fn legacy_authorization_and_readiness_reject_scheduled_sidecar() {
    let (listener, endpoint) = loopback_sentinel();
    let fixture = Fixture::new();
    let graph_run_id = prepare_scheduled_request(&fixture, &endpoint);
    let authorization = authorization_bound_to(&graph_run_id);
    let authorization_path = fixture.cwd.path().join("authorization.json");
    let pricing_path = fixture.cwd.path().join("pricing.json");
    fs::write(&authorization_path, authorization).expect("authorization fixture");
    fs::write(&pricing_path, b"{}").expect("unreachable pricing fixture");
    let before = state_bytes(fixture.state.path());

    let authorization_output = legacy_command(
        &fixture,
        &[
            "authorization",
            "verify",
            &graph_run_id,
            "--authorization",
            path_text(&authorization_path),
        ],
    );
    assert_missing_legacy_request(&authorization_output, "authorization");
    let readiness_output = legacy_command(
        &fixture,
        &[
            "readiness",
            "verify",
            &graph_run_id,
            "--authorization",
            path_text(&authorization_path),
            "--pricing",
            path_text(&pricing_path),
        ],
    );
    assert_missing_legacy_request(&readiness_output, "readiness");

    assert_eq!(state_bytes(fixture.state.path()), before);
    fixture.assert_workspace_unchanged();
    assert_eq!(
        listener
            .accept()
            .expect_err("no provider connection")
            .kind(),
        ErrorKind::WouldBlock
    );
}

fn legacy_execute(
    fixture: &Fixture,
    run: &str,
    authorization: &Path,
    pricing: &Path,
    core: &Path,
    core_sha256: &str,
) -> std::process::Output {
    command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "dispatch",
            "execute",
            run,
            "--authorization",
            path_text(authorization),
            "--pricing",
            path_text(pricing),
            "--core-bin",
            path_text(core),
            "--core-bin-sha256",
            core_sha256,
            "--confirm-off-machine",
        ],
    )
    .env(
        "OPENAI_API_KEY",
        format!("{CREDENTIAL_MARKER}\r\nx-private: rejected"),
    )
    .output()
    .expect("legacy execute scheduled-only run")
}

fn legacy_command(fixture: &Fixture, tail: &[&str]) -> std::process::Output {
    let mut args = vec!["group", "graph", "run", "dispatch"];
    args.extend_from_slice(tail);
    command(fixture.state.path(), fixture.cwd.path(), &args)
        .env(
            "OPENAI_API_KEY",
            format!("{CREDENTIAL_MARKER}\r\nx-private: rejected"),
        )
        .output()
        .expect("run legacy verifier against scheduled sidecar")
}

fn assert_missing_legacy_request(output: &std::process::Output, operation: &str) {
    assert!(
        !output.status.success(),
        "legacy {operation} unexpectedly passed"
    );
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("has no prepared dispatch request"),
        "legacy {operation}: {error}"
    );
}

fn authorization_bound_to(graph_run_id: &str) -> String {
    use group_agent_node_dispatch_authorization_support as authorization_support;

    let fixture = Fixture::new();
    let source_run = authorization_support::prepare_run(&fixture);
    let control = authorization_support::export_scheduler_control(&fixture, &source_run);
    let contract = authorization_support::build_contract_with_real_core(
        &control,
        "https://api.openai.com/v1/responses",
    );
    authorization_support::admit_contract(&fixture, &source_run, &contract);
    authorization_support::prepare_dispatch_request(&fixture, &source_run);
    let release = authorization_support::export_release_control(&fixture, &source_run, true);
    let encoded = authorization_support::authorize_with_real_core(&release);
    let mut authorization: GroupAgentNodeDispatchAuthorization =
        serde_json::from_slice(&encoded).expect("decode generated authorization");
    authorization.graph_run_id = graph_run_id.into();
    authorization.authorization_id.clear();
    authorization.authorization_sha256.clear();
    let digest = authorization
        .expected_sha256()
        .expect("authorization digest");
    authorization.authorization_id = group_agent_node_dispatch_authorization_id(&digest);
    authorization.authorization_sha256 = digest;
    authorization
        .canonical_json()
        .expect("canonical rebound authorization")
}

fn prepare_scheduled_request(fixture: &Fixture, endpoint: &str) -> String {
    let graph_run_id = prepare_run(fixture, "scheduled-dispatch-fence-run");
    let control = export_control(fixture, &graph_run_id);
    let schedule = build_schedule(&control);
    admit_schedule(fixture, &graph_run_id, &schedule);
    let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
    let candidate = build_candidate_at(&control, &schedule_sha256, endpoint);
    let admitted = admit_candidate(
        fixture,
        &graph_run_id,
        "scheduled-dispatch-fence-candidate",
        &candidate,
    );
    let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "prepare",
            &contract_id,
            "--idempotency-key",
            "scheduled-dispatch-fence-request",
        ],
    )
    .output()
    .expect("prepare scheduled provider request");
    let prepared: Value = successful_json(&output);
    assert_eq!(prepared["disposition"], "created");
    graph_run_id
}

fn sentinel_core(directory: &Path) -> (PathBuf, String, PathBuf) {
    let marker = directory.join("core-was-started");
    let core = directory.join("sentinel-core");
    let script = format!(
        "#!/bin/sh\nprintf invoked > '{}'\nprintf 1",
        marker.display()
    );
    fs::write(&core, script.as_bytes()).expect("write Core sentinel");
    fs::set_permissions(&core, fs::Permissions::from_mode(0o700)).expect("executable sentinel");
    let core = core.canonicalize().expect("canonical sentinel path");
    let sha256 = format!(
        "{:x}",
        Sha256::digest(fs::read(&core).expect("read sentinel"))
    );
    (core, sha256, marker)
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

fn state_bytes(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(directory)
        .expect("state directory")
        .map(|entry| {
            let entry = entry.expect("state entry");
            let name = entry.file_name().into_string().expect("UTF-8 state name");
            (name, fs::read(entry.path()).expect("state bytes"))
        })
        .collect()
}
