use std::{
    fs,
    io::{ErrorKind, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

pub use forge_runtime_domain::*;
use forge_runtime_infrastructure::OpenAiResponsesProvider;
use serde_json::Value;

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
#[allow(dead_code)]
#[path = "../../domain/src/group_agent_node_execution/scheduled_ready_dispatch_release_test_support.rs"]
mod scheduled_ready_release_domain_support;

use cli_group_agent_scheduled_node_contract_support::{
    CREDENTIAL_MARKER, admit_candidate, admit_schedule, build_candidate_at, build_schedule,
    export_control, json, prepare_run,
};
use cli_group_agent_scheduled_node_provider_request_support::{
    all_hub_state, checkpoint, prepare_provider_request,
};
use group_agent_graph_run_support::{Fixture, TASK_SECRET, WORKSPACE_SECRET, command};
use group_agent_graph_support::{path_text, successful_json, text};
use scheduled_graph_reconcile_cli_support::{loopback_sentinel, shared_core};
use scheduled_ready_release_domain_support::successor_control;

const RELEASE_CREDENTIAL_POISON: &str =
    "scheduled-ready-release-credential-must-not-be-read\r\nx-private-header: rejected";
const PRICING: &str = "48f3531a7d71015453dc27a71bd0f17efbaf68ddfcff04461bd5d01b52cade8d";

#[test]
fn real_pinned_core_authorizes_one_future_initial_release_without_effects() {
    let prepared = PreparedReadyRelease::new();
    let before = all_hub_state(prepared.fixture.state.path());

    let output = prepared.invoke(&prepared.core_sha256);
    let value = successful_json(&output);
    assert_release_output(&value, &prepared.graph_run_id);
    assert_private_source_not_emitted(&output);

    assert_eq!(
        all_hub_state(prepared.fixture.state.path()),
        before,
        "logical Hub state changed"
    );
    prepared.fixture.assert_workspace_unchanged();
    assert_no_network(&prepared.listener);
}

#[test]
fn mismatched_core_pin_fails_before_private_source_or_effects() {
    let prepared = PreparedReadyRelease::new();
    let before = all_hub_state(prepared.fixture.state.path());

    let output = prepared.invoke(&"0".repeat(64));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_private_source_not_emitted(&output);

    assert_eq!(
        all_hub_state(prepared.fixture.state.path()),
        before,
        "failed command changed logical Hub state"
    );
    prepared.fixture.assert_workspace_unchanged();
    assert_no_network(&prepared.listener);
}

#[test]
fn rust_successor_content_control_round_trips_through_real_go_core() {
    let content = "cross-language\u{2028}predecessor\u{2029}output";
    let control = with_exact_provider_body(successor_control(Some(content)));
    assert_meaningful_control_bindings(&control);
    let authorization = authorize_ready_with_core(&control);

    authorization
        .validate_against_release_control(&control)
        .expect("Go authorization matches Rust successor control");
    assert_eq!(authorization.execution_ordinal, 2);
    assert_eq!(authorization.node_id, "sso");
    assert_eq!(authorization.maximum_future_node_releases, 1);
    assert_eq!(control.direct_predecessor_receipts.len(), 2);
    assert_eq!(
        control
            .predecessor_content_artifact
            .as_ref()
            .expect("content artifact")
            .output_text,
        content
    );
}

#[test]
fn rust_receipt_successor_without_content_round_trips_through_real_go_core() {
    let control = with_exact_provider_body(successor_control(None));
    assert_meaningful_control_bindings(&control);
    let authorization = authorize_ready_with_core(&control);

    authorization
        .validate_against_release_control(&control)
        .expect("Go authorization matches Rust content-free successor control");
    assert_eq!(authorization.execution_ordinal, 2);
    assert_eq!(control.direct_predecessor_receipts.len(), 2);
    assert!(
        !control
            .scheduled_contract
            .request
            .predecessor_content_included
    );
    assert!(control.predecessor_content_artifact.is_none());
}

#[test]
fn compiled_go_empty_direct_successor_round_trips_through_rust_and_real_core() {
    let control = go_ready_control("zero-direct-successor");
    assert_meaningful_control_bindings(&control);
    let authorization = authorize_ready_with_core(&control);

    authorization
        .validate_against_release_control(&control)
        .expect("Go authorization matches Rust-validated empty-direct successor");
    assert_eq!(authorization.execution_ordinal, 1);
    assert!(control.direct_predecessor_receipts.is_empty());
    assert!(
        !control
            .scheduled_contract
            .request
            .predecessor_content_included
    );
    assert!(control.predecessor_content_artifact.is_none());
}

#[test]
fn compiled_go_ordinal31_control_round_trips_through_rust_and_real_core() {
    let control = go_ready_control("ordinal31");
    assert_meaningful_control_bindings(&control);
    let authorization = authorize_ready_with_core(&control);

    authorization
        .validate_against_release_control(&control)
        .expect("Go authorization matches Rust-validated ordinal31 control");
    assert_eq!(authorization.execution_ordinal, 31);
    assert_eq!(authorization.node_id, "node-31");
    assert_eq!(authorization.maximum_future_node_releases, 1);
    assert_eq!(control.direct_predecessor_receipts.len(), 31);
    assert_eq!(control.progress_snapshot.nodes.len(), 32);
}

fn go_ready_control(shape: &str) -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let directory = tempfile::tempdir().expect("ready-control fixture directory");
    let output_path = directory.path().join("ready-control.json");
    let output = Command::new("go")
        .args([
            "test",
            "./internal/graphscheduledrelease",
            "-run",
            "^TestExportReadyControlForCrossLanguage$",
            "-count=1",
        ])
        .current_dir(forge_core_dir())
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOTOOLCHAIN", "local")
        .env("FORGE_TEST_READY_CONTROL_OUTPUT", &output_path)
        .env("FORGE_TEST_READY_CONTROL_SHAPE", shape)
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .expect("run compiled Go ready-control fixture builder");
    assert!(
        output.status.success(),
        "Go fixture stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(output_path).expect("read Go ready control");
    let control = GroupAgentScheduledReadyNodeDispatchReleaseControl::decode_exact_bytes(&bytes)
        .expect("Rust validates exact Go ready control");
    assert_eq!(
        control
            .canonical_json()
            .expect("Rust canonical ready control"),
        String::from_utf8(bytes).expect("Go ready-control UTF-8")
    );
    control
}

fn forge_core_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("forge-core")
}

fn assert_meaningful_control_bindings(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) {
    for value in [
        &control.scheduled_contract.contract_id,
        &control.scheduled_contract.contract_sha256,
        &control.provider_request.provider_request_id,
        &control.provider_request.prepared_request_sha256,
        &control.provider_request.logical_request_id,
        &control.provider_request.logical_request_sha256,
    ] {
        assert!(!value.is_empty());
    }
    assert!(control.provider_request.provider_request_bytes > 0);
    assert!(!control.provider_request_json.is_empty());
}

fn with_exact_provider_body(
    mut control: GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let candidate = &control.scheduled_contract;
    let request = ModelRequest {
        system_prompt: candidate.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: candidate.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: candidate.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    };
    let body = OpenAiResponsesProvider::encode_request_bytes(&candidate.provider.model, &request)
        .expect("encode exact Rust provider body");
    control.provider_request_json = String::from_utf8(body.clone()).unwrap();
    control.provider_request.provider_request_bytes = body.len();
    control.provider_request.provider_request_sha256 =
        group_agent_node_provider_request_sha256(&body);
    control.provider_request.prepared_request_sha256.clear();
    control.provider_request.prepared_request_sha256 = control
        .provider_request
        .expected_sha256()
        .expect("provider-record digest");
    control.provider_request.provider_request_id = group_agent_scheduled_node_provider_request_id(
        &control.provider_request.prepared_request_sha256,
    );
    reseal_ready_provider_bindings(control)
}

fn reseal_ready_provider_bindings(
    mut control: GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let ordinal = control.scheduled_contract.node.execution_ordinal;
    let selected = &mut control.progress_snapshot.nodes[ordinal];
    selected.provider_request_id = Some(control.provider_request.provider_request_id.clone());
    selected.prepared_request_sha256 =
        Some(control.provider_request.prepared_request_sha256.clone());
    control.progress_snapshot.snapshot_sha256.clear();
    control.progress_snapshot = control.progress_snapshot.seal().expect("progress digest");
    control
        .reconcile_decision
        .snapshot_sha256
        .clone_from(&control.progress_snapshot.snapshot_sha256);
    control.reconcile_decision.decision_sha256.clear();
    control.reconcile_decision = control.reconcile_decision.seal().expect("decision digest");
    control.snapshot_sha256.clear();
    control.seal().expect("exact provider-bound ready control")
}

fn authorize_ready_with_core(
    control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
) -> GroupAgentScheduledReadyNodeDispatchAuthorization {
    let core = shared_core();
    let mut child = Command::new(&core.path)
        .args([
            "graph-scheduled-ready-node-dispatch-authorize",
            "--control",
            "-",
        ])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn real Go ready-release Core");
    let input = control.canonical_json().expect("Rust canonical control");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().expect("wait for real Go Core");
    assert!(
        output.status.success(),
        "Core stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.ends_with(b"\n"));
    GroupAgentScheduledReadyNodeDispatchAuthorization::decode_exact_bytes(&output.stdout)
        .expect("Rust exact Go authorization")
}

struct PreparedReadyRelease {
    fixture: Fixture,
    listener: TcpListener,
    endpoint: String,
    graph_run_id: String,
    core_path: String,
    core_sha256: String,
}

impl PreparedReadyRelease {
    fn new() -> Self {
        let (listener, endpoint) = loopback_sentinel();
        let fixture = Fixture::new();
        let graph_run_id = prepare_run(&fixture, "scheduled-ready-release-source-run");
        let control = export_control(&fixture, &graph_run_id);
        let schedule = build_schedule(&control);
        admit_schedule(&fixture, &graph_run_id, &schedule);
        let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
        let candidate = build_candidate_at(&control, &schedule_sha256, &endpoint);
        let admitted = admit_candidate(
            &fixture,
            &graph_run_id,
            "scheduled-ready-release-candidate",
            &candidate,
        );
        let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
        prepare_provider_request(
            &fixture,
            &contract_id,
            "scheduled-ready-release-provider-request",
        );
        checkpoint(fixture.state.path());
        let core = shared_core();
        Self {
            fixture,
            listener,
            endpoint,
            graph_run_id,
            core_path: path_text(&core.path).to_owned(),
            core_sha256: core.sha256.clone(),
        }
    }

    fn invoke(&self, digest: &str) -> Output {
        command(
            self.fixture.state.path(),
            self.fixture.cwd.path(),
            &[
                "group",
                "graph",
                "run",
                "ready-release",
                &self.graph_run_id,
                "--core-bin",
                &self.core_path,
                "--core-bin-sha256",
                digest,
            ],
        )
        .env("OPENAI_API_KEY", RELEASE_CREDENTIAL_POISON)
        .env("OPENAI_BASE_URL", &self.endpoint)
        .output()
        .expect("run scheduled ready release CLI")
    }
}

fn assert_release_output(value: &Value, graph_run_id: &str) {
    assert_eq!(value["type"], "scheduled_ready_node_release_authorization");
    assert_eq!(value["v"], 2);
    for field in [
        "metadata_only",
        "future_release_policy",
        "source_bundle_fresh_revalidated",
        "core_reconcile_rerun",
        "progress_observed",
        "private_source_sent_to_core",
        "sqlite_live_reader_coordination_possible",
    ] {
        assert_eq!(value[field], true, "{field} must be true");
    }
    assert_eq!(value["effect_facts_scope"], "forge_runtime");
    assert_core_trust_boundary(&value["core_trust_boundary"]);
    assert_runtime_effects_false(&value["runtime_effect_facts"]);
    assert_authorization(&value["authorization"], graph_run_id);
}

fn assert_core_trust_boundary(value: &Value) {
    for field in [
        "same_user_code",
        "operator_trust_required",
        "binary_identity_validated",
        "reconcile_handshake_validated",
        "ready_release_handshake_validated",
        "empty_environment",
    ] {
        assert_eq!(value[field], true, "{field} must be true");
    }
    for field in [
        "filesystem_isolation_enforced",
        "network_isolation_enforced",
        "effect_containment_enforced",
        "effect_attestation_present",
    ] {
        assert_eq!(value[field], false, "{field} must be false");
    }
}

fn assert_runtime_effects_false(value: &Value) {
    let fields = value.as_object().expect("runtime effect object");
    assert_eq!(
        fields.len(),
        23,
        "new effect facts need an explicit assertion"
    );
    for (field, effect) in fields {
        assert_eq!(effect, false, "{field} must be false");
    }
}

fn assert_authorization(value: &Value, graph_run_id: &str) {
    assert_eq!(value["v"], 2);
    assert_eq!(value["scheduler_protocol_version"], 1);
    assert_eq!(value["dispatch_authorization_protocol_version"], 2);
    assert_eq!(value["graph_run_id"], graph_run_id);
    assert_eq!(value["execution_ordinal"], 0);
    assert_eq!(value["node_id"], "frontend");
    assert_eq!(value["maximum_future_node_releases"], 1);
    for field in [
        "scheduled_contract_id",
        "scheduled_contract_sha256",
        "scheduled_provider_request_id",
        "scheduled_provider_request_sha256",
        "logical_request_id",
        "logical_request_sha256",
        "request_body_sha256",
    ] {
        assert!(!text(&value[field]).is_empty(), "{field} must be nonempty");
    }
    assert!(value["request_body_bytes"].as_u64().unwrap() > 0);
    for field in [
        "lifecycle_contract_admission_authorized",
        "execution_authority_release_authorized",
        "dispatch_authority_release_authorized",
        "scheduled_contract_candidate_present",
        "provider_request_prepared",
    ] {
        assert_eq!(value[field], true, "{field} must be true");
    }
    for field in [
        "lifecycle_contract_admitted",
        "execution_authority_released",
        "dispatch_authority_released",
        "project_lane_claimed",
        "provider_request_sent",
        "progress_observed",
        "terminal_receipt_recorded",
        "successor_advance_authorized",
    ] {
        assert_eq!(value[field], false, "{field} must be false");
    }
}

fn assert_private_source_not_emitted(output: &Output) {
    let bytes = [&output.stdout[..], &output.stderr[..]].concat();
    let rendered = String::from_utf8_lossy(&bytes);
    for private in [
        RELEASE_CREDENTIAL_POISON,
        CREDENTIAL_MARKER,
        TASK_SECRET,
        WORKSPACE_SECRET,
    ] {
        assert!(!rendered.contains(private), "output leaked private source");
    }
}

fn assert_no_network(listener: &TcpListener) {
    let error = listener
        .accept()
        .expect_err("unexpected provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}
