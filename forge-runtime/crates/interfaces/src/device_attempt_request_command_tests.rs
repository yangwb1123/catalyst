use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{execute, write_output};
use crate::args::DeviceCommand;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-attempt-request-v1.json")
}

#[test]
fn attempt_request_preview_evaluates_and_normalizes_the_shared_fixture() {
    let output = execute(&DeviceCommand::AttemptRequestPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Attempt request preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.attempt-request/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("pure_attempt_request_only")
    );
    let cases = object
        .get("cases")
        .and_then(Value::as_array)
        .expect("case output");
    assert_eq!(cases.len(), 18);
    let valid = cases
        .iter()
        .find(|case| case.get("name").and_then(Value::as_str) == Some("valid_normalizes_order"))
        .expect("valid case");
    assert_eq!(valid.get("accepted").and_then(Value::as_bool), Some(true));
    assert_eq!(
        valid.get("requested_effects"),
        Some(&serde_json::json!(["read.repo", "write.file"]))
    );
    assert_eq!(
        valid.get("approval_record_ids"),
        Some(&serde_json::json!(["approval.a", "approval.b"]))
    );
    let rejected = cases
        .iter()
        .find(|case| case.get("name").and_then(Value::as_str) == Some("effects_require_grant"))
        .expect("rejected case");
    assert_eq!(
        rejected.get("accepted").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        rejected.get("error").and_then(Value::as_str),
        Some("reference_mismatch")
    );
    assert_eq!(
        object
            .get("authority")
            .and_then(|authority| authority.get("execution_authorized"))
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn attempt_request_preview_rejects_authority_unknown_and_duplicate_keys() {
    let mut fixture: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();
    fixture["authority"]["execution_authorized"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::AttemptRequestPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["execution_authorized"] = Value::Bool(false);
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::AttemptRequestPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );

    let duplicate_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate_path.path(),
        br#"{"schema_version":"forge.attempt-request/v1","schema_version":"forge.attempt-request/v1"}"#,
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::AttemptRequestPreview {
            input: duplicate_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn attempt_request_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::AttemptRequestPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Attempt request preview");
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).expect("human output");
    let rendered = String::from_utf8(rendered).expect("UTF-8 output");
    assert!(rendered.contains(
        "offline Attempt request preview [forge.attempt-request/v1] cases=18 accepted=1 rejected=17"
    ));
    assert!(rendered.contains("valid_normalizes_order: accepted=true error=none state=requested"));
    assert!(rendered.contains("effects_require_grant: accepted=false error=reference_mismatch"));
    assert!(rendered.contains("authority: device_identity_verified=false"));
    assert!(!rendered.contains("attempt-key-000001"));
}
