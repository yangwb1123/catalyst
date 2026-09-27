use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{DeviceCommand, execute, write_output};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-attempt-boundary-v1.json")
}

#[test]
fn runner_attempt_boundary_preview_consumes_the_canonical_observation() {
    let output = execute(&DeviceCommand::RunnerAttemptBoundaryPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Runner Attempt boundary preview");
    assert!(output.attempt_boundary_ready);
    assert_eq!(output.current_attempt_state, "accepted");
    assert_eq!(output.next_attempt_state, "starting");
    assert_eq!(output.transition, "begin_starting");
    assert!(output.preview_only);
    assert!(!output.authority.attempt_persisted);
    assert!(!output.authority.dispatch_performed);
}

#[test]
fn runner_attempt_boundary_preview_rejects_drift_duplicate_and_unknown_fields() {
    let original = fs::read(fixture_path()).unwrap();
    let mut value: Value = serde_json::from_slice(&original).unwrap();

    value["authority"]["dispatch_performed"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerAttemptBoundaryPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    value["authority"]["dispatch_performed"] = Value::Bool(false);
    value["attempt_boundary_ready"] = Value::Bool(false);
    let drift_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(drift_path.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerAttemptBoundaryPreview {
            input: drift_path.path().display().to_string(),
        })
        .is_err()
    );

    value["attempt_boundary_ready"] = Value::Bool(true);
    value["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerAttemptBoundaryPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );

    let duplicate_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate_path.path(),
        br#"{"schema_version":"forge.runner-attempt-boundary/v1","schema_version":"forge.runner-attempt-boundary/v1"}"#,
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::RunnerAttemptBoundaryPreview {
            input: duplicate_path.path().display().to_string(),
        })
        .is_err()
    );

    let trailing_path = tempfile::NamedTempFile::new().unwrap();
    let mut trailing = original;
    trailing.extend_from_slice(b" {}\n");
    fs::write(trailing_path.path(), trailing).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerAttemptBoundaryPreview {
            input: trailing_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn runner_attempt_boundary_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::RunnerAttemptBoundaryPreview {
        input: fixture_path().display().to_string(),
    })
    .unwrap();
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).unwrap();
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(rendered.contains("offline Runner Attempt boundary preview"));
    assert!(rendered.contains("attempt_boundary_ready=true"));
    assert!(rendered.contains("preview_only=true"));
    assert!(!rendered.contains("fencing_token"));
    assert!(!rendered.contains("argv"));
}
