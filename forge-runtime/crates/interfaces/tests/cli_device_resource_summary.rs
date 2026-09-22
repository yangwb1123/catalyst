use std::{
    collections::BTreeSet,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

const FIXTURE: &str =
    include_str!("../../../../docs/contracts/fixtures/forge-device-resource-summary-v1.json");

fn run_path(path: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "resource-summary",
            "--input",
            path.to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect("run device inventory resource-summary");
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON output")
}

fn assert_read_only_output(value: &Value) {
    let keys = value
        .as_object()
        .expect("resource summary output object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        [
            "schema_version",
            "evaluation_mode",
            "owner",
            "conversation_id",
            "run_id",
            "evaluated_at_ms",
            "owner_declaration_unverified",
            "inventory_declarations_unverified",
            "placement_declaration_unverified",
            "notice",
            "device_count",
            "runner_instance_count",
            "available_cpu_cores",
            "available_memory_bytes",
            "available_storage_bytes",
            "available_gpu_count",
            "available_gpu_memory_bytes",
            "eligible_device_count",
            "eligible_instance_count",
            "selected_device_id",
            "selected_instance_id",
            "authority",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
    );
    assert!(value.get("v").is_none());
    assert!(value.get("type").is_none());
    assert_eq!(value["schema_version"], "forge.device-resource-summary/v1");
    assert_eq!(value["evaluation_mode"], "offline_static_only");
    assert_eq!(value["device_count"], 9);
    assert_eq!(value["runner_instance_count"], 9);
    assert_eq!(value["available_cpu_cores"], 66);
    assert_eq!(value["available_memory_bytes"], 135_168);
    assert_eq!(value["available_storage_bytes"], 67_584);
    assert_eq!(value["available_gpu_count"], 1);
    assert_eq!(value["available_gpu_memory_bytes"], 4096);
    assert_eq!(value["eligible_device_count"], 2);
    assert_eq!(value["eligible_instance_count"], 2);
    assert_eq!(value["selected_device_id"], Value::Null);
    assert_eq!(value["selected_instance_id"], Value::Null);
    let authority = &value["authority"];
    for field in [
        "identity_verified",
        "heartbeat_persisted",
        "inventory_authoritative",
        "reservation_created",
        "execution_authorized",
        "dispatch_performed",
    ] {
        assert_eq!(authority[field], false, "authority field {field}");
    }
}

#[test]
fn resource_summary_fixture_is_aggregated_in_stable_json_output() {
    let fixture = NamedTempFile::new().expect("temporary fixture");
    fixture
        .as_file()
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let value = run_path(fixture.path());
    assert_read_only_output(&value);
}

#[test]
fn resource_summary_supports_stdin_without_hub_or_device_request() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "resource-summary",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn device resource summary");
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
fn resource_summary_rejects_unknown_oversized_and_confused_bindings() {
    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let unknown_file = NamedTempFile::new().expect("temporary unknown input");
    std::fs::write(
        unknown_file.path(),
        serde_json::to_vec(&unknown).expect("unknown field JSON"),
    )
    .expect("write unknown field input");
    assert!(!run_command(unknown_file.path()).status.success());

    let mut confused: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    confused["placement_observation"]["owner"]["subject"] = Value::String("other-user".into());
    let confused_file = NamedTempFile::new().expect("temporary confused input");
    std::fs::write(
        confused_file.path(),
        serde_json::to_vec(&confused).expect("confused JSON"),
    )
    .expect("write confused input");
    assert!(!run_command(confused_file.path()).status.success());

    let oversized_file = NamedTempFile::new().expect("temporary oversized input");
    std::fs::write(oversized_file.path(), vec![b'x'; 2 * 1024 * 1024 + 1])
        .expect("write oversized input");
    assert!(!run_command(oversized_file.path()).status.success());
}

#[test]
fn resource_summary_rejects_duplicate_json_keys() {
    let duplicate = FIXTURE.replacen(
        r#""api_version": "forgeos.device-resource-summary-contract/v1""#,
        r#""api_version": "forgeos.device-resource-summary-contract/v1", "api_version": "forgeos.device-resource-summary-contract/v1""#,
        1,
    );
    let duplicate_file = NamedTempFile::new().expect("temporary duplicate-key input");
    std::fs::write(duplicate_file.path(), duplicate).expect("write duplicate-key input");
    assert!(!run_command(duplicate_file.path()).status.success());

    let nested_duplicate = FIXTURE.replacen(
        r#""subject": "user-1""#,
        r#""subject": "user-1", "subject": "user-1""#,
        1,
    );
    let nested_duplicate_file = NamedTempFile::new().expect("temporary nested duplicate input");
    std::fs::write(nested_duplicate_file.path(), nested_duplicate)
        .expect("write nested duplicate-key input");
    assert!(!run_command(nested_duplicate_file.path()).status.success());
}

#[test]
fn resource_summary_accepts_zero_capacity_unknown_state_and_lease_edge_declarations() {
    let mut fixture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let device = &mut fixture["inventory"]["devices"][0]["device"];
    device["available_cpu_cores"] = json!(0);
    device["available_memory_bytes"] = json!(0);
    device["available_storage_bytes"] = json!(0);
    device["snapshot_observed_at_ms"] = json!(0);
    device["lease_expires_at_ms"] = json!(0);
    device["approval_state"] = json!("unknown");
    device["cordon_state"] = json!("unknown");
    device["liveness"] = json!("unknown");
    device["trust_zone"] = json!("unknown");
    fixture["expected"]["available_cpu_cores"] = json!(58);
    fixture["expected"]["available_memory_bytes"] = json!(118_784);
    fixture["expected"]["available_storage_bytes"] = json!(59_392);

    let input = NamedTempFile::new().expect("temporary declaration edge input");
    std::fs::write(
        input.path(),
        serde_json::to_vec(&fixture).expect("edge declaration JSON"),
    )
    .expect("write declaration edge input");
    let output = run_path(input.path());
    assert_eq!(output["available_cpu_cores"], 58);
    assert_eq!(output["available_memory_bytes"], 118_784);
    assert_eq!(output["available_storage_bytes"], 59_392);
}

fn run_command(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "device",
            "inventory",
            "resource-summary",
            "--input",
            path.to_str().expect("temporary path is UTF-8"),
        ])
        .output()
        .expect("run device resource summary command")
}
