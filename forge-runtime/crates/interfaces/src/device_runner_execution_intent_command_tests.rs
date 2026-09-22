use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{DeviceCommand, execute};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-execution-intent-v1.json")
}

#[test]
fn runner_execution_intent_preview_emits_the_canonical_observation_envelope() {
    let command = DeviceCommand::RunnerExecutionIntentPreview {
        input: fixture_path().display().to_string(),
    };
    let output = execute(&command).expect("Runner execution intent preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.runner-execution-intent/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("pure_runner_binding_only")
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
    assert_eq!(
        object.get("attempt_id").and_then(Value::as_str),
        Some("attempt-001")
    );
    assert_eq!(
        object.get("command_id").and_then(Value::as_str),
        Some("command-001")
    );
    assert_eq!(
        object.get("target_id").and_then(Value::as_str),
        Some("runner-1")
    );
    assert_eq!(
        object
            .get("prompt_run_binding_valid")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        object
            .get("runner_command_binding_valid")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        object.get("preview_only").and_then(Value::as_bool),
        Some(true)
    );
    assert!(object.get("selected_target_id").is_some_and(Value::is_null));
    assert_eq!(
        object
            .get("authority")
            .and_then(|authority| authority.get("execution_authorized"))
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn runner_execution_intent_preview_rejects_authority_mutation_and_unknown_fields() {
    let mut fixture: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();
    fixture["authority"]["execution_authorized"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerExecutionIntentPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["execution_authorized"] = Value::Bool(false);
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerExecutionIntentPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );
}
