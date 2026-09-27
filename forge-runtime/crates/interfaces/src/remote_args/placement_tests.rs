use super::*;
use crate::args::{Command, parse_tokens};

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

#[test]
fn remote_scheduler_selection_preview_parses_a_bound_request_file() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "placement",
            "scheduler-preview",
            "--input",
            "scheduler.json",
        ])
        .unwrap(),
        RemoteCommand::SchedulerSelectionPreview {
            input: "scheduler.json".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(parse_remote_command(&["remote", "placement", "scheduler-preview"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "scheduler-preview",
            "--input",
            "scheduler.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
}

#[test]
fn remote_scheduler_selection_preview_accepts_an_instance_projection() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "placement",
            "scheduler-preview",
            "--input",
            "scheduler.json",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-view.json",
        ])
        .unwrap(),
        RemoteCommand::SchedulerSelectionPreview {
            input: "scheduler.json".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-view.json".into()),
        }
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "scheduler-preview",
            "--input",
            "scheduler.json",
            "--instance-view",
            "client-view.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "scheduler-preview",
            "--input",
            "scheduler.json",
            "--instance",
            "client-web-001",
            "--instance",
            "client-cli-001",
        ])
        .is_err()
    );
}

#[test]
fn remote_scheduler_lease_release_parses_a_regular_proof_file() {
    let args = parse_tokens(
        [
            "--idempotency-key",
            "scheduler-lease-release-1",
            "remote",
            "placement",
            "scheduler-lease-release",
            "--input",
            "release.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        args.idempotency_key.as_deref(),
        Some("scheduler-lease-release-1")
    );
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::SchedulerSelectionLeaseRelease {
            input: "release.json".into(),
            instance_id: None,
            instance_view: None,
        })
    );
    let missing_key = parse_tokens(
        [
            "remote",
            "placement",
            "scheduler-lease-release",
            "--input",
            "release.json",
        ]
        .map(str::to_owned),
    )
    .expect_err("scheduler lease release must require an explicit idempotency key");
    assert!(missing_key.contains("remote writes require an explicit --idempotency-key"));
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "scheduler-lease-release",
            "--input",
            "-",
        ])
        .is_err()
    );
}

#[test]
fn remote_scheduler_lease_lifecycle_accepts_instance_session_resource_projection() {
    assert_eq!(
        parse_remote_command(&[
            "--idempotency-key",
            "scheduler-lease-claim-1",
            "remote",
            "placement",
            "scheduler-lease",
            "--input",
            "request.json",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-instance-view.json",
        ])
        .unwrap(),
        RemoteCommand::SchedulerSelectionLease {
            input: "request.json".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-instance-view.json".into()),
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "--idempotency-key",
            "scheduler-lease-renew-1",
            "remote",
            "placement",
            "scheduler-lease-renew",
            "--input",
            "renewal.json",
            "--instance",
            "client-web-001",
            "--instance-view",
            "-",
        ])
        .unwrap(),
        RemoteCommand::SchedulerSelectionLeaseRenew {
            input: "renewal.json".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("-".into()),
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "--idempotency-key",
            "scheduler-lease-release-1",
            "remote",
            "placement",
            "scheduler-lease-release",
            "--input",
            "release.json",
            "--instance",
            "client-web-001",
        ])
        .unwrap(),
        RemoteCommand::SchedulerSelectionLeaseRelease {
            input: "release.json".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        }
    );
}

#[test]
fn remote_scheduler_lease_lifecycle_rejects_missing_instance_and_duplicate_projection_options() {
    for command in [
        "scheduler-lease",
        "scheduler-lease-renew",
        "scheduler-lease-release",
    ] {
        assert!(
            parse_remote_command(&[
                "remote",
                "placement",
                command,
                "--input",
                "request.json",
                "--instance-view",
                "client-instance-view.json",
            ])
            .is_err(),
            "{command} must reject --instance-view without --instance"
        );
        assert!(
            parse_remote_command(&[
                "remote",
                "placement",
                command,
                "--input",
                "request.json",
                "--instance",
                "client-web-001",
                "--instance",
                "client-cli-001",
            ])
            .is_err(),
            "{command} must reject duplicate --instance"
        );
        assert!(
            parse_remote_command(&[
                "remote",
                "placement",
                command,
                "--input",
                "request.json",
                "--instance",
                "client-web-001",
                "--instance-view",
                "first.json",
                "--instance-view",
                "second.json",
            ])
            .is_err(),
            "{command} must reject duplicate --instance-view"
        );
    }
}

#[test]
fn remote_runner_dispatch_admission_preview_parses_a_bounded_request_file() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "placement",
            "runner-dispatch-admission-preview",
            "--input",
            "admission.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerDispatchAdmissionPreview {
            input: "admission.json".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(
        parse_remote_command(&["remote", "placement", "runner-dispatch-admission-preview"])
            .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "runner-dispatch-admission-preview",
            "--input",
            "-",
            "--input",
            "other.json",
        ])
        .is_err()
    );
}

#[test]
fn remote_runner_transport_admission_preview_parses_a_bounded_request_file() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "placement",
            "runner-transport-admission-preview",
            "--input",
            "transport.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerTransportAdmissionPreview {
            input: "transport.json".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(
        parse_remote_command(&["remote", "placement", "runner-transport-admission-preview"])
            .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "runner-transport-admission-preview",
            "--input",
            "transport.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
}

#[test]
fn remote_runner_admission_preview_accepts_instance_projection_and_rejects_partial_options() {
    for command in [
        "runner-dispatch-admission-preview",
        "runner-transport-admission-preview",
    ] {
        let parsed = parse_remote_command(&[
            "remote",
            "placement",
            command,
            "--input",
            "admission.json",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-instance-view.json",
        ])
        .expect("instance projection");
        match parsed {
            RemoteCommand::RunnerDispatchAdmissionPreview {
                instance_id,
                instance_view,
                ..
            }
            | RemoteCommand::RunnerTransportAdmissionPreview {
                instance_id,
                instance_view,
                ..
            } => {
                assert_eq!(instance_id.as_deref(), Some("client-web-001"));
                assert_eq!(instance_view.as_deref(), Some("client-instance-view.json"));
            }
            _ => panic!("unexpected Runner admission command"),
        }
        assert!(
            parse_remote_command(&[
                "remote",
                "placement",
                command,
                "--input",
                "admission.json",
                "--instance-view",
                "client-instance-view.json",
            ])
            .is_err()
        );
    }
}

#[test]
fn remote_runner_execution_boundary_preview_parses_a_bounded_request_file() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "placement",
            "runner-execution-boundary-preview",
            "--input",
            "boundary.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerExecutionBoundaryPreview {
            input: "boundary.json".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(
        parse_remote_command(&["remote", "placement", "runner-execution-boundary-preview"])
            .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "placement",
            "runner-execution-boundary-preview",
            "--input",
            "boundary.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
}
