use crate::args::{Command, RemoteCommand, parse_tokens};

#[test]
fn remote_local_runner_preview_parses_bounded_input() {
    let parsed = parse_tokens(
        [
            "remote",
            "runner",
            "execution-readiness-preview",
            "--input",
            "request.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::LocalRunnerPreview {
            input: "request.json".into(),
        })
    );
}

#[test]
fn remote_local_runner_preview_rejects_missing_or_duplicate_input() {
    assert!(
        parse_tokens(["remote", "runner", "execution-readiness-preview"].map(str::to_owned))
            .is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "runner",
                "execution-readiness-preview",
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
                "runner",
                "execution-readiness-preview",
                "--input",
                " ",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
}
