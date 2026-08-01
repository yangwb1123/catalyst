use super::{Command, GroupCommand, GroupPanelCommand, parse_tokens};

fn parse(arguments: &[&str]) -> Result<Command, String> {
    parse_tokens(arguments.iter().map(|value| (*value).to_owned())).map(|args| args.command)
}

#[test]
fn parses_ordered_panel_prepare_inputs() {
    let parsed = parse(&[
        "group",
        "panel",
        "prepare",
        "group-run-1",
        "--analysis",
        "analysis-a",
        "--analysis",
        "analysis-b",
        "--idempotency-key",
        "panel-key",
    ])
    .expect("valid panel prepare");
    assert_eq!(
        parsed,
        Command::Group(GroupCommand::Panel(GroupPanelCommand::Prepare {
            group_run_id: "group-run-1".into(),
            analysis_ids: vec!["analysis-a".into(), "analysis-b".into()],
        }))
    );
}

#[test]
fn rejects_too_few_or_duplicate_panel_inputs() {
    let too_few = parse(&[
        "group",
        "panel",
        "prepare",
        "group-run-1",
        "--analysis",
        "analysis-a",
    ])
    .expect_err("one analysis is insufficient");
    assert!(too_few.contains("between 2 and 8"));

    let duplicate = parse(&[
        "group",
        "panel",
        "prepare",
        "group-run-1",
        "--analysis",
        "analysis-a",
        "--analysis",
        "analysis-a",
    ])
    .expect_err("duplicate analysis IDs fail");
    assert!(duplicate.contains("duplicate analysis IDs"));
}

#[test]
fn parses_panel_show_and_list() {
    assert_eq!(
        parse(&["group", "panel", "show", "panel-1", "--include-results"]).expect("show"),
        Command::Group(GroupCommand::Panel(GroupPanelCommand::Show {
            panel_id: "panel-1".into(),
            include_results: true,
        }))
    );
    assert_eq!(
        parse(&["group", "panel", "list", "group-run-1", "--limit", "7"]).expect("list"),
        Command::Group(GroupCommand::Panel(GroupPanelCommand::List {
            group_run_id: Some("group-run-1".into()),
            limit: 7,
        }))
    );
}
