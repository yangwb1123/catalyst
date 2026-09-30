use super::*;

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
fn device_runner_attempt_boundary_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "runner-attempt-boundary-preview",
        "--input",
        "attempt-boundary.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::RunnerAttemptBoundaryPreview {
            input: "attempt-boundary.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_runner_attempt_boundary_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "runner-attempt-boundary-preview"],
        vec![
            "device",
            "runner-attempt-boundary-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid Runner Attempt boundary preview arguments must fail"
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
