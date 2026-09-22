use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str = include_str!(
    "../../../../docs/contracts/fixtures/forge-device-heartbeat-persistence-contract-v1.json"
);

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "heartbeat-persistence-preview", "--input"])
        .arg(path)
        .output()
        .expect("run heartbeat persistence preview")
}

fn assert_preview(value: &Value) {
    assert_eq!(value["type"], "device_heartbeat_persistence_preview");
    assert_eq!(
        value["schema_version"],
        "forge.device-heartbeat-persistence-contract/v1"
    );
    assert_eq!(value["evaluation_mode"], "pure_compare_and_swap_plan");
    assert_eq!(value["cases"].as_array().map(Vec::len), Some(10));
    assert_eq!(value["cases"][0]["accepted"], true);
    assert_eq!(value["cases"][0]["revision"], 1);
    assert_eq!(value["cases"][2]["error"], "sequence_not_increasing");
    assert_eq!(value["cases"][3]["error"], "revision_conflict");
    assert_eq!(value["cases"][5]["error"], "device_revoked");
    assert_eq!(value["cases"][8]["error"], "invalid_persisted_state");
    assert_eq!(value["cases"][9]["error"], "revision_overflow");
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
fn heartbeat_persistence_fixture_is_evaluated_as_pure_json() {
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
    assert_preview(&value);
}

#[test]
fn heartbeat_persistence_preview_supports_bounded_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "heartbeat-persistence-preview",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn heartbeat persistence preview");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = child.wait_with_output().expect("wait for CLI");
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_preview(&value);
}

#[test]
fn heartbeat_persistence_preview_rejects_authority_and_unknown_fields() {
    for (field, value) in [("dispatch_performed", Value::Bool(true))] {
        let mut mutated: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        mutated["authority"][field] = value;
        let file = NamedTempFile::new().expect("temporary authority input");
        std::fs::write(
            file.path(),
            serde_json::to_vec(&mutated).expect("authority JSON"),
        )
        .expect("write authority input");
        assert!(!run_path(file.path(), false).status.success());
    }
    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    let file = NamedTempFile::new().expect("temporary unknown input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(&unknown).expect("unknown JSON"),
    )
    .expect("write unknown input");
    assert!(!run_path(file.path(), false).status.success());
}
