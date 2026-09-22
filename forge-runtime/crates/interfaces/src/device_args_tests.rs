use super::{Command, DeviceCommand, DeviceInventoryCommand, DevicePlacementCommand, parse_tokens};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

#[test]
fn device_attempt_request_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "attempt-request-preview",
        "--input",
        "attempt.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::AttemptRequestPreview {
            input: "attempt.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_attempt_request_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "attempt-request-preview"],
        vec![
            "device",
            "attempt-request-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Attempt request preview arguments must fail"
        );
    }
}

#[test]
fn device_placement_dry_run_requires_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "placement",
        "dry-run",
        "--input",
        "inventory.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Placement(DevicePlacementCommand::DryRun {
            input: "inventory.json".into(),
        },))
    );
    assert!(args.json);
}

#[test]
fn device_placement_dry_run_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "placement", "dry-run"],
        vec![
            "device",
            "placement",
            "dry-run",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid device placement arguments must fail"
        );
    }
}

#[test]
fn device_runner_receipt_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "runner-receipt-preview",
        "--input",
        "receipt.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::RunnerReceiptPreview {
            input: "receipt.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_runner_receipt_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "runner-receipt-preview"],
        vec![
            "device",
            "runner-receipt-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Runner terminal receipt preview arguments must fail"
        );
    }
}

#[test]
fn device_runner_lease_fencing_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "runner-lease-fencing-preview",
        "--input",
        "lease.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::RunnerLeaseFencingPreview {
            input: "lease.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_runner_lease_fencing_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "runner-lease-fencing-preview"],
        vec![
            "device",
            "runner-lease-fencing-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Runner lease fencing preview arguments must fail"
        );
    }
}

#[test]
fn device_execution_lease_checkpoint_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "execution-lease-checkpoint-preview",
        "--input",
        "checkpoint.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::ExecutionLeaseCheckpointPreview {
            input: "checkpoint.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_execution_lease_checkpoint_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "execution-lease-checkpoint-preview"],
        vec![
            "device",
            "execution-lease-checkpoint-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid execution lease checkpoint preview arguments must fail"
        );
    }
}

#[test]
fn device_runner_dispatch_plan_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "runner-dispatch-plan-preview",
        "--input",
        "dispatch-plan.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::RunnerDispatchPlanPreview {
            input: "dispatch-plan.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_runner_dispatch_plan_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "runner-dispatch-plan-preview"],
        vec![
            "device",
            "runner-dispatch-plan-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Runner dispatch-plan preview arguments must fail"
        );
    }
}

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

#[test]
fn device_runner_execution_intent_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "runner-execution-intent-preview",
        "--input",
        "execution-intent.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::RunnerExecutionIntentPreview {
            input: "execution-intent.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_runner_execution_intent_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "runner-execution-intent-preview"],
        vec![
            "device",
            "runner-execution-intent-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Runner execution intent preview arguments must fail"
        );
    }
}

#[test]
fn device_session_runner_receipt_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "session-runner-receipt-preview",
        "--input",
        "session-receipt.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::SessionRunnerReceiptPreview {
            input: "session-receipt.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_session_runner_receipt_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "session-runner-receipt-preview"],
        vec![
            "device",
            "session-runner-receipt-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid session Runner receipt preview arguments must fail"
        );
    }
}

#[test]
fn device_run_observed_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "run-observed-preview",
        "--input",
        "run-observed.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::RunObservedPreview {
            input: "run-observed.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_run_observed_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "run-observed-preview"],
        vec![
            "device",
            "run-observed-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Run observed preview arguments must fail"
        );
    }
}

#[test]
fn device_run_intent_preview_requires_two_bounded_inputs() {
    let args = parse(&[
        "--json",
        "device",
        "placement",
        "run-intent-preview",
        "--input",
        "run.json",
        "--placement-input",
        "placement.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::Placement(
            DevicePlacementCommand::RunIntentPreview {
                input: "run.json".into(),
                placement_input: "placement.json".into(),
            },
        ))
    );
    assert!(args.json);
}

#[test]
fn device_run_intent_preview_rejects_missing_duplicate_and_double_stdin_inputs() {
    for tokens in [
        vec![
            "device",
            "placement",
            "run-intent-preview",
            "--input",
            "run.json",
        ],
        vec![
            "device",
            "placement",
            "run-intent-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
            "--placement-input",
            "placement.json",
        ],
        vec![
            "device",
            "placement",
            "run-intent-preview",
            "--input",
            "-",
            "--placement-input",
            "-",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Run-intent preview arguments must fail"
        );
    }
}

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

#[test]
fn device_run_attempt_lease_dispatch_preflight_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "run-attempt-lease-dispatch-preflight-preview",
        "--input",
        "preflight.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::RunAttemptLeaseDispatchPreflightPreview {
            input: "preflight.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_run_attempt_lease_dispatch_preflight_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "run-attempt-lease-dispatch-preflight-preview"],
        vec![
            "device",
            "run-attempt-lease-dispatch-preflight-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Run/Attempt/lease preflight arguments must fail"
        );
    }
}
