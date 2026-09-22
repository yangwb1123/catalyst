use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const RUN_FIXTURE: &str =
    include_str!("../../../../docs/contracts/fixtures/forge-run-intent-observation-v1.json");

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures")
        .join(name)
}

fn command(run_input: &Path, placement_input: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "placement", "run-intent-preview", "--input"])
        .arg(run_input)
        .arg("--placement-input")
        .arg(placement_input)
        .output()
        .expect("run offline Run-intent preview")
}

#[test]
fn shared_fixtures_produce_a_payload_free_non_authoritative_preview() {
    let output = command(
        &fixture_path("forge-run-intent-observation-v1.json"),
        &fixture_path("forge-session-placement-observation-v1.json"),
        true,
    );
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["type"], "device_run_intent_preview");
    assert_eq!(value["schema_version"], "forge.run-intent-observation/v1");
    assert_eq!(value["evaluation_mode"], "offline_static_only");
    assert_eq!(value["conversation_id"], "conversation-001");
    assert_eq!(value["prompt_id"], "prompt-001");
    assert_eq!(value["run_id"], "run-001");
    assert_eq!(value["preview_only"], true);
    assert_eq!(value["placement_decision_count"], 9);
    assert_eq!(value["eligible_instance_count"], 2);
    assert!(value.get("prompt_content").is_none());
    assert!(value["selected_device_id"].is_null());
    assert!(value["selected_instance_id"].is_null());
    for field in [
        "identity_verified",
        "heartbeat_persisted",
        "inventory_authoritative",
        "reservation_created",
        "execution_authorized",
        "dispatch_performed",
    ] {
        assert_eq!(value["authority"][field], false, "authority field {field}");
    }
}

#[test]
fn human_output_names_the_preview_and_never_claims_a_selected_target() {
    let output = command(
        &fixture_path("forge-run-intent-observation-v1.json"),
        &fixture_path("forge-session-placement-observation-v1.json"),
        false,
    );
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(stdout.contains("offline Run-intent preview"));
    assert!(stdout.contains("decisions=9 eligible=2"));
    assert!(stdout.contains("selected_device=none selected_instance=none"));
    assert!(stdout.contains("execution_authorized=false dispatch_performed=false"));
}

#[test]
fn one_input_may_be_read_from_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "placement",
            "run-intent-preview",
            "--input",
            "-",
            "--placement-input",
        ])
        .arg(fixture_path("forge-session-placement-observation-v1.json"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn offline Run-intent preview");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(RUN_FIXTURE.as_bytes())
        .expect("write Run-intent fixture");
    let output = child.wait_with_output().expect("wait for preview");
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["preview_only"], true);
    assert_eq!(value["eligible_instance_count"], 2);
}

#[test]
fn unknown_fields_oversize_and_confused_owner_binding_fail_closed() {
    let placement = fixture_path("forge-session-placement-observation-v1.json");
    let mut unknown: Value = serde_json::from_str(RUN_FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let unknown_file = NamedTempFile::new().expect("temporary input");
    std::fs::write(
        unknown_file.path(),
        serde_json::to_vec(&unknown).expect("unknown JSON"),
    )
    .expect("write unknown input");
    assert!(
        !command(unknown_file.path(), &placement, true)
            .status
            .success()
    );

    let mut confused: Value = serde_json::from_str(RUN_FIXTURE).expect("fixture JSON");
    confused["owner"]["subject"] = Value::String("other-user".into());
    let confused_file = NamedTempFile::new().expect("temporary input");
    std::fs::write(
        confused_file.path(),
        serde_json::to_vec(&confused).expect("confused JSON"),
    )
    .expect("write confused input");
    assert!(
        !command(confused_file.path(), &placement, true)
            .status
            .success()
    );

    let oversized_file = NamedTempFile::new().expect("temporary oversized input");
    std::fs::write(oversized_file.path(), vec![b'x'; 2 * 1024 * 1024 + 1])
        .expect("write oversized input");
    assert!(
        !command(oversized_file.path(), &placement, true)
            .status
            .success()
    );
}
