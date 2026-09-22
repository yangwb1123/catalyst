use crate::args::{Command, RemoteCommand, parse_tokens};

#[test]
fn remote_run_attempt_lease_dispatch_preflight_parses_bounded_input() {
    let parsed = parse_tokens(
        [
            "remote",
            "run-attempt-lease-dispatch-preflight-preview",
            "--input",
            "request.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::RunAttemptLeaseDispatchPreflightPreview {
            input: "request.json".into(),
        })
    );
}

#[test]
fn remote_run_attempt_lease_dispatch_preflight_rejects_missing_or_duplicate_input() {
    assert!(
        parse_tokens(["remote", "run-attempt-lease-dispatch-preflight-preview"].map(str::to_owned))
            .is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "run-attempt-lease-dispatch-preflight-preview",
                "--input",
                "a.json",
                "--input",
                "b.json",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "run-attempt-lease-dispatch-preflight-preview",
                "--input",
                " ",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
}
