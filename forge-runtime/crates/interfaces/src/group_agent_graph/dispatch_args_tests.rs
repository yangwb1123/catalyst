//! Parser and help tests for passive Node Dispatch Request commands.

use super::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, GroupGraphRunDispatchCommand,
    parse_tokens, usage,
};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn dispatch_prepare_binds_graph_run_and_idempotency_key() {
    let prepared = parse(&[
        "group",
        "graph",
        "run",
        "dispatch",
        "prepare",
        "graph-run-1",
        "--idempotency-key",
        "dispatch-request-key",
    ]);
    assert_eq!(
        prepared.idempotency_key.as_deref(),
        Some("dispatch-request-key")
    );
    assert_eq!(
        prepared.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::Prepare {
                graph_run_id: "graph-run-1".into(),
            })
        )))
    );
}

#[test]
fn dispatch_show_is_private_by_default_and_can_explicitly_include_request() {
    assert_eq!(
        parse(&[
            "group",
            "graph",
            "run",
            "dispatch",
            "show",
            "node-dispatch-request-1",
        ])
        .command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::Show {
                dispatch_request_id: "node-dispatch-request-1".into(),
                include_request: false,
            })
        )))
    );
    assert_eq!(
        parse(&[
            "group",
            "graph",
            "run",
            "dispatch",
            "show",
            "node-dispatch-request-1",
            "--include-request",
        ])
        .command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::Show {
                dispatch_request_id: "node-dispatch-request-1".into(),
                include_request: true,
            })
        )))
    );
}

#[test]
fn dispatch_list_accepts_an_optional_run_and_bound() {
    assert_eq!(
        parse(&[
            "group",
            "graph",
            "run",
            "dispatch",
            "list",
            "graph-run-1",
            "--limit",
            "8",
        ])
        .command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Dispatch(GroupGraphRunDispatchCommand::List {
                graph_run_id: Some("graph-run-1".into()),
                limit: 8,
            })
        )))
    );
}

#[test]
fn dispatch_rejects_effectful_and_malformed_options() {
    for tokens in [
        vec!["group", "graph", "run", "dispatch", "prepare"],
        vec![
            "group",
            "graph",
            "run",
            "dispatch",
            "prepare",
            "run-1",
            "--confirm-off-machine",
        ],
        vec![
            "group",
            "graph",
            "run",
            "dispatch",
            "prepare",
            "run-1",
            "--include-request",
        ],
        vec!["group", "graph", "run", "dispatch", "send", "run-1"],
        vec!["group", "graph", "run", "dispatch", "claim", "run-1"],
        vec![
            "group",
            "graph",
            "run",
            "dispatch",
            "show",
            "request-1",
            "--include-request",
            "--include-request",
        ],
        vec![
            "group", "graph", "run", "dispatch", "list", "--limit", "101",
        ],
        vec![
            "group",
            "graph",
            "run",
            "dispatch",
            "list",
            "--include-request",
        ],
    ] {
        assert!(parse_error(&tokens).contains("usage:"));
    }
}

#[test]
fn dispatch_argument_errors_do_not_echo_secrets() {
    // secret-scan:ignore -- inert rejected-option fixture, never a credential.
    let secret = "do-not-echo-this-api-key";
    let secret_option = format!("--api-key={secret}");
    let error = parse_error(&[
        "group",
        "graph",
        "run",
        "dispatch",
        "prepare",
        "run-1",
        &secret_option,
    ]);
    assert!(!error.contains(secret));
}

#[test]
fn dispatch_read_only_commands_reject_keys_and_help_has_no_effectful_surface() {
    for operation in [
        vec![
            "--idempotency-key",
            "wrong",
            "group",
            "graph",
            "run",
            "dispatch",
            "show",
            "request-1",
        ],
        vec![
            "--idempotency-key",
            "wrong",
            "group",
            "graph",
            "run",
            "dispatch",
            "list",
        ],
    ] {
        assert!(parse_error(&operation).contains("only valid for mutating commands"));
    }
    assert!(!usage().contains("group graph run dispatch claim"));
    assert!(!usage().contains("group graph run dispatch send"));
}
