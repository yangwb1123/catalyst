use crate::args::{Command, RemoteCommand, parse_tokens};

#[test]
fn remote_session_runner_receipt_history_preview_parses_bounded_input() {
    let parsed = parse_tokens(
        [
            "remote",
            "session-runner-receipt-history",
            "preview",
            "--input",
            "history.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::SessionRunnerReceiptHistoryPreview {
            input: "history.json".into(),
        })
    );
}

#[test]
fn remote_session_runner_receipt_history_preview_rejects_missing_or_duplicate_input() {
    assert!(
        parse_tokens(["remote", "session-runner-receipt-history", "preview"].map(str::to_owned),)
            .is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "session-runner-receipt-history",
                "preview",
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
                "session-runner-receipt-history",
                "preview",
                "--input",
                " ",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
}
