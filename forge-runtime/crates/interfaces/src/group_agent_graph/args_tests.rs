//! Parser rejection and visibility tests for Group Agent Graph commands.

use super::{
    Command, GroupCommand, GroupGraphCommand, GroupGraphRunCommand, GroupGraphRunContractCommand,
    GroupGraphRunControlCommand, parse_tokens,
};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn prepare_binds_source_spec_and_key() {
    let args = parse(&[
        "group",
        "graph",
        "prepare",
        "group-run-1",
        "--spec",
        "-",
        "--idempotency-key",
        "graph-key",
    ]);
    assert_eq!(args.idempotency_key.as_deref(), Some("graph-key"));
    assert_eq!(
        args.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Prepare {
            group_run_id: "group-run-1".into(),
            spec_source: "-".into(),
        }))
    );
}

#[test]
fn show_and_list_are_private_and_bounded_by_default() {
    assert_eq!(
        parse(&["group", "graph", "show", "graph-1"]).command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Show {
            graph_id: "graph-1".into(),
            include_spec: false,
        }))
    );
    assert_eq!(
        parse(&["group", "graph", "list", "group-run-1", "--limit", "7"]).command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::List {
            group_run_id: Some("group-run-1".into()),
            limit: 7,
        }))
    );
    assert_eq!(
        parse(&["group", "graph", "show", "graph-1", "--include-spec"]).command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Show {
            graph_id: "graph-1".into(),
            include_spec: true,
        }))
    );
}

#[test]
fn invalid_and_duplicate_graph_options_fail_closed() {
    for tokens in [
        vec!["group", "graph", "prepare", "group-run-1"],
        vec!["group", "graph", "prepare", "--spec", "-"],
        vec![
            "group",
            "graph",
            "prepare",
            "group-run-1",
            "--spec",
            "-",
            "--spec",
            "other.json",
        ],
        vec!["group", "graph", "show", "graph-1", "--include-tasks"],
        vec![
            "group",
            "graph",
            "show",
            "graph-1",
            "--include-spec",
            "--include-spec",
        ],
        vec!["group", "graph", "list", "--limit", "0"],
        vec!["group", "graph", "list", "--include-spec"],
    ] {
        assert!(parse_error(&tokens).contains("usage:"));
    }
}

#[test]
fn selectors_and_non_prepare_keys_are_rejected() {
    assert!(
        parse_error(&["-C", "/srv/frontend", "group", "graph", "list"])
            .contains("selectors are not valid")
    );
    assert!(
        parse_error(&["--group", "group-1", "group", "graph", "show", "graph-1",])
            .contains("selectors are not valid")
    );
    assert!(
        parse_error(&[
            "--idempotency-key",
            "wrong-scope",
            "group",
            "graph",
            "show",
            "graph-1",
        ])
        .contains("only valid for mutating commands")
    );
    assert!(
        parse_error(&["--idempotency-key", "wrong-scope", "group", "graph", "list",])
            .contains("only valid for mutating commands")
    );
}

#[test]
fn untrusted_graph_arguments_are_terminal_escaped() {
    let error = parse_error(&[
        "group",
        "graph",
        "list",
        "--limit",
        "12\u{1b}[31mINJECT\nNEXT",
    ]);
    assert!(!error.contains('\u{1b}'));
    assert!(!error.contains("INJECT\nNEXT"));
    assert!(error.contains(r"\x1b[31mINJECT\nNEXT"));
}

#[test]
fn passive_graph_run_commands_parse_with_explicit_plan_and_key() {
    let prepared = parse(&[
        "group",
        "graph",
        "run",
        "prepare",
        "graph-1",
        "--plan",
        "-",
        "--idempotency-key",
        "run-key",
    ]);
    assert_eq!(prepared.idempotency_key.as_deref(), Some("run-key"));
    assert_eq!(
        prepared.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Prepare {
                graph_id: "graph-1".into(),
                plan_source: "-".into(),
            }
        )))
    );
    assert_eq!(
        parse(&["group", "graph", "run", "show", "graph-run-1"]).command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Show {
                graph_run_id: "graph-run-1".into(),
                include_plan: false,
            }
        )))
    );
    assert_eq!(
        parse(&["group", "graph", "run", "list", "graph-1", "--limit", "3",]).command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::List {
                graph_id: Some("graph-1".into()),
                limit: 3,
            }
        )))
    );
}

