use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{DeviceCommand, execute, write_output};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json")
}

#[test]
fn client_instance_session_view_preview_consumes_the_canonical_fixture() {
    let output = execute(&DeviceCommand::ClientSessionViewPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("client-instance/session-view preview");
    let value = serde_json::to_value(output).expect("JSON output");
    let object = value.as_object().expect("object output");
    assert_eq!(
        object.get("schema_version").and_then(Value::as_str),
        Some("forge.client-instance-session-view/v1")
    );
    assert_eq!(
        object.get("evaluation_mode").and_then(Value::as_str),
        Some("owner_bound_session_view_only")
    );
    assert_eq!(
        object
            .get("owner_declaration_unverified")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(object.get("read_only").and_then(Value::as_bool), Some(true));
    let instances = object
        .get("instances")
        .and_then(Value::as_array)
        .expect("instances");
    assert_eq!(instances.len(), 5);
    assert_eq!(
        instances[0].get("instance_id").and_then(Value::as_str),
        Some("client-app-001")
    );
    assert_eq!(
        instances[0].get("client_kind").and_then(Value::as_str),
        Some("app")
    );
    assert_eq!(
        instances[2].get("client_kind").and_then(Value::as_str),
        Some("mobile")
    );
    assert_eq!(
        instances[3].get("client_kind").and_then(Value::as_str),
        Some("tui")
    );
    assert_eq!(
        instances[4].get("client_kind").and_then(Value::as_str),
        Some("web")
    );
    assert_eq!(
        object
            .get("authority")
            .and_then(|authority| authority.get("prompt_write_authorized"))
            .and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn client_instance_session_view_preview_rejects_authority_unknown_and_duplicate_fields() {
    let original = fs::read(fixture_path()).unwrap();
    let mut fixture: Value = serde_json::from_slice(&original).unwrap();

    fixture["authority"]["session_read_authorized"] = Value::Bool(true);
    let authority_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(authority_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ClientSessionViewPreview {
            input: authority_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["session_read_authorized"] = Value::Bool(false);
    fixture["instances"][0]["instance_id"] = Value::String("client-cli-001".into());
    let duplicate_instance_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate_instance_path.path(),
        serde_json::to_vec(&fixture).unwrap(),
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::ClientSessionViewPreview {
            input: duplicate_instance_path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["instances"][0]["instance_id"] = Value::String("client-app-001".into());
    fixture["unexpected"] = Value::Bool(true);
    let unknown_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(unknown_path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ClientSessionViewPreview {
            input: unknown_path.path().display().to_string(),
        })
        .is_err()
    );

    let duplicate_path = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate_path.path(),
        br#"{"schema_version":"forge.client-instance-session-view/v1","schema_version":"forge.client-instance-session-view/v1"}"#,
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::ClientSessionViewPreview {
            input: duplicate_path.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn client_instance_session_view_preview_human_output_is_read_only_metadata() {
    let output = execute(&DeviceCommand::ClientSessionViewPreview {
        input: fixture_path().display().to_string(),
    })
    .unwrap();
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).unwrap();
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(rendered.contains("offline client-instance/session-view"));
    assert!(rendered.contains("instance client-cli-001"));
    assert!(rendered.contains("read_only=true"));
    assert!(rendered.contains("prompt_write_authorized=false"));
    assert!(!rendered.contains("prompt_text"));
}
