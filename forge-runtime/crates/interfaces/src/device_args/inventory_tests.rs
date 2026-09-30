use super::*;

#[test]
fn device_inventory_show_requires_a_bounded_input_source() {
    let args = parse(&["device", "inventory", "show", "--input", "inventory.json"]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(DeviceInventoryCommand::Show {
            input: "inventory.json".into(),
        }))
    );
}

#[test]
fn device_inventory_persistence_preview_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "inventory",
        "persistence-preview",
        "--input",
        "persistence.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(
            DeviceInventoryCommand::PersistencePreview {
                input: "persistence.json".into(),
            },
        ))
    );
    assert!(args.json);
    for tokens in [
        vec!["device", "inventory", "persistence-preview"],
        vec![
            "device",
            "inventory",
            "persistence-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid device inventory persistence preview arguments must fail"
        );
    }
}

#[test]
fn device_inventory_persisted_observation_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "inventory",
        "persisted-observation",
        "--input",
        "persisted-observation.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(
            DeviceInventoryCommand::PersistedObservation {
                input: "persisted-observation.json".into(),
            },
        ))
    );
    assert!(args.json);
    for tokens in [
        vec!["device", "inventory", "persisted-observation"],
        vec![
            "device",
            "inventory",
            "persisted-observation",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid persisted inventory observation arguments must fail"
        );
    }
}

#[test]
fn device_inventory_resource_summary_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "inventory",
        "resource-summary",
        "--input",
        "resource-summary.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(
            DeviceInventoryCommand::ResourceSummary {
                input: "resource-summary.json".into(),
            },
        ))
    );
    assert!(args.json);
}

#[test]
fn device_inventory_session_observation_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "inventory",
        "session-observation",
        "--input",
        "session-device-observation.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(
            DeviceInventoryCommand::SessionObservation {
                input: "session-device-observation.json".into(),
            },
        ))
    );
    assert!(args.json);
}

#[test]
fn device_inventory_status_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "inventory",
        "status",
        "--input",
        "status.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(DeviceInventoryCommand::Status {
            input: "status.json".into(),
        }))
    );
    assert!(args.json);
}

#[test]
fn device_inventory_status_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "inventory", "status"],
        vec![
            "device",
            "inventory",
            "status",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid device inventory status arguments must fail"
        );
    }
}

#[test]
fn device_inventory_snapshot_canonical_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "inventory",
        "snapshot-canonical",
        "--input",
        "snapshot.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(
            DeviceInventoryCommand::SnapshotCanonical {
                input: "snapshot.json".into(),
            },
        ))
    );
    assert!(args.json);
}

#[test]
fn device_inventory_snapshot_canonical_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "inventory", "snapshot-canonical"],
        vec![
            "device",
            "inventory",
            "snapshot-canonical",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid device inventory snapshot arguments must fail"
        );
    }
}

#[test]
fn device_inventory_resource_summary_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "inventory", "resource-summary"],
        vec![
            "device",
            "inventory",
            "resource-summary",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid device resource summary arguments must fail"
        );
    }
}

#[test]
fn device_inventory_session_observation_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "inventory", "session-observation"],
        vec![
            "device",
            "inventory",
            "session-observation",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid session device observation arguments must fail"
        );
    }
}

#[test]
fn device_inventory_show_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "inventory", "show"],
        vec![
            "device",
            "inventory",
            "show",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid device inventory arguments must fail"
        );
    }
}

#[test]
fn device_inventory_placement_evaluation_v2_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "inventory",
        "placement-evaluation-v2",
        "--input",
        "placement-v2.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Inventory(
            DeviceInventoryCommand::PlacementEvaluationV2 {
                input: "placement-v2.json".into(),
            },
        ))
    );
    assert!(args.json);
}

#[test]
fn device_inventory_placement_evaluation_v2_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "inventory", "placement-evaluation-v2"],
        vec![
            "device",
            "inventory",
            "placement-evaluation-v2",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid v2 placement-evaluation arguments must fail"
        );
    }
}
