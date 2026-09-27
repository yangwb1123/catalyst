use super::*;

#[test]
fn remote_run_execution_evidence_preview_parses_bounded_input() {
    let parsed = parse_tokens(
        [
            "remote",
            "run-execution-evidence",
            "preview",
            "--input",
            "evidence.json",
        ]
        .map(str::to_owned),
    )
    .expect("Run execution evidence preview parses");
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::RunExecutionEvidencePreview {
            input: "evidence.json".into()
        })
    );
}

#[test]
fn remote_run_execution_evidence_preview_rejects_missing_or_duplicate_input() {
    for tokens in [
        vec!["remote", "run-execution-evidence", "preview"],
        vec![
            "remote",
            "run-execution-evidence",
            "preview",
            "--input",
            "one.json",
            "--input",
            "two.json",
        ],
    ] {
        assert!(parse_tokens(tokens.into_iter().map(str::to_owned)).is_err());
    }
}
