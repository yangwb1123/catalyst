use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::tempdir;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v1.json"
);
const SOURCE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-placement-input-v1.json"
);

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v1.json",
    )
}

fn run(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "placement-evaluation",
            "--input",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("run placement evaluation")
}

fn write_fixture(dir: &Path, value: &Value) -> PathBuf {
    let path = dir.join("evaluation.json");
    std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    std::fs::write(
        dir.join("forge-device-inventory-placement-input-v1.json"),
        SOURCE,
    )
    .unwrap();
    path
}

#[test]
fn canonical_fixture_is_recomputed_as_metadata_only_evaluation() {
    let output = run(&fixture_path());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        value["schema_version"],
        "forge.device-inventory-placement-evaluation/v1"
    );
    assert_eq!(value["source_case"], "online");
    assert_eq!(value["expected"]["revision"], 7);
    assert_eq!(value["expected"]["device_id"], "device-a");
    assert_eq!(value["expected"]["instance_id"], "runner-a");
    assert_eq!(value["expected"]["matches_requirements"], false);
    assert_eq!(
        value["expected"]["exclusion_reasons"],
        serde_json::json!([
            "concurrency_capacity_insufficient",
            "data_residency_zone_mismatch",
            "sandbox_floor_unmet",
            "trust_zone_unconfirmed"
        ])
    );
    assert_eq!(value["authority"]["placement_evaluated"], false);
    assert_eq!(value["authority"]["execution_authorized"], false);
    assert_eq!(value["authority"]["dispatch_performed"], false);
}

#[test]
fn stdin_is_supported_and_mutations_fail_closed() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "placement-evaluation",
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

    let mut reason: Value = serde_json::from_str(FIXTURE).unwrap();
    reason["expected"]["exclusion_reasons"][3] = Value::String("drifted".into());
    assert!(!run(&write_fixture(dir.path(), &reason)).status.success());

    let duplicate = FIXTURE.replacen(
        "\"schema_version\": \"forge.device-inventory-placement-evaluation/v1\",",
        "\"schema_version\": \"forge.device-inventory-placement-evaluation/v1\",\"schema_version\": \"forge.device-inventory-placement-evaluation/v1\",",
        1,
    );
    let duplicate_path = dir.path().join("duplicate.json");
    std::fs::write(&duplicate_path, duplicate).unwrap();
    assert!(!run(&duplicate_path).status.success());
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
