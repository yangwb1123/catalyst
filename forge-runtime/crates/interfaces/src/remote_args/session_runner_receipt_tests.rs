use crate::args::{Command, RemoteCommand, parse_tokens};

#[test]
fn remote_session_runner_receipt_preview_parses_bounded_input() {
    let parsed = parse_tokens(
        [
            "remote",
            "session-runner-receipt",
            "preview",
            "--input",
            "receipt.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::SessionRunnerReceiptPreview {
            input: "receipt.json".into(),
        })
    );
}

#[test]
fn remote_session_runner_receipt_preview_rejects_missing_or_duplicate_input() {
    assert!(
        parse_tokens(["remote", "session-runner-receipt", "preview"].map(str::to_owned)).is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "session-runner-receipt",
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
                "session-runner-receipt",
                "preview",
                "--input",
                " "
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
}
