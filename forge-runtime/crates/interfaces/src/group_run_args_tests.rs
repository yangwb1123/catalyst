use forge_runtime_application::DEFAULT_GROUP_CONTEXT_CONTENT_BYTES;

use super::{Command, GroupCommand, GroupRunCommand, parse_tokens};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn group_run_prepare_defaults_to_a_redacted_bounded_snapshot() {
    let args = parse(&["group", "run", "prepare", "group-1"]);

    assert_eq!(
        args.command,
        Command::Group(GroupCommand::Run(GroupRunCommand::Prepare {
            group_id: "group-1".into(),
            include_content: false,
            max_bytes: DEFAULT_GROUP_CONTEXT_CONTENT_BYTES,
        }))
    );
}

#[test]
fn group_run_prepare_accepts_explicit_content_bound_and_local_key() {
    let args = parse(&[
        "group",
        "run",
        "prepare",
        "group-1",
        "--include-content",
        "--max-bytes",
        "2048",
        "--idempotency-key",
        "stable-freeze",
    ]);

    assert_eq!(args.idempotency_key.as_deref(), Some("stable-freeze"));
    assert_eq!(
        args.command,
        Command::Group(GroupCommand::Run(GroupRunCommand::Prepare {
            group_id: "group-1".into(),
            include_content: true,
            max_bytes: 2_048,
        }))
    );
}

#[test]
fn group_run_prepare_retains_the_global_key_form() {
    let args = parse(&[
        "--idempotency-key",
        "global-key",
        "group",
        "run",
        "prepare",
        "group-1",
    ]);

    assert_eq!(args.idempotency_key.as_deref(), Some("global-key"));
}

#[test]
fn group_run_show_and_list_parse_without_execution_selectors() {
    assert_eq!(
        parse(&["group", "run", "show", "group-run-1", "--include-content"]).command,
        Command::Group(GroupCommand::Run(GroupRunCommand::Show {
            run_id: "group-run-1".into(),
            include_content: true,
        }))
    );
    assert_eq!(
        parse(&["group", "run", "list", "group-1", "--limit", "7"]).command,
        Command::Group(GroupCommand::Run(GroupRunCommand::List {
            group_id: Some("group-1".into()),
            limit: 7,
        }))
    );
    assert_eq!(
        parse(&["group", "run", "list"]).command,
        Command::Group(GroupCommand::Run(GroupRunCommand::List {
            group_id: None,
            limit: 50,
        }))
    );
}

#[test]
fn group_run_ids_and_numeric_bounds_fail_during_argument_parsing() {
    let missing_group = parse_error(&["group", "run", "prepare", "--include-content"]);
    let missing_run = parse_error(&["group", "run", "show", "--include-content"]);
    let zero_bytes = parse_error(&["group", "run", "prepare", "group-1", "--max-bytes", "0"]);
    let zero_limit = parse_error(&["group", "run", "list", "--limit", "0"]);

    assert!(missing_group.contains("requires GROUP_ID"));
    assert!(missing_run.contains("requires RUN_ID"));
    assert!(zero_bytes.contains("--max-bytes must be between 1"));
    assert!(zero_limit.contains("--limit must be between 1"));
}

#[test]
fn group_run_rejects_keys_on_reads_and_duplicate_prepare_keys() {
    let read_key = parse_error(&[
        "--idempotency-key",
        "unused",
        "group",
        "run",
        "show",
        "group-run-1",
    ]);
    let duplicate = parse_error(&[
        "--idempotency-key",
        "first",
        "group",
        "run",
        "prepare",
        "group-1",
        "--idempotency-key",
        "second",
    ]);

    assert!(read_key.contains("only valid for mutating commands"));
    assert!(duplicate.contains("specified more than once"));
}

#[test]
fn group_run_prepare_rejects_duplicate_global_keys() {
    let duplicate = parse_error(&[
        "--idempotency-key",
        "first",
        "--idempotency-key",
        "second",
        "group",
        "run",
        "prepare",
        "group-1",
    ]);

    assert!(duplicate.contains("specified more than once"));
}

#[test]
fn group_run_management_rejects_project_and_group_selectors() {
    for tokens in [
        vec!["-C", "/srv/api", "group", "run", "list"],
        vec!["--group", "group-1", "group", "run", "show", "run-1"],
    ] {
        assert!(parse_error(&tokens).contains("selectors are not valid"));
    }
}
