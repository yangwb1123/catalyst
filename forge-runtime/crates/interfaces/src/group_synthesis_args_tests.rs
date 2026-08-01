use super::{Command, GroupCommand, GroupSynthesisCommand, parse_tokens};

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

fn parse_error(tokens: &[&str]) -> String {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect_err("arguments must fail")
}

#[test]
fn prepare_binds_panel_model_limit_and_key() {
    let defaults = parse(&["group", "synthesis", "prepare", "panel-1"]);
    assert_eq!(
        defaults.command,
        Command::Group(GroupCommand::Synthesis(GroupSynthesisCommand::Prepare {
            panel_id: "panel-1".into(),
            model: None,
            max_output_tokens: 4_096,
        }))
    );

    let configured = parse(&[
        "group",
        "synthesis",
        "prepare",
        "panel-1",
        "--model",
        "requested-model",
        "--max-output-tokens",
        "8192",
        "--idempotency-key",
        "synthesis-key",
    ]);
    assert_eq!(configured.idempotency_key.as_deref(), Some("synthesis-key"));
    assert_eq!(
        configured.command,
        Command::Group(GroupCommand::Synthesis(GroupSynthesisCommand::Prepare {
            panel_id: "panel-1".into(),
            model: Some("requested-model".into()),
            max_output_tokens: 8_192,
        }))
    );
}

#[test]
fn send_keeps_consent_and_result_visibility_separate() {
    let args = parse(&[
        "group",
        "synthesis",
        "send",
        "synthesis-1",
        "--confirm-off-machine",
        "--include-result",
    ]);
    assert_eq!(
        args.command,
        Command::Group(GroupCommand::Synthesis(GroupSynthesisCommand::Send {
            synthesis_id: "synthesis-1".into(),
            confirm_off_machine: true,
            include_result: true,
        }))
    );
}

#[test]
fn show_and_list_are_private_by_default() {
    assert_eq!(
        parse(&["group", "synthesis", "show", "synthesis-1"]).command,
        Command::Group(GroupCommand::Synthesis(GroupSynthesisCommand::Show {
            synthesis_id: "synthesis-1".into(),
            include_result: false,
        }))
    );
    assert_eq!(
        parse(&["group", "synthesis", "list", "panel-1", "--limit", "7",]).command,
        Command::Group(GroupCommand::Synthesis(GroupSynthesisCommand::List {
            panel_id: Some("panel-1".into()),
            limit: 7,
        }))
    );
}

#[test]
fn forbidden_capabilities_and_debug_surfaces_fail_closed() {
    for option in [
        "--include-prompt",
        "--include-request",
        "--include-panel-results",
        "--endpoint",
        "--tools",
        "--workspace",
        "--output",
        "--writeback",
        "--allow-read",
    ] {
        let error = parse_error(&["group", "synthesis", "show", "synthesis-1", option]);
        assert!(error.contains("usage:"));
    }
}

#[test]
fn invalid_or_duplicate_options_fail_closed() {
    for tokens in [
        vec!["group", "synthesis", "prepare"],
        vec![
            "group",
            "synthesis",
            "prepare",
            "panel-1",
            "--max-output-tokens",
            "0",
        ],
        vec![
            "group",
            "synthesis",
            "send",
            "synthesis-1",
            "--confirm-off-machine",
            "--confirm-off-machine",
        ],
        vec![
            "group",
            "synthesis",
            "show",
            "synthesis-1",
            "--confirm-off-machine",
        ],
        vec!["group", "synthesis", "list", "--limit", "0"],
    ] {
        assert!(parse_error(&tokens).contains("usage:"));
    }
}

#[test]
fn selectors_and_send_keys_are_rejected() {
    assert!(
        parse_error(&["-C", "/srv/api", "group", "synthesis", "list"])
            .contains("selectors are not valid")
    );
    assert!(
        parse_error(&[
            "--idempotency-key",
            "wrong-scope",
            "group",
            "synthesis",
            "send",
            "synthesis-1",
            "--confirm-off-machine",
        ])
        .contains("owns the single dispatch claim")
    );
}
