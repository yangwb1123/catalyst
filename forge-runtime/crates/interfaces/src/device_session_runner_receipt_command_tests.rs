use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{execute, write_output};
use crate::args::DeviceCommand;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json")
}

#[test]
fn session_runner_receipt_preview_emits_the_canonical_observation_envelope() {
    let output = execute(&DeviceCommand::SessionRunnerReceiptPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("session Runner receipt preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.session-runner-receipt-observation/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("pure_session_runner_receipt_binding_only")
    );
    assert_eq!(
        object.get("conversation_id").and_then(Value::as_str),
        Some("conversation-001")
    );
    assert_eq!(
        object.get("prompt_id").and_then(Value::as_str),
        Some("prompt-001")
    );
    assert_eq!(
        object.get("run_id").and_then(Value::as_str),
        Some("run-001")
    );
    assert!(object.get("selected_target_id").is_some_and(Value::is_null));
    assert_eq!(
        object
            .get("receipt_observation")
            .and_then(|receipt| receipt.get("command_id"))
            .and_then(Value::as_str),
        Some("command-001")
    );
    assert_eq!(
        object
            .get("authority")
            .and_then(|authority| authority.get("execution_authorized"))
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(!object.contains_key("argv"));
    assert!(!object.contains_key("output"));
}

#[test]
fn session_runner_receipt_preview_rejects_authority_selection_and_unknown_fields() {
    let mut fixture: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();
    fixture["authority"]["execution_authorized"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::SessionRunnerReceiptPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["execution_authorized"] = Value::Bool(false);
    fixture["selected_target_id"] = Value::String("runner-1".into());
    let selected_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(selected_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::SessionRunnerReceiptPreview {
            input: selected_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["selected_target_id"] = Value::Null;
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::SessionRunnerReceiptPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn session_runner_receipt_preview_rejects_root_and_nested_duplicate_keys() {
    let fixture = fs::read_to_string(fixture_path()).unwrap();
    let root_duplicate = fixture.replacen(
        "  \"schema_version\": \"forge.session-runner-receipt-observation/v1\",\n",
        "  \"schema_version\": \"forge.session-runner-receipt-observation/v1\",\n  \"schema_version\": \"forge.session-runner-receipt-observation/v1\",\n",
        1,
    );
    let root_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(root_path.path(), root_duplicate).unwrap();
    assert!(
        execute(&DeviceCommand::SessionRunnerReceiptPreview {
            input: root_path.path().display().to_string(),
        })
        .is_err()
    );

    let nested_duplicate = fixture.replacen(
        "      \"device_identity_verified\": false,\n",
        "      \"device_identity_verified\": false,\n      \"device_identity_verified\": false,\n",
        1,
    );
    let nested_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(nested_path.path(), nested_duplicate).unwrap();
    assert!(
        execute(&DeviceCommand::SessionRunnerReceiptPreview {
            input: nested_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn session_runner_receipt_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::SessionRunnerReceiptPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("session Runner receipt preview");
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).expect("human output");
    let rendered = String::from_utf8(rendered).expect("UTF-8 output");
    assert!(rendered.contains(
        "offline session Runner terminal receipt observation [forge.session-runner-receipt-observation/v1]"
    ));
    assert!(
        rendered
            .contains("owner=user-1 conversation=conversation-001 prompt=prompt-001 run=run-001")
    );
    assert!(rendered.contains("receipt_command=command-001 attempt=attempt-001 target=runner-1"));
    assert!(rendered.contains("selected_target=none"));
    assert!(rendered.contains("receipt_persisted=false execution_authorized=false"));
    assert!(!rendered.contains("forge-task"));
}
