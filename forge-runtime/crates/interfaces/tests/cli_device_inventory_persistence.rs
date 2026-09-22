use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str =
    include_str!("../../../../docs/contracts/fixtures/forge-device-inventory-persistence-v1.json");

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-persistence-v1.json")
}

fn run_path(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_forge-runtime"));
    if json {
        command.arg("--json");
    }
    command
        .args(["device", "inventory", "persistence-preview", "--input"])
        .arg(path)
        .output()
        .expect("run device inventory persistence preview")
}

fn assert_preview(value: &Value) {
    assert_eq!(value["type"], "device_inventory_persistence_preview");
    assert_eq!(
        value["schema_version"],
        "forge.device-inventory-persistence/v1"
    );
    assert_eq!(
        value["evaluation_mode"],
        "pure_persisted_inventory_cas_projection"
    );
    assert_eq!(value["cases"].as_array().map(Vec::len), Some(12));
    assert_eq!(value["cases"][0]["accepted"], true);
    assert_eq!(value["cases"][0]["status"], "online");
    assert_eq!(value["cases"][1]["revision"], 4);
    assert_eq!(value["cases"][2]["error"], "revision_conflict");
    assert_eq!(value["cases"][3]["error"], "runner_device_mismatch");
    assert_eq!(value["cases"][4]["error"], "owner_mismatch");
    assert_eq!(value["cases"][5]["error"], "invalid_evaluation_owner");
    assert_eq!(value["cases"][6]["status"], "stale");
    assert_eq!(value["cases"][10]["status"], "revoked");
    assert_eq!(value["cases"][11]["error"], "revision_overflow");
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
fn persistence_fixture_is_evaluated_as_pure_json() {
    let output = run_path(&fixture_path(), true);
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_preview(&value);
}

#[test]
fn persistence_preview_supports_bounded_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "persistence-preview",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn persistence preview");
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
fn persistence_preview_rejects_authority_unknown_and_duplicate_keys() {
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
