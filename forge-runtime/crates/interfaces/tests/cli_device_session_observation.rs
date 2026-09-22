use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::NamedTempFile;

const FIXTURE: &str =
    include_str!("../../../../docs/contracts/fixtures/forge-session-device-observation-v1.json");

fn run_path(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "session-observation",
            "--input",
        ])
        .arg(path)
        .output()
        .expect("run device inventory session-observation")
}

fn assert_unverified_summary(value: &Value) {
    assert!(value.get("v").is_none());
    assert!(value.get("type").is_none());
    assert_eq!(value["schema_version"], "forge.device-resource-summary/v1");
    assert_eq!(value["evaluation_mode"], "offline_static_only");
    assert_eq!(value["conversation_id"], "conversation-001");
    assert_eq!(value["run_id"], "run-001");
    assert_eq!(value["device_count"], 9);
    assert_eq!(value["eligible_device_count"], 2);
    assert_eq!(value["selected_device_id"], Value::Null);
    assert_eq!(value["selected_instance_id"], Value::Null);
    assert_eq!(value["owner_declaration_unverified"], true);
    assert_eq!(value["inventory_declarations_unverified"], true);
    assert_eq!(value["placement_declaration_unverified"], true);
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
fn canonical_session_device_observation_emits_unverified_summary() {
    let fixture = NamedTempFile::new().expect("temporary fixture");
    fixture
        .as_file()
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = run_path(fixture.path());
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_unverified_summary(&value);
}

#[test]
fn session_device_observation_supports_bounded_stdin() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--json",
            "device",
            "inventory",
            "session-observation",
            "--input",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn session device observation");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(FIXTURE.as_bytes())
        .expect("write fixture");
    let output = child.wait_with_output().expect("wait for CLI");
    assert!(output.status.success());
    assert_unverified_summary(
        &serde_json::from_slice(&output.stdout).expect("JSON output from stdin"),
    );
}

#[test]
fn session_device_observation_rejects_confused_authorized_and_unordered_inputs() {
    for mutation in [
        |value: &mut Value| {
            value["placement_observation"]["run_id"] = Value::String("other-run".into());
        },
        |value: &mut Value| {
            value["authority"]["execution_authorized"] = Value::Bool(true);
        },
        |value: &mut Value| {
            value["inventory"]["devices"]
                .as_array_mut()
                .expect("devices")
                .swap(0, 1);
        },
        |value: &mut Value| {
            value["resource_summary"]["available_cpu_cores"] = Value::from(65);
        },
        |value: &mut Value| {
            value["placement_observation"]["decisions"][1]["exclusion_reasons"][0] =
                Value::String("invalid reason".into());
        },
    ] {
        let mut value: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
        mutation(&mut value);
        let file = NamedTempFile::new().expect("temporary invalid input");
        std::fs::write(
            file.path(),
            serde_json::to_vec(&value).expect("invalid JSON"),
        )
        .expect("write invalid input");
        assert!(!run_path(file.path()).status.success());
    }
}

#[test]
fn session_device_observation_rejects_unknown_missing_nullable_and_oversized_inputs() {
    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = Value::Bool(true);
    assert_rejected(&unknown);

    let mut missing: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    missing
        .as_object_mut()
        .unwrap()
        .remove("selected_device_id");
    assert_rejected(&missing);

    let oversized = NamedTempFile::new().expect("temporary oversized input");
    std::fs::write(oversized.path(), vec![b'x'; 2 * 1024 * 1024 + 1])
        .expect("write oversized input");
    assert!(!run_path(oversized.path()).status.success());
}

#[test]
fn session_device_observation_rejects_nested_duplicate_json_keys() {
    let duplicate = FIXTURE.replacen(
        r#""subject": "user-1""#,
        r#""subject": "user-1", "subject": "user-1""#,
        1,
    );
    let file = NamedTempFile::new().expect("temporary nested duplicate input");
    std::fs::write(file.path(), duplicate).expect("write nested duplicate input");
    assert!(!run_path(file.path()).status.success());
}

fn assert_rejected(value: &Value) {
    let file = NamedTempFile::new().expect("temporary invalid input");
    std::fs::write(
        file.path(),
        serde_json::to_vec(value).expect("invalid JSON"),
    )
    .expect("write invalid input");
    assert!(!run_path(file.path()).status.success());
}
