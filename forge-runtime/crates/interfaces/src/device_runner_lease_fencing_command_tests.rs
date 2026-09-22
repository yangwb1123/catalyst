use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{DeviceCommand, execute, write_output};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-lease-fencing-v1.json")
}

#[test]
fn runner_lease_fencing_preview_evaluates_the_canonical_fixture() {
    let output = execute(&DeviceCommand::RunnerLeaseFencingPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("lease fencing preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.runner-lease-fencing/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("pure_lease_fencing_only")
    );
    assert_eq!(
        object.get("cases").and_then(Value::as_array).map(Vec::len),
        Some(16)
    );
    let cases = object.get("cases").and_then(Value::as_array).unwrap();
    let replay = cases
        .iter()
        .find(|case| case.get("name").and_then(Value::as_str) == Some("terminal_replay"))
        .expect("terminal replay case");
    assert_eq!(replay.get("accepted").and_then(Value::as_bool), Some(true));
    assert_eq!(replay.get("replayed").and_then(Value::as_bool), Some(true));
    let uncertain = cases
        .iter()
        .find(|case| case.get("name").and_then(Value::as_str) == Some("terminal_uncertain"))
        .expect("uncertain case");
    assert_eq!(
        uncertain.get("uncertain").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        object
            .get("authority")
            .and_then(|authority| authority.get("execution_authorized"))
            .and_then(Value::as_bool),
        Some(false)
    );
    let encoded = serde_json::to_string(&value).unwrap();
    assert!(!encoded.contains("fence-1"));
    assert!(!encoded.contains("transport ended after effect boundary"));
    assert!(!encoded.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
}

#[test]
fn runner_lease_fencing_preview_rejects_authority_unknown_and_duplicate_fields() {
    let original = fs::read(fixture_path()).unwrap();
    let mut fixture: Value = serde_json::from_slice(&original).unwrap();
    fixture["authority"]["execution_authorized"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerLeaseFencingPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["execution_authorized"] = Value::Bool(false);
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerLeaseFencingPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );

    let duplicate_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate_path.path(),
        br#"{"schema_version":"forge.runner-lease-fencing/v1","schema_version":"forge.runner-lease-fencing/v1"}"#,
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::RunnerLeaseFencingPreview {
            input: duplicate_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn runner_lease_fencing_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::RunnerLeaseFencingPreview {
        input: fixture_path().display().to_string(),
    })
    .unwrap();
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).unwrap();
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(rendered.contains("offline Runner lease fencing preview"));
    assert!(rendered.contains("terminal_replay: accepted=true replayed=true"));
    assert!(rendered.contains("authority: device_identity_verified=false"));
    assert!(!rendered.contains("fence-1"));
    assert!(!rendered.contains("transport ended after effect boundary"));
}
