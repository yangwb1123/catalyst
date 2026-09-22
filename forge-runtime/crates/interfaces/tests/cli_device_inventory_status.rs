use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-status-contract-v1.json"
);

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "inventory", "status", "--input"])
        .arg(path)
        .output()
        .expect("run device inventory status")
}

fn assert_read_only_output(value: &Value) {
    assert_eq!(value["type"], "device_inventory_status");
    assert_eq!(
        value["schema_version"],
        "forge.device-inventory-status-contract/v1"
    );
    assert_eq!(value["evaluation_mode"], "pure_projection_only");
    assert_eq!(value["stale_after_ms"], 90_000);
    assert_eq!(value["cases"].as_array().map(Vec::len), Some(11));
    assert_eq!(value["cases"][0]["status"], "online");
    assert_eq!(value["cases"][0]["declared_eligible"], true);
    assert_eq!(value["cases"][8]["error"], "snapshot_from_future");
    assert_eq!(value["cases"][9]["error"], "lease_before_snapshot");
    assert_eq!(value["cases"][10]["error"], "unknown_liveness");
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
fn status_fixture_is_projected_in_stable_json_output() {
    let fixture = NamedTempFile::new().expect("temporary fixture");
    fixture
        .as_file()
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = run_path(fixture.path(), true);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_read_only_output(&value);
}

#[test]
fn status_supports_bounded_stdin_without_hub_or_device_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--json", "device", "inventory", "status", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn device inventory status");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = child.wait_with_output().expect("wait for CLI");
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_read_only_output(&value);
}

#[test]
fn unknown_duplicate_oversized_and_authority_mutations_fail_closed() {
    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let unknown_file = NamedTempFile::new().expect("temporary unknown input");
    std::fs::write(
        unknown_file.path(),
        serde_json::to_vec(&unknown).expect("unknown field JSON"),
    )
    .expect("write unknown field input");
    assert!(!run_path(unknown_file.path(), false).status.success());

    let mut duplicate: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let first = duplicate["cases"][0].clone();
    duplicate["cases"].as_array_mut().unwrap().push(first);
    let duplicate_file = NamedTempFile::new().expect("temporary duplicate input");
    std::fs::write(
        duplicate_file.path(),
        serde_json::to_vec(&duplicate).expect("duplicate JSON"),
    )
    .expect("write duplicate input");
    assert!(!run_path(duplicate_file.path(), false).status.success());

    let mut authorized: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    authorized["authority"]["dispatch_performed"] = Value::Bool(true);
    let authorized_file = NamedTempFile::new().expect("temporary authority input");
    std::fs::write(
        authorized_file.path(),
        serde_json::to_vec(&authorized).expect("authority JSON"),
    )
    .expect("write authority input");
    assert!(!run_path(authorized_file.path(), false).status.success());

    let oversized_file = NamedTempFile::new().expect("temporary oversized input");
    std::fs::write(oversized_file.path(), vec![b'x'; 2 * 1024 * 1024 + 1])
        .expect("write oversized input");
    assert!(!run_path(oversized_file.path(), false).status.success());
}

#[test]
fn human_output_identifies_projection_and_false_authority() {
    let fixture = NamedTempFile::new().expect("temporary fixture");
    fixture
        .as_file()
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = run_path(fixture.path(), false);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8 output");
    assert!(text.contains("offline device inventory status"));
    assert!(text.contains("approved_online_is_declared_eligible: status=online"));
    assert!(text.contains("future_snapshot_rejected: error=snapshot_from_future"));
    assert!(text.contains("execution_authorized=false dispatch_performed=false"));
}
