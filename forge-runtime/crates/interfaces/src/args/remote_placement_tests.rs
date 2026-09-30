use super::*;

#[test]
fn scheduler_lease_requires_and_accepts_an_explicit_idempotency_key() {
    let args = parse(&[
        "--json",
        "--idempotency-key",
        "scheduler-lease-1",
        "remote",
        "placement",
        "scheduler-lease",
        "--input",
        "request.json",
    ]);
    assert_eq!(args.idempotency_key.as_deref(), Some("scheduler-lease-1"));
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::SchedulerSelectionLease {
            input: "request.json".into(),
            instance_id: None,
            instance_view: None,
        })
    );

    let error = parse_tokens(
        [
            "remote",
            "placement",
            "scheduler-lease",
            "--input",
            "request.json",
        ]
        .map(str::to_owned),
    )
    .expect_err("scheduler lease must not silently invent an idempotency key");
    assert!(error.contains("remote writes require an explicit --idempotency-key"));
}

#[test]
fn scheduler_lease_renewal_requires_and_accepts_an_explicit_idempotency_key() {
    let args = parse(&[
        "--json",
        "--idempotency-key",
        "scheduler-lease-renew-1",
        "remote",
        "placement",
        "scheduler-lease-renew",
        "--input",
        "renewal.json",
    ]);
    assert_eq!(
        args.idempotency_key.as_deref(),
        Some("scheduler-lease-renew-1")
    );
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::SchedulerSelectionLeaseRenew {
            input: "renewal.json".into(),
            instance_id: None,
            instance_view: None,
        })
    );

    let error = parse_tokens(
        [
            "remote",
            "placement",
            "scheduler-lease-renew",
            "--input",
            "renewal.json",
        ]
        .map(str::to_owned),
    )
    .expect_err("scheduler lease renewal must not silently invent an idempotency key");
    assert!(error.contains("remote writes require an explicit --idempotency-key"));
}

#[test]
fn scheduler_lease_release_requires_and_accepts_an_explicit_idempotency_key() {
    let args = parse(&[
        "--json",
        "--idempotency-key",
        "scheduler-lease-release-1",
        "remote",
        "placement",
        "scheduler-lease-release",
        "--input",
        "release.json",
    ]);
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

    let error = parse_tokens(
        [
            "remote",
            "placement",
            "scheduler-lease-release",
            "--input",
            "release.json",
        ]
        .map(str::to_owned),
    )
    .expect_err("scheduler lease release must not silently invent an idempotency key");
    assert!(error.contains("remote writes require an explicit --idempotency-key"));
}

#[test]
fn runner_dispatch_admission_preview_is_read_only_and_has_no_idempotency_key() {
    let args = parse(&[
        "--json",
        "remote",
        "placement",
        "runner-dispatch-admission-preview",
        "--input",
        "admission.json",
    ]);
    assert_eq!(args.idempotency_key, None);
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::RunnerDispatchAdmissionPreview {
            input: "admission.json".into(),
            instance_id: None,
            instance_view: None,
        })
    );
    assert!(
        parse_tokens(
            [
                "--idempotency-key",
                "unexpected",
                "remote",
                "placement",
                "runner-dispatch-admission-preview",
                "--input",
                "admission.json",
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
}

#[test]
fn runner_transport_admission_preview_is_read_only_and_requires_input() {
    let args = parse(&[
        "--json",
        "remote",
        "placement",
        "runner-transport-admission-preview",
        "--input",
        "transport.json",
    ]);
    assert_eq!(args.idempotency_key, None);
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::RunnerTransportAdmissionPreview {
            input: "transport.json".into(),
            instance_id: None,
            instance_view: None,
        })
    );
    assert!(
        parse_tokens(
            ["remote", "placement", "runner-transport-admission-preview"].map(str::to_owned)
        )
        .is_err()
    );
}

#[test]
fn runner_execution_boundary_preview_is_read_only_and_requires_input() {
    let args = parse(&[
        "--json",
        "remote",
        "placement",
        "runner-execution-boundary-preview",
        "--input",
        "boundary.json",
    ]);
    assert_eq!(args.idempotency_key, None);
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::RunnerExecutionBoundaryPreview {
            input: "boundary.json".into(),
            instance_id: None,
            instance_view: None,
        })
    );
    assert!(
        parse_tokens(
            ["remote", "placement", "runner-execution-boundary-preview"].map(str::to_owned)
        )
        .is_err()
    );
}
