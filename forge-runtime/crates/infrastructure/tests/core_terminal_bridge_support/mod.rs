use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_CONTROL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION, GroupAgentNodeTerminalClassification,
    GroupAgentNodeTerminalControl, GroupAgentScheduledNodeTerminalArtifact,
    GroupAgentScheduledNodeTerminalArtifactKind, GroupAgentScheduledNodeTerminalControl,
    group_agent_scheduled_node_terminal_artifact_id,
    group_agent_scheduled_node_terminal_output_sha256,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
pub(crate) struct SharedTerminalFixture {
    pub(crate) canonical_terminal_control_json: String,
    pub(crate) canonical_terminal_receipt_json: String,
    pub(crate) terminal_receipt_sha256: String,
}

pub(crate) fn shared_terminal_fixture() -> SharedTerminalFixture {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-terminal-lifecycle-v1.json"
    )))
    .expect("shared terminal fixture")
}

pub(crate) fn shared_terminal_control() -> GroupAgentNodeTerminalControl {
    let fixture = shared_terminal_fixture();
    GroupAgentNodeTerminalControl::decode_exact(fixture.canonical_terminal_control_json.as_bytes())
        .expect("strict shared terminal control")
}

pub(crate) fn scheduled_terminal_control() -> GroupAgentScheduledNodeTerminalControl {
    let artifact = scheduled_terminal_artifact();
    let mut control = GroupAgentScheduledNodeTerminalControl {
        v: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        terminal_control_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        release_control_snapshot_sha256: "1".repeat(64),
        graph_run_id: artifact.graph_run_id.clone(),
        graph_id: "scheduled-graph".into(),
        node_id: artifact.node_id.clone(),
        attempt: artifact.attempt,
        dispatch_id: artifact.dispatch_id.clone(),
        provider_request_id: artifact.provider_request_id.clone(),
        authorization_sha256: artifact.authorization_sha256.clone(),
        provider_request_sha256: artifact.provider_request_sha256.clone(),
        request_body_sha256: artifact.request_body_sha256.clone(),
        expected_last_event_seq: 1,
        expected_last_event_sha256: "2".repeat(64),
        claim_event_sha256: artifact.claim_event_sha256.clone(),
        project_lane_sha256: artifact.project_lane_sha256.clone(),
        artifact,
        snapshot_sha256: String::new(),
    };
    control.snapshot_sha256 = control.expected_sha256().expect("scheduled control digest");
    control
        .validate()
        .expect("valid scheduled terminal control");
    control
}

fn scheduled_terminal_artifact() -> GroupAgentScheduledNodeTerminalArtifact {
    let output = "scheduled terminal output".to_owned();
    let mut artifact = GroupAgentScheduledNodeTerminalArtifact {
        v: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION,
        terminal_artifact_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind::Result,
        graph_run_id: "scheduled-graph-run".into(),
        node_id: "frontend".into(),
        attempt: 1,
        dispatch_id: "scheduled-dispatch".into(),
        provider_request_id: "scheduled-provider-request".into(),
        claim_event_sha256: "3".repeat(64),
        authorization_sha256: "4".repeat(64),
        provider_request_sha256: "5".repeat(64),
        request_body_sha256: "6".repeat(64),
        pricing_snapshot_sha256: "7".repeat(64),
        lane_ownership_id: "scheduled-lane-owner".into(),
        project_lane_sha256: "8".repeat(64),
        provider_poll_started: true,
        terminal_seen: true,
        stream_eof_seen: true,
        classification: GroupAgentNodeTerminalClassification::Completed,
        output_bytes: output.len(),
        output_sha256: group_agent_scheduled_node_terminal_output_sha256(&output),
        output_text: output,
        usage_observed: true,
        input_tokens: 2,
        output_tokens: 3,
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms: 100,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    };
    seal_scheduled_artifact(&mut artifact);
    artifact
}

fn seal_scheduled_artifact(artifact: &mut GroupAgentScheduledNodeTerminalArtifact) {
    let sha256 = artifact
        .expected_sha256()
        .expect("scheduled artifact digest");
    artifact.artifact_id = group_agent_scheduled_node_terminal_artifact_id(&sha256);
    artifact.artifact_sha256 = sha256;
    loop {
        let bytes = artifact
            .canonical_json()
            .expect("scheduled artifact JSON")
            .len();
        if artifact.artifact_bytes == bytes {
            break;
        }
        artifact.artifact_bytes = bytes;
    }
    artifact.validate().expect("valid scheduled artifact");
}

pub(crate) fn write_script(directory: &Path, name: &str, body: &str, executable: bool) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, body).expect("write Core test script");
    set_executable(&path, executable);
    path.canonicalize().expect("canonical Core test script")
}

#[cfg(unix)]
fn set_executable(path: &Path, executable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if executable { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("set script mode");
}

#[cfg(not(unix))]
fn set_executable(_path: &Path, _executable: bool) {}

pub(crate) fn script_digest(path: &Path) -> String {
    let bytes = fs::read(path).expect("read Core test executable");
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn core_script(decision: &str) -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$1\" != \"graph-node-terminal-receipt\" ]; then exit 90; fi\n\
         if [ \"$2\" = \"--protocol-version\" ]; then printf '%s' '1'; exit 0; fi\n\
         if [ \"$2\" != \"--control\" ] || [ \"$3\" != \"-\" ]; then exit 91; fi\n\
         {decision}\n"
    )
}

pub(crate) fn environment_probe_script() -> String {
    core_script("if [ \"${FORGE_CORE_ENV_WORKER+x}\" = x ]; then exit 92; fi; printf '%s' '{}'")
}

pub(crate) fn build_go_forge(directory: &Path) -> PathBuf {
    let output = directory.join("forge");
    let status = Command::new("go")
        .args(["build", "-trimpath", "-o"])
        .arg(&output)
        .arg("./cmd/forge")
        .current_dir(repository_root().join("forge-core"))
        .env("GOPROXY", "off")
        .env("GOSUMDB", "off")
        .env("GOTOOLCHAIN", "local")
        .status()
        .expect("start deterministic Go build");
    assert!(status.success(), "deterministic Go Core build failed");
    output.canonicalize().expect("canonical Go Core binary")
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repository root")
}
