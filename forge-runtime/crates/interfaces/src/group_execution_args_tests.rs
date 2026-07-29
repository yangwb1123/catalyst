use super::{Command, GroupCommand, GroupExecutionCommand, parse_tokens};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn start_requires_explicit_idempotency_for_recovery() {
    assert!(
        parse_error(&["group", "execution", "start", "group-run-1"])
            .contains("requires an explicit --idempotency-key for durable recovery")
    );

    let local = parse(&[
        "group",
        "execution",
        "start",
        "group-run-1",
        "--idempotency-key",
        "stable-execution",
    ]);
    assert_eq!(
        local.command,
        Command::Group(GroupCommand::Execution(GroupExecutionCommand::Start {
            group_run_id: "group-run-1".into(),
        }))
    );
    assert_eq!(local.idempotency_key.as_deref(), Some("stable-execution"));

    let global = parse(&[
        "--idempotency-key",
        "global-execution",
        "group",
        "execution",
        "start",
        "group-run-1",
    ]);
    assert_eq!(global.idempotency_key.as_deref(), Some("global-execution"));
}

#[test]
fn show_and_list_parse_without_a_key() {
    assert_eq!(
        parse(&["group", "execution", "show", "execution-1"]).command,
        Command::Group(GroupCommand::Execution(GroupExecutionCommand::Show {
            execution_id: "execution-1".into(),
        }))
    );
    assert_eq!(
        parse(&["group", "execution", "list", "group-run-1", "--limit", "7",]).command,
        Command::Group(GroupCommand::Execution(GroupExecutionCommand::List {
            group_run_id: Some("group-run-1".into()),
            limit: 7,
        }))
    );
    assert_eq!(
        parse(&["group", "execution", "list"]).command,
        Command::Group(GroupCommand::Execution(GroupExecutionCommand::List {
            group_run_id: None,
            limit: 50,
        }))
    );
}

#[test]
fn reads_reject_keys_and_all_commands_reject_extra_arguments() {
    let keyed_show = [
        "--idempotency-key",
        "unused",
        "group",
        "execution",
        "show",
        "execution-1",
    ];
    assert!(parse_error(&keyed_show).contains("only valid for mutating commands"));
    assert!(
        parse_error(&["group", "execution", "list", "--idempotency-key", "unused",])
            .contains("unexpected argument")
    );
    for tokens in [
        vec!["group", "execution", "start", "run-1", "--live"],
        vec!["group", "execution", "show", "execution-1", "extra"],
        vec!["group", "execution", "list", "run-1", "extra"],
    ] {
        assert!(parse_error(&tokens).contains("unexpected argument"));
    }
}

#[test]
fn ids_limits_duplicates_and_selectors_fail_during_parsing() {
    assert!(
        parse_error(&["group", "execution", "start", "--idempotency-key", "key"])
            .contains("requires GROUP_RUN_ID")
    );
    assert!(parse_error(&["group", "execution", "show"]).contains("requires EXECUTION_ID"));
    assert!(
        parse_error(&["group", "execution", "list", "--limit", "0"])
            .contains("--limit must be between 1")
    );
    assert!(
        parse_error(&[
            "--idempotency-key",
            "first",
            "group",
            "execution",
            "start",
            "run-1",
            "--idempotency-key",
            "second",
        ])
        .contains("specified more than once")
    );
    for tokens in [
        vec!["-C", "/srv/api", "group", "execution", "list"],
        vec![
            "--group",
            "group-1",
            "group",
            "execution",
            "show",
            "execution-1",
        ],
    ] {
        assert!(parse_error(&tokens).contains("selectors are not valid"));
    }
}
