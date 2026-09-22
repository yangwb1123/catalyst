use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::tempdir;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-batch-evaluation-v1.json"
);
const SOURCE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
);

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-batch-evaluation-v1.json",
    )
}

fn run(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "placement-batch-evaluation",
            "--input",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run placement batch evaluation")
}

fn write_fixture(dir: &Path, value: &Value) -> PathBuf {
    let path = dir.join("batch.json");
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::write(
        dir.join("forge-device-inventory-placement-input-v1.json"),
        SOURCE,
    )
    .unwrap();
    path
}

#[test]
fn canonical_fixture_is_consumed_as_metadata_only_evaluation() {
    let output = run(&fixture_path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["schema_version"],
        "forge.persisted-inventory-placement-evaluation/v1"
    );
    assert_eq!(
        value["evaluation_mode"],
        "pure_persisted_inventory_placement_dry_run"
    );
    assert_eq!(value["decisions"].as_array().unwrap().len(), 6);
    assert!(value["selected_device_id"].is_null());
    assert!(value["selected_instance_id"].is_null());
    assert_eq!(value["authority"]["identity_verified"], false);
    assert_eq!(value["authority"]["placement_selected"], false);
    assert_eq!(value["authority"]["execution_authorized"], false);
    assert_eq!(value["authority"]["dispatch_performed"], false);
}

#[test]
fn stdin_is_supported_and_unknown_authority_duplicate_and_unsafe_inputs_fail_closed() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "placement-batch-evaluation",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(FIXTURE.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dir = tempdir().unwrap();
    let mut unknown: Value = serde_json::from_str(FIXTURE).unwrap();
    unknown["unexpected"] = Value::Bool(true);
    assert!(!run(&write_fixture(dir.path(), &unknown)).status.success());

    let mut authority: Value = serde_json::from_str(FIXTURE).unwrap();
    authority["authority"]["dispatch_performed"] = Value::Bool(true);
    assert!(!run(&write_fixture(dir.path(), &authority)).status.success());

    let mut duplicate: Value = serde_json::from_str(FIXTURE).unwrap();
    duplicate["cases"][1]["device_id"] = duplicate["cases"][0]["device_id"].clone();
    assert!(!run(&write_fixture(dir.path(), &duplicate)).status.success());

    let mut unsafe_time: Value = serde_json::from_str(FIXTURE).unwrap();
    unsafe_time["evaluated_at_ms"] = Value::Number(9_007_199_254_740_992u64.into());
    assert!(
        !run(&write_fixture(dir.path(), &unsafe_time))
            .status
            .success()
    );
}

#[test]
fn duplicate_json_keys_are_rejected_before_deserialization() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("duplicate.json");
    let duplicate = FIXTURE.replacen(
        "\"evaluation_mode\"",
        "\"schema_version\":\"duplicate\",\"evaluation_mode\"",
        1,
    );
    std::fs::write(&path, duplicate).unwrap();
    std::fs::write(
        dir.path()
            .join("forge-device-inventory-placement-input-v1.json"),
        SOURCE,
    )
    .unwrap();
    assert!(!run(&path).status.success());
}

#[test]
fn sibling_source_fixture_drift_is_rejected() {
    let dir = tempdir().unwrap();
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let path = write_fixture(dir.path(), &fixture);
    let drifted = SOURCE.replace(
        "pure_persisted_inventory_to_placement_input",
        "drifted_persisted_inventory_to_placement_input",
    );
    std::fs::write(
        dir.path()
            .join("forge-device-inventory-placement-input-v1.json"),
        drifted,
    )
    .unwrap();
    assert!(!run(&path).status.success());
}
