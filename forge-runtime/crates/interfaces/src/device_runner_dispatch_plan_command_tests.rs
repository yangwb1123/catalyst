use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{DeviceCommand, execute, write_output};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json")
}

#[test]
fn runner_dispatch_plan_preview_consumes_the_go_observation_fixture() {
    let output = execute(&DeviceCommand::RunnerDispatchPlanPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Runner dispatch-plan preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.runner-dispatch-plan-preview/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("pure_dispatch_plan_preview_only")
    );
    assert_eq!(
        object.get("candidate_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        object
            .get("declarative_ready_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(object.get("selected_target_id").is_some_and(Value::is_null));
    assert_eq!(
        object
            .get("authority")
            .and_then(|authority| authority.get("reservation_created"))
            .and_then(Value::as_bool),
        Some(false)
    );
    let candidates = object
        .get("candidates")
        .and_then(Value::as_array)
        .expect("candidates");
    assert_eq!(
        candidates[0].get("target_id").and_then(Value::as_str),
        Some("runner-1")
    );
    assert_eq!(
        candidates[0]
            .get("declarative_ready")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        candidates[1].get("reasons").and_then(Value::as_array),
        Some(&vec![Value::String("lease_target_mismatch".into())])
    );
}

#[test]
fn runner_dispatch_plan_preview_rejects_authority_selection_unknown_and_duplicate_fields() {
    let original = fs::read(fixture_path()).unwrap();
    let mut fixture: Value = serde_json::from_slice(&original).unwrap();

    fixture["authority"]["dispatch_performed"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerDispatchPlanPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["dispatch_performed"] = Value::Bool(false);
    fixture["selected_target_id"] = Value::String("runner-1".into());
    let selected_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(selected_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerDispatchPlanPreview {
            input: selected_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["selected_target_id"] = Value::Null;
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunnerDispatchPlanPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );

    let duplicate_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate_path.path(),
        br#"{"schema_version":"forge.runner-dispatch-plan-preview/v1","schema_version":"forge.runner-dispatch-plan-preview/v1"}"#,
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::RunnerDispatchPlanPreview {
            input: duplicate_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn runner_dispatch_plan_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::RunnerDispatchPlanPreview {
        input: fixture_path().display().to_string(),
    })
    .unwrap();
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).unwrap();
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(rendered.contains("offline Runner dispatch-plan preview"));
    assert!(rendered.contains("candidate runner-1:"));
    assert!(rendered.contains("selected_target=none"));
    assert!(rendered.contains("authority: device_identity_verified=false"));
    assert!(!rendered.contains("fencing_token"));
    assert!(!rendered.contains("argv"));
}
