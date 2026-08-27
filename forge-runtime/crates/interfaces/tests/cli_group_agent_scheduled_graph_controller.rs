#![cfg(unix)]

use std::{
    fs,
    io::ErrorKind,
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::{TempDir, tempdir};

#[allow(clippy::duplicate_mod)]
mod group_agent_graph_run_support;
#[allow(clippy::duplicate_mod)]
mod group_agent_graph_support;
#[allow(clippy::duplicate_mod, dead_code)]
mod scheduled_graph_reconcile_cli_support;

use group_agent_graph_run_support::command;
use group_agent_graph_support::{path_text, successful_json, text};
use scheduled_graph_reconcile_cli_support::{
    CREDENTIAL_POISON, PinnedGoCore, ReconcileFixture, TASK_SECRET, WORKSPACE_SECRET, shared_core,
};

const MODEL: &str = "private-controller-model";
const OFFICIAL_ENDPOINT: &str = "https://api.openai.com/v1/responses";
const PRICING_SHA256: &str = "4444444444444444444444444444444444444444444444444444444444444444";

#[test]
fn compiled_controller_is_passive_private_and_rejects_core_repin_before_spawn() {
    let core = shared_core();
    let fixture = ReconcileFixture::new(core);
    let (listener, proxy) = loopback_proxy_sentinel();
    let schedule_sha256 = schedule_sha256(&fixture);

    let started_output = start_controller(&fixture, core, &schedule_sha256, &proxy);
    let started = successful_json(&started_output);
    assert_controller_output(&started, &fixture.graph_run_id, &schedule_sha256, true);
    assert_private_output(&started_output, &proxy);

    let passive_state = fixture.hub_state();
    let shown_output = show_controller(&fixture, &proxy);
    let shown = successful_json(&shown_output);
    assert_controller_output(&shown, &fixture.graph_run_id, &schedule_sha256, false);
    assert_show_event_chain(&shown);
    assert_stable_anchors(&started, &shown);
    assert_eq!(fixture.hub_state(), passive_state, "show changed the Hub");

    let advanced_output = advance_controller(&fixture, core, &proxy);
    let advanced = successful_json(&advanced_output);
    assert_controller_output(&advanced, &fixture.graph_run_id, &schedule_sha256, true);
    assert_stable_anchors(&started, &advanced);
    assert_eq!(
        fixture.hub_state(),
        passive_state,
        "advance changed the Hub"
    );

    assert_pin_mismatch_before_spawn(&fixture, &proxy);
    assert_private_output(&shown_output, &proxy);
    assert_private_output(&advanced_output, &proxy);
    fixture.assert_workspace_unchanged();
    assert_no_network(&listener);
}

fn schedule_sha256(fixture: &ReconcileFixture) -> String {
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "schedule",
            "list",
            &fixture.graph_run_id,
        ],
    )
    .output()
    .expect("list admitted schedule");
    let value = successful_json(&output);
    assert_eq!(value["schedules"].as_array().map(Vec::len), Some(1));
    text(&value["schedules"][0]["schedule_sha256"])
}

fn start_controller(
    fixture: &ReconcileFixture,
    core: &PinnedGoCore,
    schedule_sha256: &str,
    proxy: &str,
) -> Output {
    let mut args = vec![
        "group",
        "graph",
        "run",
        "controller",
        "start",
        &fixture.graph_run_id,
        "--expected-schedule-sha256",
        schedule_sha256,
        "--core-bin",
        path_text(&core.path),
        "--core-bin-sha256",
        &core.sha256,
    ];
    args.extend(start_profile_args());
    run_controller(fixture, &args, proxy)
}

fn start_profile_args() -> Vec<&'static str> {
    vec![
        "--endpoint",
        OFFICIAL_ENDPOINT,
        "--model",
        MODEL,
        "--max-output-tokens",
        "32",
        "--max-model-output-bytes",
        "4096",
        "--max-model-events",
        "64",
        "--timeout-ms",
        "5000",
        "--max-cost-usd-micros",
        "10000",
        "--pricing-snapshot-sha256",
        PRICING_SHA256,
        "--max-result-bytes",
        "4096",
        "--max-effectful-steps",
        "2",
        "--max-total-cost-usd-micros",
        "20000",
    ]
}

fn show_controller(fixture: &ReconcileFixture, proxy: &str) -> Output {
    run_controller(
        fixture,
        &[
            "group",
            "graph",
            "run",
            "controller",
            "show",
            &fixture.graph_run_id,
        ],
        proxy,
    )
}

fn advance_controller(fixture: &ReconcileFixture, core: &PinnedGoCore, proxy: &str) -> Output {
    run_controller(
        fixture,
        &[
            "group",
            "graph",
            "run",
            "controller",
            "advance",
            &fixture.graph_run_id,
            "--core-bin",
            path_text(&core.path),
            "--core-bin-sha256",
            &core.sha256,
        ],
        proxy,
    )
}

fn run_controller(fixture: &ReconcileFixture, args: &[&str], proxy: &str) -> Output {
    secure_command(fixture, args, proxy)
        .output()
        .expect("run controller CLI")
}

fn secure_command(fixture: &ReconcileFixture, args: &[&str], proxy: &str) -> Command {
    let mut process = command(fixture.state.path(), fixture.cwd.path(), args);
    process
        .env("OPENAI_API_KEY", CREDENTIAL_POISON)
        .env("HTTP_PROXY", proxy)
        .env("HTTPS_PROXY", proxy)
        .env("http_proxy", proxy)
        .env("https_proxy", proxy)
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy");
    process
}

