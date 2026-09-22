use super::*;

#[test]
fn remote_placement_preview_parses_a_bounded_input_path_without_write_authority() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "placement",
            "preview",
            "--input",
            "placement.json",
        ])
        .unwrap(),
        RemoteCommand::PlacementPreview {
            input: "placement.json".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "placement", "preview"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "preview",
            "--input",
            "placement.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "--idempotency-key",
            "unexpected",
            "remote",
            "placement",
            "preview",
            "--input",
            "placement.json",
        ])
        .is_err()
    );
}

#[test]
fn remote_registry_placement_preview_parses_a_requirements_file() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "placement",
            "registry-preview",
            "--input",
            "requirements.json",
        ])
        .unwrap(),
        RemoteCommand::PlacementRegistryPreview {
            input: "requirements.json".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "placement", "registry-preview"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "registry-preview",
            "--input",
            "requirements.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
}
