use super::*;

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
fn device_session_runner_receipt_history_preview_accepts_a_bounded_input_source() {
    let args = parse(&[
        "--json",
        "device",
        "session-runner-receipt-history-preview",
        "--input",
        "session-receipt-history.json",
    ]);
    assert_eq!(
        args.command,
        Command::Device(DeviceCommand::SessionRunnerReceiptHistoryPreview {
            input: "session-receipt-history.json".into(),
        })
    );
    assert!(args.json);
}

#[test]
fn device_session_runner_receipt_history_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["device", "session-runner-receipt-history-preview"],
        vec![
            "device",
            "session-runner-receipt-history-preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(
            parse_tokens(tokens.into_iter().map(str::to_owned)).is_err(),
            "invalid session Runner receipt history preview arguments must fail"
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
