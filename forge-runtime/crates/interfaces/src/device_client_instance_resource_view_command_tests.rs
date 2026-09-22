use std::{fs, path::PathBuf};

use serde_json::Value;

use super::{DeviceCommand, execute, write_output};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json")
}

#[test]
fn client_instance_resource_view_preview_consumes_canonical_fixture() {
    let output = execute(&DeviceCommand::ClientInstanceResourceViewPreview {
        input: fixture_path().display().to_string(),
    })
    .expect("client-instance/resource-view preview");
    assert_eq!(
        output.schema_version,
        "forge.client-instance-resource-view/v1"
    );
    assert_eq!(
        output.evaluation_mode,
        "owner_bound_instance_resource_view_only"
    );
    assert!(output.owner_declaration_unverified);
    assert!(output.device_attributes_unverified);
    assert!(output.read_only);
    assert_eq!(output.instances.len(), 5);
    assert_eq!(output.instances[0].instance_id, "client-app-001");
    assert_eq!(output.instances[0].client_kind, "app");
    assert_eq!(output.instances[2].client_kind, "mobile");
    assert_eq!(output.instances[3].client_kind, "tui");
    assert_eq!(output.instances[4].client_kind, "web");
    assert_eq!(output.devices.len(), 2);
    assert_eq!(output.devices[0].device_id, "device-a");
    assert_eq!(output.devices[0].runner_instance_id, "runner-a");
    assert_eq!(output.devices[0].gpu_count, 2);
    assert!(!output.authority.owner_authenticated);
    assert!(!output.authority.reservation_created);
}

#[test]
fn client_instance_resource_view_preview_rejects_authority_unknown_duplicate_and_ordering() {
    let original = fs::read(fixture_path()).unwrap();
    let mut fixture: Value = serde_json::from_slice(&original).unwrap();

    fixture["authority"]["reservation_created"] = Value::Bool(true);
    let path = tempfile::NamedTempFile::new().unwrap();
    fs::write(path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ClientInstanceResourceViewPreview {
            input: path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["authority"]["reservation_created"] = Value::Bool(false);
    fixture["unexpected"] = Value::Bool(true);
    fs::write(path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ClientInstanceResourceViewPreview {
            input: path.path().display().to_string(),
        })
        .is_err()
    );

    fixture.as_object_mut().unwrap().remove("unexpected");
    fixture["devices"][0]["owner"]["subject"] = Value::String("other-user".into());
    fs::write(path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ClientInstanceResourceViewPreview {
            input: path.path().display().to_string(),
        })
        .is_err()
    );

    fixture["devices"][0]["owner"]["subject"] = Value::String("user-1".into());
    fixture["devices"] = Value::Array(
        fixture["devices"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .cloned()
            .collect(),
    );
    fs::write(path.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    assert!(
        execute(&DeviceCommand::ClientInstanceResourceViewPreview {
            input: path.path().display().to_string(),
        })
        .is_err()
    );

    let duplicate = tempfile::NamedTempFile::new().unwrap();
    fs::write(
        duplicate.path(),
        br#"{"schema_version":"forge.client-instance-resource-view/v1","schema_version":"forge.client-instance-resource-view/v1"}"#,
    )
    .unwrap();
    assert!(
        execute(&DeviceCommand::ClientInstanceResourceViewPreview {
            input: duplicate.path().display().to_string(),
        })
        .is_err()
    );
}

#[test]
fn client_instance_resource_view_preview_human_output_is_metadata_only() {
    let output = execute(&DeviceCommand::ClientInstanceResourceViewPreview {
        input: fixture_path().display().to_string(),
    })
    .unwrap();
    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).unwrap();
    let rendered = String::from_utf8(rendered).unwrap();
    assert!(rendered.contains(
        "offline client-instance/resource-view [forge.client-instance-resource-view/v1]"
    ));
    assert!(
        rendered.contains("device device-a: runner=runner-a revision=1 generation=1 heartbeat=1")
    );
    assert!(rendered.contains("prompt_write_authorized=false"));
    assert!(!rendered.contains("fencing"));
    assert!(!rendered.contains("argv"));
    assert!(!rendered.contains("output"));
}
