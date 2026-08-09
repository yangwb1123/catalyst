use std::path::Path;

use super::{Command, GovernanceCommand, GovernanceJournalCommand, parse_tokens};
use crate::runtime_domain::GovernanceRecordKind;

fn parse(tokens: &[&str]) -> super::Args {
    parse_tokens(tokens.iter().map(ToString::to_string)).expect("arguments parse")
}

#[test]
fn append_requires_explicit_key_and_named_file() {
    let missing_key = parse_tokens(
        ["governance", "journal", "append", "--file", "records.json"].map(str::to_owned),
    )
    .expect_err("append key is mandatory");
    assert!(missing_key.contains("requires an explicit --idempotency-key"));

    let parsed = parse(&[
        "--idempotency-key",
        "journal-key",
        "governance",
        "journal",
        "append",
        "--file",
        "records.json",
    ]);
    assert_eq!(parsed.idempotency_key.as_deref(), Some("journal-key"));
    assert_eq!(
        parsed.command,
        Command::Governance(GovernanceCommand::Journal(
            GovernanceJournalCommand::Append {
                file: Path::new("records.json").to_path_buf(),
            }
        ))
    );
}

#[test]
fn show_is_metadata_only_unless_reveal_is_explicit() {
    let hidden = parse(&["governance", "journal", "show", "evr-1"]);
    let revealed = parse(&["governance", "journal", "show", "evr-1", "--include-record"]);
    assert_eq!(show_reveal(&hidden.command), Some(false));
    assert_eq!(show_reveal(&revealed.command), Some(true));
}

#[test]
fn list_accepts_bounded_filters_in_any_supported_order() {
    let parsed = parse(&[
        "governance",
        "journal",
        "list",
        "--aggregate-id",
        "claim-1",
        "--include-record",
        "--limit",
        "7",
        "--kind",
        "KnowledgeClaim",
    ]);
    assert_eq!(
        parsed.command,
        Command::Governance(GovernanceCommand::Journal(GovernanceJournalCommand::List {
            record_kind: Some(GovernanceRecordKind::KnowledgeClaim),
            aggregate_id: Some("claim-1".into()),
            limit: 7,
            include_record: true,
        }))
    );
}

#[test]
fn invalid_kind_limit_and_read_key_fail_closed() {
    for tokens in [
        vec!["governance", "journal", "head", "Claim", "claim-1"],
        vec!["governance", "journal", "list", "--limit", "0"],
        vec!["governance", "journal", "list", "--limit", "101"],
        vec![
            "governance",
            "journal",
            "list",
            "--limit",
            "1",
            "--limit",
            "2",
        ],
        vec![
            "--idempotency-key",
            "unused",
            "governance",
            "journal",
            "show",
            "evr-1",
        ],
    ] {
        assert!(parse_tokens(tokens.into_iter().map(str::to_owned)).is_err());
    }
}

#[test]
fn duplicate_list_limit_is_rejected_explicitly() {
    let error = parse_tokens(
        [
            "governance",
            "journal",
            "list",
            "--limit",
            "1",
            "--limit",
            "2",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .expect_err("duplicate limit must fail closed");
    assert!(error.contains("duplicate --limit"), "{error}");
}

#[test]
fn governance_reads_reject_project_and_group_selectors() {
    for prefix in [["-C", "/tmp/project"], ["--group", "group-1"]] {
        let tokens = prefix
            .into_iter()
            .chain(["governance", "journal", "list"])
            .map(str::to_owned);
        let error = parse_tokens(tokens).expect_err("selectors must not be ignored");
        assert!(error.contains("selectors are not valid"));
    }
}

fn show_reveal(command: &Command) -> Option<bool> {
    let Command::Governance(GovernanceCommand::Journal(GovernanceJournalCommand::Show {
        include_record,
        ..
    })) = command
    else {
        return None;
    };
    Some(*include_record)
}
