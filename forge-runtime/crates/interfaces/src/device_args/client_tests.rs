use super::*;

#[test]
fn device_client_session_view_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "client-session-view-preview",
        "--input",
        "client-view.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::ClientSessionViewPreview {
            input: "client-view.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_client_session_view_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "client-session-view-preview"],
        vec![
            "device",
            "client-session-view-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid client-instance/session-view preview arguments must fail"
        );
    }
}

#[test]
fn device_client_instance_resource_view_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "client-instance-resource-view-preview",
        "--input",
        "client-resource-view.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::ClientInstanceResourceViewPreview {
            input: "client-resource-view.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_client_instance_resource_view_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "client-instance-resource-view-preview"],
        vec![
            "device",
            "client-instance-resource-view-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid client-instance/resource-view preview arguments must fail"
        );
    }
}

#[test]
fn device_client_instance_scheduler_selection_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "client-instance-scheduler-selection-preview",
        "--input",
        "instance-scheduler.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::ClientInstanceSchedulerSelectionPreview {
            input: "instance-scheduler.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_client_instance_scheduler_selection_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "client-instance-scheduler-selection-preview"],
        vec![
            "device",
            "client-instance-scheduler-selection-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid instance-scoped scheduler preview arguments must fail"
        );
    }
}

#[test]
fn device_heartbeat_persistence_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "heartbeat-persistence-preview",
        "--input",
        "heartbeat.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::HeartbeatPersistencePreview {
            input: "heartbeat.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_heartbeat_persistence_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "heartbeat-persistence-preview"],
        vec![
            "device",
            "heartbeat-persistence-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid heartbeat persistence preview arguments must fail"
        );
    }
}

#[test]
fn device_identity_proof_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "identity-proof-preview",
        "--input",
        "identity.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::IdentityProofPreview {
            input: "identity.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_identity_proof_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "identity-proof-preview"],
        vec![
            "device",
            "identity-proof-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid identity proof preview arguments must fail"
        );
    }
}
