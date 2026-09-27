use crate::args::{Command, RemoteCommand, parse_tokens};

#[test]
fn remote_session_runner_reconciliation_preview_requires_one_explicit_input() {
    let parsed = parse_tokens(
        [
            "remote",
            "session-runner-reconciliation",
            "preview",
            "--input",
            "projection.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::SessionRunnerReconciliationPreview {
            input: "projection.json".into(),
        })
    );

    assert!(
        parse_tokens(["remote", "session-runner-reconciliation", "preview"].map(str::to_owned))
            .is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "session-runner-reconciliation",
                "preview",
                "--input",
                "a.json",
                "--input",
                "b.json",
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
}

#[test]
fn remote_session_runner_reconciliation_remote_preview_is_explicit() {
    let parsed = parse_tokens(
        [
            "remote",
            "session-runner-reconciliation",
            "remote-preview",
            "--input",
            "history.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::SessionRunnerReconciliationRemotePreview {
            input: "history.json".into(),
        })
    );
}
