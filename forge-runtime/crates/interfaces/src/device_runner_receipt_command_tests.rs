use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{DeviceCommand, execute};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-command-terminal-receipt-v1.json")
}

#[test]
fn runner_receipt_preview_emits_the_canonical_observation_envelope() {
    let command = DeviceCommand::RunnerReceiptPreview {
        input: fixture_path().display().to_string(),
    };
    let output = execute(&command).expect("Runner terminal receipt preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.runner-command-terminal-receipt/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("pure_runner_command_receipt_only")
    );
    assert_eq!(
        object.get("command_id").and_then(Value::as_str),
        Some("command-1")
    );
    assert_eq!(
        object.get("disposition_kind").and_then(Value::as_str),
        Some("completed")
    );
    assert_eq!(
        object.get("receipt_valid").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        object.get("preview_only").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        object
            .get("authority")
            .and_then(|value| value.get("audit_published"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(!object.contains_key("v"));
    assert!(!object.contains_key("type"));
}

#[test]
fn runner_receipt_preview_rejects_authority_mutation_and_unknown_fields() {
    let mut fixture: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();
    fixture["authority"]["execution_authorized"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerReceiptPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["execution_authorized"] = Value::Bool(false);
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerReceiptPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn runner_receipt_preview_rejects_root_and_nested_duplicate_json_keys() {
    let fixture = fs::read_to_string(fixture_path()).expect("receipt fixture");
    let duplicate_root = fixture.replacen(
        "  \"schema_version\": \"forge.runner-command-terminal-receipt/v1\",\n",
        "  \"schema_version\": \"forge.runner-command-terminal-receipt/v1\",\n  \"schema_version\": \"forge.runner-command-terminal-receipt/v1\",\n",
        1,
    );
    let root_path = tempfile::NamedTempFile::new().expect("root duplicate fixture");
    fs::write(root_path.path(), duplicate_root).expect("write root duplicate fixture");
    let root_error = execute(&DeviceCommand::RunnerReceiptPreview {
        input: root_path.path().display().to_string(),
    })
    .expect_err("root duplicate keys must fail");
    assert!(root_error.to_string().contains("duplicate JSON keys"));

    let duplicate_nested = fixture.replacen(
        "    \"device_identity_verified\": false,\n",
        "    \"device_identity_verified\": false,\n    \"device_identity_verified\": false,\n",
        1,
    );
    let nested_path = tempfile::NamedTempFile::new().expect("nested duplicate fixture");
    fs::write(nested_path.path(), duplicate_nested).expect("write nested duplicate fixture");
    let nested_error = execute(&DeviceCommand::RunnerReceiptPreview {
        input: nested_path.path().display().to_string(),
    })
    .expect_err("nested duplicate keys must fail");
    assert!(nested_error.to_string().contains("duplicate JSON keys"));
}
