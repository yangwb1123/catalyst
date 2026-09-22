use std::{
    collections::BTreeSet,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str =
    include_str!("../../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json");

fn run_path(path: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "show",
            "--input",
            path.to_str().expect("fixture path is UTF-8"),
        ])
        .output()
        .expect("run device inventory show");
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
        .expect("inventory output object")
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        keys,
        [
            "schema_version",
            "evaluation_mode",
            "evaluated_at_ms",
            "owner_declaration",
            "owner_declaration_unverified",
            "inventory_declarations_unverified",
            "notice",
            "devices",
            "execution_authorized",
            "reservation_created",
            "dispatch_performed",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
    );
    assert!(value.get("v").is_none());
    assert!(value.get("type").is_none());
    assert!(value.get("authority").is_none());
    assert_eq!(
        value["schema_version"],
        "forge.device-inventory-observation/v1"
    );
    assert_eq!(value["evaluation_mode"], "offline_static_only");
    assert_eq!(value["owner_declaration_unverified"], true);
    assert_eq!(value["inventory_declarations_unverified"], true);
    for field in [
        "execution_authorized",
        "reservation_created",
        "dispatch_performed",
    ] {
        assert_eq!(value[field], false, "authority field {field}");
    }
}

#[test]
fn observation_fixture_is_rendered_in_stable_device_instance_order() {
    let mut shuffled: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    shuffled["devices"]
        .as_array_mut()
        .expect("devices")
        .reverse();
    let shuffled_file = NamedTempFile::new().expect("temporary shuffled input");
    std::fs::write(
        shuffled_file.path(),
        serde_json::to_vec(&shuffled).expect("shuffled JSON"),
    )
    .expect("write shuffled input");
    let value = run_path(shuffled_file.path());
    assert_read_only_output(&value);
    let devices = value["devices"].as_array().expect("devices");
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0]["device"]["device_id"], "device-a");
    assert_eq!(devices[0]["instance_id"], "runner-a");
    assert_eq!(devices[1]["device"]["device_id"], "device-b");
    assert_eq!(devices[1]["instance_id"], "runner-b");
    assert_eq!(devices[0]["device"]["available_cpu_cores"], 8);
    assert_eq!(devices[0]["device"]["available_memory_bytes"], 16384);
    assert_eq!(devices[0]["device"]["available_storage_bytes"], 8192);
}

#[test]
fn stdin_input_is_supported_without_creating_hub_state() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--json", "device", "inventory", "show", "--input", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn device inventory show");
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
fn unknown_fields_oversized_input_and_authority_mutations_fail_closed() {
    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let unknown_file = NamedTempFile::new().expect("temporary input");
    std::fs::write(
        unknown_file.path(),
        serde_json::to_vec(&unknown).expect("unknown field JSON"),
    )
    .expect("write unknown field input");
    assert!(!run_command(unknown_file.path()).status.success());

    let mut authorized: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    authorized["execution_authorized"] = Value::Bool(true);
    let authorized_file = NamedTempFile::new().expect("temporary input");
    std::fs::write(
        authorized_file.path(),
        serde_json::to_vec(&authorized).expect("authorized JSON"),
    )
    .expect("write authorized input");
    assert!(!run_command(authorized_file.path()).status.success());

    let oversized_file = NamedTempFile::new().expect("temporary oversized input");
    std::fs::write(oversized_file.path(), vec![b'x'; 2 * 1024 * 1024 + 1])
        .expect("write oversized input");
    assert!(!run_command(oversized_file.path()).status.success());
}

#[test]
fn duplicate_device_or_runner_instance_rows_fail_closed() {
    let mut duplicate_instance: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let first_instance = duplicate_instance["devices"][0]["instance_id"].clone();
    duplicate_instance["devices"][1]["instance_id"] = first_instance;
    let duplicate_instance_file = NamedTempFile::new().expect("temporary duplicate instance");
    std::fs::write(
        duplicate_instance_file.path(),
        serde_json::to_vec(&duplicate_instance).expect("duplicate instance JSON"),
    )
    .expect("write duplicate instance input");
    assert!(!run_command(duplicate_instance_file.path()).status.success());

    let mut duplicate_device: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let first_device = duplicate_device["devices"][0]["device"]["device_id"].clone();
    duplicate_device["devices"][1]["device"]["device_id"] = first_device;
    let duplicate_device_file = NamedTempFile::new().expect("temporary duplicate device");
    std::fs::write(
        duplicate_device_file.path(),
        serde_json::to_vec(&duplicate_device).expect("duplicate device JSON"),
    )
    .expect("write duplicate device input");
    assert!(!run_command(duplicate_device_file.path()).status.success());
}

#[test]
fn inventory_row_limit_matches_shared_contract() {
    let mut oversized: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let template = oversized["devices"][0].clone();
    let devices = oversized["devices"].as_array_mut().expect("devices array");
    for index in devices.len()..129 {
        let mut candidate = template.clone();
        candidate["instance_id"] = Value::String(format!("runner-{index:03}"));
        candidate["device"]["device_id"] = Value::String(format!("device-{index:03}"));
        devices.push(candidate);
    }
    assert_eq!(devices.len(), 129);
    let oversized_file = NamedTempFile::new().expect("temporary oversized row input");
    std::fs::write(
        oversized_file.path(),
        serde_json::to_vec(&oversized).expect("oversized row JSON"),
    )
    .expect("write oversized row input");
    assert!(!run_command(oversized_file.path()).status.success());
}

#[test]
fn safe_integer_bounds_match_shared_contract() {
    const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
    let paths = [
        vec!["evaluated_at_ms"],
        vec!["devices", "0", "device", "snapshot_observed_at_ms"],
        vec!["devices", "0", "device", "lease_expires_at_ms"],
        vec!["devices", "0", "device", "available_memory_bytes"],
        vec!["devices", "0", "device", "available_storage_bytes"],
        vec!["devices", "0", "device", "gpu", "memory_bytes"],
    ];
    for path in paths {
        let mut oversized: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        let pointer = format!("/{}", path.join("/"));
        *oversized
            .pointer_mut(&pointer)
            .expect("safe integer field path") = Value::Number((MAX_SAFE_INTEGER + 1).into());
        let oversized_file = NamedTempFile::new().expect("temporary safe integer input");
        std::fs::write(
            oversized_file.path(),
            serde_json::to_vec(&oversized).expect("safe integer JSON"),
        )
        .expect("write safe integer input");
        assert!(
            !run_command(oversized_file.path()).status.success(),
            "path {path:?} must reject values above the shared safe integer bound"
        );
    }
}

fn run_command(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "device",
            "inventory",
            "show",
            "--input",
            path.to_str().expect("temporary path is UTF-8"),
        ])
        .output()
        .expect("run device inventory command")
}
