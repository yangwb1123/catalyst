use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{execute, write_output};
use crate::args::DeviceCommand;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-execution-lease-checkpoint-v1.json")
}

#[test]
fn execution_lease_checkpoint_preview_evaluates_restart_and_terminal_cases() {
    let output = execute(&DeviceCommand::ExecutionLeaseCheckpointPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("execution lease checkpoint preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.execution-lease-checkpoint/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("pure_execution_lease_checkpoint_only")
    );
    assert_eq!(
        object.get("cases").and_then(Value::as_array).map(Vec::len),
        Some(4)
    );
    let cases = object.get("cases").and_then(Value::as_array).unwrap();
    let uncertain = cases
        .iter()
        .find(|case| {
            case.get("name").and_then(Value::as_str) == Some("uncertain_receipt_remains_terminal")
        })
        .expect("uncertain case");
    assert_eq!(
        uncertain.get("accepted").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        uncertain.get("terminal").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        uncertain.get("uncertain").and_then(Value::as_bool),
        Some(true)
    );
    let foreign = cases
        .iter()
        .find(|case| case.get("name").and_then(Value::as_str) == Some("foreign_proof_rejected"))
        .expect("foreign proof case");
    assert_eq!(
        foreign.get("accepted").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        foreign.get("error").and_then(Value::as_str),
        Some("invalid_checkpoint")
    );
    let encoded = serde_json::to_string(&value).unwrap();
    assert!(!encoded.contains("fence-1"));
    assert!(!encoded.contains("transport ended after effect boundary"));
    assert!(!encoded.contains("receipt_sha256"));
    assert_eq!(
        object
            .get("authority")
            .and_then(|authority| authority.get("execution_authorized"))
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn execution_lease_checkpoint_preview_rejects_authority_unknown_and_duplicate_fields() {
    let fixture = fs::read(fixture_path()).unwrap();
    let mut value: Value = serde_json::from_slice(&fixture).unwrap();

    value["authority"]["dispatch_performed"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ExecutionLeaseCheckpointPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    let mut value: Value = serde_json::from_slice(&fixture).unwrap();
    value["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ExecutionLeaseCheckpointPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );

    let duplicate_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate_path.path(),
        br#"{"schema_version":"forge.execution-lease-checkpoint/v1","schema_version":"forge.execution-lease-checkpoint/v1"}"#,
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::ExecutionLeaseCheckpointPreview {
            input: duplicate_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn execution_lease_checkpoint_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::ExecutionLeaseCheckpointPreview {
        input: fixture_path().display().to_string(),
    })
    .unwrap();
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).unwrap();
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(rendered.contains(
        "offline execution lease checkpoint preview [forge.execution-lease-checkpoint/v1]"
    ));
    assert!(rendered.contains(
        "uncertain_receipt_remains_terminal: accepted=true terminal=true uncertain=true"
    ));
    assert!(rendered.contains("foreign_proof_rejected: accepted=false error=invalid_checkpoint"));
    assert!(rendered.contains("authority: lease_issued=false"));
    assert!(!rendered.contains("fence-1"));
    assert!(!rendered.contains("transport ended after effect boundary"));
}
