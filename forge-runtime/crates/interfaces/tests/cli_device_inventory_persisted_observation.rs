use std::{path::Path, process::Command};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-inventory-persisted-observation-v1.json"
);

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-persisted-observation-v1.json",
    )
}

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "inventory", "persisted-observation", "--input"])
        .arg(path)
        .output()
        .expect("run persisted inventory observation")
}

#[test]
fn persisted_observation_fixture_emits_shared_envelope() {
    let output = run_path(&fixture_path(), true);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let expected: Value = fixture["expected"].clone();
    assert_eq!(value, expected);
    assert_eq!(value["devices"][0]["device"]["device_id"], "device-a");
    assert_eq!(value["devices"][1]["device"]["device_id"], "device-b");
    for field in [
        "execution_authorized",
        "reservation_created",
        "dispatch_performed",
    ] {
        assert_eq!(value[field], false, "observation authority field {field}");
    }
}

#[test]
fn persisted_observation_rejects_authority_unknown_and_duplicate_keys() {
    let mut authority: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    authority["authority"]["dispatch_performed"] = Value::Bool(true);
    let file = NamedTempFile::new().expect("temporary authority input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&authority).expect("authority JSON"),
    )
    .expect("write authority input");
    assert!(!run_path(file.path(), false).status.success());

    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let file = NamedTempFile::new().expect("temporary unknown input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&unknown).expect("unknown JSON"),
    )
    .expect("write unknown input");
    assert!(!run_path(file.path(), false).status.success());

    let file = NamedTempFile::new().expect("temporary duplicate input");
    let duplicate = FIXTURE.replacen(
        "\"evaluation_mode\"",
        "\"evaluation_mode\":\"duplicate\",\"evaluation_mode\"",
        1,
    );
    std::fs::write(file.path(), duplicate).expect("write duplicate input");
    assert!(!run_path(file.path(), false).status.success());
}