fn assert_controller_output(value: &Value, run: &str, schedule: &str, core_observed: bool) {
    assert_eq!(value["type"], "scheduled_graph_controller");
    assert_eq!(value["v"], 1);
    assert_eq!(value["metadata_only"], true);
    assert_eq!(value["graph_run_id"], run);
    assert_eq!(value["schedule_sha256"], schedule);
    assert_eq!(value["state"], "awaiting_fresh_consent");
    assert!(value["invocation"].is_null());
    assert_eq!(value["effectful_steps_reserved"], 0);
    assert_eq!(value["cost_usd_micros_reserved"], 0);
    assert_eq!(value["automatic_retry_or_resend_performed"], false);
    assert!(value["post_invocation_error"].is_null());
    assert_eq!(value["journal_current_observed"], true);
    assert_awaiting_anchors(value);
    assert_core_trust(&value["core_trust_boundary"], core_observed);
}

fn assert_awaiting_anchors(value: &Value) {
    let awaiting = &value["awaiting_fresh_consent"];
    assert_eq!(
        awaiting["awaiting_event_sha256"],
        value["head_event_sha256"]
    );
    assert_eq!(awaiting["execution_ordinal"], 0);
    assert_eq!(awaiting["node_id"], "build");
    assert!(text(&awaiting["provider_request_id"]).starts_with("scheduled-node-provider-request-"));
    for field in [
        "awaiting_event_sha256",
        "authorization_sha256",
        "snapshot_sha256",
        "decision_sha256",
    ] {
        assert_digest(&text(&awaiting[field]));
    }
    assert_eq!(awaiting["predecessor_content_included"], false);
}

fn assert_core_trust(trust: &Value, observed: bool) {
    assert_eq!(trust["same_user_code"], true);
    assert_eq!(trust["operator_trust_required"], true);
    for field in [
        "binary_identity_validated",
        "reconcile_handshake_validated",
        "materialization_handshake_validated",
        "ready_release_handshake_validated",
        "terminal_protocol_handshake_validated",
        "empty_environment",
    ] {
        assert_eq!(trust[field], observed, "unexpected {field}");
    }
    assert_eq!(trust["filesystem_isolation_enforced"], false);
    assert_eq!(trust["network_isolation_enforced"], false);
    assert_eq!(trust["effect_containment_enforced"], false);
    assert_eq!(trust["effect_attestation_present"], false);
}

fn assert_show_event_chain(value: &Value) {
    let events = value["events"].as_array().expect("show event chain");
    let head_sequence = usize::try_from(value["head_sequence"].as_u64().unwrap()).unwrap();
    assert_eq!(events.len(), head_sequence);
    let mut previous = None;
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["sequence"], u64::try_from(index + 1).unwrap());
        assert_eq!(event["previous_event_sha256"].as_str(), previous.as_deref());
        let sha256 = text(&event["event_sha256"]);
        assert_digest(&sha256);
        previous = Some(sha256);
    }
    assert_eq!(
        events.last().unwrap()["event_sha256"],
        value["head_event_sha256"]
    );
}

fn assert_stable_anchors(expected: &Value, actual: &Value) {
    for field in [
        "controller_id",
        "schedule_id",
        "schedule_sha256",
        "head_sequence",
        "head_event_sha256",
        "awaiting_fresh_consent",
        "effectful_steps_reserved",
        "cost_usd_micros_reserved",
    ] {
        assert_eq!(actual[field], expected[field], "drifted {field}");
    }
}

fn assert_pin_mismatch_before_spawn(fixture: &ReconcileFixture, proxy: &str) {
    let expected_state = fixture.hub_state();
    let replacement = MarkerCore::new();
    let output = run_controller(
        fixture,
        &[
            "group",
            "graph",
            "run",
            "controller",
            "advance",
            &fixture.graph_run_id,
            "--core-bin",
            path_text(&replacement.path),
            "--core-bin-sha256",
            &replacement.sha256,
        ],
        proxy,
    );
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not match the pinned Core"),
        "{stderr}"
    );
    assert!(!replacement.marker.exists(), "replacement Core was started");
    assert_eq!(
        fixture.hub_state(),
        expected_state,
        "pin mismatch changed Hub"
    );
}

struct MarkerCore {
    _directory: TempDir,
    path: PathBuf,
    sha256: String,
    marker: PathBuf,
}

impl MarkerCore {
    fn new() -> Self {
        let directory = tempdir().expect("replacement Core directory");
        let path = directory.path().join("replacement-core");
        fs::write(
            &path,
            b"#!/bin/sh\nprintf '%s' started > \"${0}.started\"\nexit 97\n",
        )
        .expect("write replacement Core");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("make replacement Core executable");
        let path = path.canonicalize().expect("canonical replacement Core");
        let marker = PathBuf::from(format!("{}.started", path.display()));
        let sha256 = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        Self {
            _directory: directory,
            path,
            sha256,
            marker,
        }
    }
}

fn assert_private_output(output: &Output, proxy: &str) {
    let combined = [&output.stdout[..], &output.stderr[..]].concat();
    let rendered = String::from_utf8_lossy(&combined);
    for private in [
        CREDENTIAL_POISON,
        TASK_SECRET,
        WORKSPACE_SECRET,
        OFFICIAL_ENDPOINT,
        proxy,
        MODEL,
        PRICING_SHA256,
    ] {
        assert!(!rendered.contains(private), "output leaked private source");
    }
}

fn assert_digest(value: &str) {
    assert_eq!(value.len(), 64);
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}

fn assert_no_network(listener: &TcpListener) {
    let error = listener
        .accept()
        .expect_err("unexpected provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}

fn loopback_proxy_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback proxy sentinel");
    listener
        .set_nonblocking(true)
        .expect("nonblocking proxy sentinel");
    let port = listener
        .local_addr()
        .expect("proxy sentinel address")
        .port();
    (listener, format!("http://127.0.0.1:{port}"))
}
