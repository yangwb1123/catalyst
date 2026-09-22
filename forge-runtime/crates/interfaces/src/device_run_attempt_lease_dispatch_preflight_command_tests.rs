use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{execute, write_output};
use crate::args::DeviceCommand;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json")
}

#[test]
fn preflight_preview_emits_the_canonical_fixture() {
    let output = execute(&DeviceCommand::RunAttemptLeaseDispatchPreflightPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Run/Attempt/lease preflight");
    let actual = serde_json::to_value(output).expect("JSON output");
    let expected: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual["selected_target_id"], Value::Null);
    assert_eq!(actual["preview_only"], true);
    assert_eq!(actual["authority"]["dispatch_performed"], false);
}

#[test]
fn preflight_preview_rejects_authority_selection_and_unknown_fields() {
    let mut fixture: Value = serde_json::from_slice(&fs::read(fixture_path()).unwrap()).unwrap();

    fixture["authority"]["dispatch_performed"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunAttemptLeaseDispatchPreflightPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["dispatch_performed"] = Value::Bool(false);
    fixture["selected_target_id"] = Value::String("runner-1".into());
    let selected_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(selected_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunAttemptLeaseDispatchPreflightPreview {
            input: selected_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["selected_target_id"] = Value::Null;
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::RunAttemptLeaseDispatchPreflightPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn preflight_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::RunAttemptLeaseDispatchPreflightPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("Run/Attempt/lease preflight");
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).expect("human output");
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(rendered.contains(
        "offline Run/Attempt/lease preflight [forge.run-attempt-lease-dispatch-preflight/v1]"
    ));
    assert!(rendered.contains("conversation=conversation-001 run=run-001 status=nonterminal"));
    assert!(rendered.contains("declarative_preflight_ready=true selected_target_id=null"));
    assert!(rendered.contains("dispatch_performed=false"));
    assert!(!rendered.contains("fencing_token"));
    assert!(!rendered.contains("argv"));
}
