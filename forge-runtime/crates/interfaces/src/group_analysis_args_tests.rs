use super::{Command, GroupAnalysisCommand, GroupCommand, parse_tokens};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn prepare_binds_model_limits_and_optional_recovery_key() {
    let defaults = parse(&["group", "analysis", "prepare", "group-run-1"]);
    assert_eq!(
        defaults.command,
        Command::Group(GroupCommand::Analysis(GroupAnalysisCommand::Prepare {
            group_run_id: "group-run-1".into(),
            model: None,
            max_output_tokens: 4_096,
        }))
    );

    let configured = parse(&[
        "group",
        "analysis",
        "prepare",
        "group-run-1",
        "--model",
        "requested-model",
        "--max-output-tokens",
        "8192",
        "--idempotency-key",
        "analysis-key",
    ]);
    assert_eq!(configured.idempotency_key.as_deref(), Some("analysis-key"));
    assert_eq!(
        configured.command,
        Command::Group(GroupCommand::Analysis(GroupAnalysisCommand::Prepare {
            group_run_id: "group-run-1".into(),
            model: Some("requested-model".into()),
            max_output_tokens: 8_192,
        }))
    );
}

#[test]
fn send_records_explicit_consent_and_result_visibility_separately() {
    let args = parse(&[
        "group",
        "analysis",
        "send",
        "analysis-1",
        "--confirm-off-machine",
        "--include-result",
    ]);

    assert_eq!(
        args.command,
        Command::Group(GroupCommand::Analysis(GroupAnalysisCommand::Send {
            analysis_id: "analysis-1".into(),
            confirm_off_machine: true,
            include_result: true,
        }))
    );
}

#[test]
fn show_and_list_are_metadata_safe_by_default() {
    assert_eq!(
        parse(&["group", "analysis", "show", "analysis-1"]).command,
        Command::Group(GroupCommand::Analysis(GroupAnalysisCommand::Show {
            analysis_id: "analysis-1".into(),
            include_result: false,
        }))
    );
    assert_eq!(
        parse(&["group", "analysis", "list", "group-run-1", "--limit", "7",]).command,
        Command::Group(GroupCommand::Analysis(GroupAnalysisCommand::List {
            group_run_id: Some("group-run-1".into()),
            limit: 7,
        }))
    );
}

#[test]
fn analysis_options_fail_closed() {
    for tokens in [
        vec!["group", "analysis", "prepare"],
        vec![
            "group",
            "analysis",
            "prepare",
            "run-1",
            "--max-output-tokens",
            "0",
        ],
        vec![
            "group",
            "analysis",
            "prepare",
            "run-1",
            "--allow-read",
            "secret",
        ],
        vec![
            "group",
            "analysis",
            "send",
            "analysis-1",
            "--confirm-off-machine",
            "--confirm-off-machine",
        ],
        vec![
            "group",
            "analysis",
            "show",
            "analysis-1",
            "--confirm-off-machine",
        ],
        vec!["group", "analysis", "list", "--limit", "0"],
    ] {
        assert!(parse_error(&tokens).contains("usage:"));
    }
}

#[test]
fn analysis_selectors_and_keys_on_effect_or_reads_are_rejected() {
    for tokens in [
        vec!["-C", "/srv/api", "group", "analysis", "list"],
        vec![
            "--group",
            "group-1",
            "group",
            "analysis",
            "show",
            "analysis-1",
        ],
    ] {
        assert!(parse_error(&tokens).contains("selectors are not valid"));
    }

    let keyed_send = parse_error(&[
        "--idempotency-key",
        "wrong-scope",
        "group",
        "analysis",
        "send",
        "analysis-1",
        "--confirm-off-machine",
    ]);
    assert!(keyed_send.contains("ANALYSIS_ID owns the single dispatch claim"));
}