#[test]
fn graph_run_options_fail_closed_and_do_not_accept_selectors() {
    for tokens in [
        vec!["group", "graph", "run", "prepare", "graph-1"],
        vec!["group", "graph", "run", "prepare", "--plan", "-"],
        vec![
            "group",
            "graph",
            "run",
            "prepare",
            "graph-1",
            "--plan",
            "-",
            "--plan",
            "other.json",
        ],
        vec!["group", "graph", "run", "show", "run-1", "--include-spec"],
        vec!["group", "graph", "run", "list", "--limit", "101"],
    ] {
        assert!(parse_error(&tokens).contains("usage:"));
    }
    assert!(
        parse_error(&["-C", ".", "group", "graph", "run", "list"])
            .contains("selectors are not valid")
    );
}

#[test]
fn control_export_and_contract_commands_parse_explicit_ids() {
    assert_eq!(
        parse(&["group", "graph", "run", "control", "export", "graph-run-1",]).command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Control(GroupGraphRunControlCommand::Export {
                graph_run_id: "graph-run-1".into(),
            })
        )))
    );
    let admitted = parse(&[
        "group",
        "graph",
        "run",
        "contract",
        "admit",
        "graph-run-1",
        "--contract",
        "-",
        "--idempotency-key",
        "contract-key",
    ]);
    assert_eq!(admitted.idempotency_key.as_deref(), Some("contract-key"));
    assert_eq!(
        admitted.command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Contract(GroupGraphRunContractCommand::Admit {
                graph_run_id: "graph-run-1".into(),
                contract_source: "-".into(),
            })
        )))
    );
}

#[test]
fn contract_show_and_list_are_private_and_bounded_by_default() {
    assert_eq!(
        parse(&[
            "group",
            "graph",
            "run",
            "contract",
            "show",
            "node-contract-1",
            "--include-contract",
        ])
        .command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Contract(GroupGraphRunContractCommand::Show {
                contract_id: "node-contract-1".into(),
                include_contract: true,
            })
        )))
    );
    assert_eq!(
        parse(&[
            "group",
            "graph",
            "run",
            "contract",
            "list",
            "graph-run-1",
            "--limit",
            "9",
        ])
        .command,
        Command::Group(GroupCommand::Graph(GroupGraphCommand::Run(
            GroupGraphRunCommand::Contract(GroupGraphRunContractCommand::List {
                graph_run_id: Some("graph-run-1".into()),
                limit: 9,
            })
        )))
    );
}

#[test]
fn control_and_contract_options_fail_closed() {
    for tokens in [
        vec!["group", "graph", "run", "control", "export"],
        vec!["group", "graph", "run", "control", "advance", "run-1"],
        vec!["group", "graph", "run", "contract", "admit", "run-1"],
        vec![
            "group",
            "graph",
            "run",
            "contract",
            "admit",
            "run-1",
            "--contract",
            "-",
            "--contract",
            "other.json",
        ],
        vec![
            "group",
            "graph",
            "run",
            "contract",
            "show",
            "contract-1",
            "--include-plan",
        ],
        vec![
            "group", "graph", "run", "contract", "list", "--limit", "101",
        ],
    ] {
        assert!(parse_error(&tokens).contains("usage:"));
    }
}

#[test]
fn read_only_control_and_contract_commands_reject_idempotency_keys() {
    for operation in [
        vec![
            "--idempotency-key",
            "wrong",
            "group",
            "graph",
            "run",
            "control",
            "export",
            "run-1",
        ],
        vec![
            "--idempotency-key",
            "wrong",
            "group",
            "graph",
            "run",
            "contract",
            "show",
            "contract-1",
        ],
    ] {
        assert!(parse_error(&operation).contains("only valid for mutating commands"));
    }
}
