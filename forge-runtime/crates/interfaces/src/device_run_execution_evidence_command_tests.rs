use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{execute, write_output};
use crate::args::DeviceCommand;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-run-execution-evidence-v1.json")
}

#[test]
fn run_execution_evidence_preview_emits_the_canonical_fixture() {
    let output = execute(&DeviceCommand::RunExecutionEvidencePreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Run execution-evidence preview");
    let actual = serde_json::to_value(output).expect("JSON output");
    let expected: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual["metadata_observed"], true);
    assert_eq!(actual["content_included"], false);
    assert_eq!(actual["authority"]["execution_authorized"], false);
}

#[test]
fn run_execution_evidence_preview_rejects_authority_content_and_unknown_fields() {
    let mut fixture: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();

    fixture["authority"]["dispatch_performed"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunExecutionEvidencePreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["dispatch_performed"] = Value::Bool(false);
    fixture["content_included"] = Value::Bool(true);
    let content_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(content_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunExecutionEvidencePreview {
            input: content_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["content_included"] = Value::Bool(false);
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunExecutionEvidencePreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn run_execution_evidence_preview_rejects_root_and_nested_duplicate_keys() {
    let fixture = fs::read_to_string(fixture_path()).unwrap();
    let root_duplicate = fixture.replacen(
        "  \"api_version\": \"forge.run.execution-evidence.v1\",\n",
        "  \"api_version\": \"forge.run.execution-evidence.v1\",\n  \"api_version\": \"forge.run.execution-evidence.v1\",\n",
        1,
    );
    let root_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(root_path.path(), root_duplicate).unwrap();
    let root_error = execute(&DeviceCommand::RunExecutionEvidencePreview {
        input: root_path.path().display().to_string(),
    })
    .expect_err("root duplicate keys must fail");
    assert!(root_error.to_string().contains("duplicate JSON keys"));

    let nested_duplicate = fixture.replacen(
        "    \"identity_verified\": false,\n",
        "    \"identity_verified\": false,\n    \"identity_verified\": false,\n",
        1,
    );
    let nested_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(nested_path.path(), nested_duplicate).unwrap();
    let nested_error = execute(&DeviceCommand::RunExecutionEvidencePreview {
        input: nested_path.path().display().to_string(),
    })
    .expect_err("nested duplicate keys must fail");
    assert!(nested_error.to_string().contains("duplicate JSON keys"));
}

#[test]
fn run_execution_evidence_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::RunExecutionEvidencePreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Run execution-evidence preview");
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).expect("human output");
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(
        rendered
            .contains("offline Run execution-evidence preview [forge.run.execution-evidence.v1]")
    );
    assert!(rendered.contains("conversation=conversation-001 prompt=prompt-001 run=run-001"));
    assert!(rendered.contains("metadata_only: metadata_observed=true content_included=false"));
    assert!(rendered.contains("execution_authorized=false dispatch_performed=false"));
    assert!(!rendered.contains("argv"));
    assert!(!rendered.contains("output"));
}
