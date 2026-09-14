use super::*;
use crate::args::parse_tokens;

#[test]
fn remote_login_and_tui_commands_reject_unsupported_options() {
    assert_eq!(
        parse_tokens(["remote", "login"].map(str::to_owned))
            .unwrap()
            .command,
        Command::Remote(RemoteCommand::Login)
    );
    assert!(parse_tokens(["remote", "login", "extra"].map(str::to_owned)).is_err());
    let args = parse_tokens(["remote", "tui"].map(str::to_owned)).unwrap();
    assert_eq!(args.command, Command::Remote(RemoteCommand::Tui));
    assert!(parse_tokens(["--json", "remote", "tui"].map(str::to_owned)).is_err());
    assert!(
        parse_tokens(["--state-dir", "/tmp/local", "remote", "tui"].map(str::to_owned)).is_err()
    );

    let local_scope =
        parse_tokens(["-C", "/workspace", "remote", "sessions", "list"].map(str::to_owned));
    assert!(local_scope.unwrap_err().contains("project/group selectors"));
}

#[test]
fn remote_session_list_parses_exact_scope_and_cursor() {
    let first = parse_tokens(["remote", "sessions", "list"].map(str::to_owned)).unwrap();
    assert_eq!(
        first.command,
        Command::Remote(RemoteCommand::SessionsList {
            after_id: None,
            scope: None,
            all_pages: false,
        })
    );
    let group = parse_tokens(
        [
            "remote",
            "sessions",
            "list",
            "--scope",
            "group:grp_1",
            "--after",
            "c-2",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        group.command,
        Command::Remote(RemoteCommand::SessionsList {
            after_id: Some("c-2".into()),
            scope: Some(RemoteConversationScope::Group("grp_1".into())),
            all_pages: false,
        })
    );
    assert!(
        parse_tokens(["remote", "sessions", "create", "--scope", "project:"].map(str::to_owned))
            .is_err()
    );
    let duplicate = parse_tokens(
        [
            "remote",
            "sessions",
            "list",
            "--scope",
            "global",
            "--scope",
            "group:grp_1",
        ]
        .map(str::to_owned),
    );
    assert!(duplicate.is_err());
}

#[test]
fn remote_session_list_all_pages_flag_is_opt_in_and_unique() {
    let all_pages = parse_tokens(
        [
            "remote",
            "sessions",
            "list",
            "--all",
            "--scope",
            "project:prj_1",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        all_pages.command,
        Command::Remote(RemoteCommand::SessionsList {
            after_id: None,
            scope: Some(RemoteConversationScope::Project("prj_1".into())),
            all_pages: true,
        })
    );
    assert!(
        parse_tokens(["remote", "sessions", "list", "--all", "--all"].map(str::to_owned)).is_err()
    );
}

#[test]
fn remote_session_create_parses_project_scope_and_requires_idempotency() {
    let project = parse_tokens(
        [
            "--idempotency-key",
            "project-create",
            "remote",
            "sessions",
            "create",
            "--scope",
            "project:prj_1",
            "--title",
            "Review",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        project.command,
        Command::Remote(RemoteCommand::SessionsCreate {
            title: "Review".into(),
            scope: RemoteConversationScope::Project("prj_1".into()),
        })
    );
    assert!(
        parse_tokens(["remote", "sessions", "create", "--scope", "project:"].map(str::to_owned))
            .is_err()
    );
    assert!(
        parse_tokens(["remote", "sessions", "create", "--title", "New"].map(str::to_owned))
            .is_err()
    );
}

#[test]
fn remote_session_import_preview_accepts_local_state() {
    let preview = parse_tokens(
        [
            "--state-dir",
            "/tmp/forge",
            "remote",
            "sessions",
            "import",
            "local-1",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        preview.command,
        Command::Remote(RemoteCommand::SessionsImport {
            conversation_id: "local-1".into(),
            confirm: None,
        })
    );
}

#[test]
fn remote_session_import_confirmation_requires_lowercase_sha256() {
    let id = "local-1";
    let digest = "a".repeat(64);
    let confirmed = parse_tokens(
        [
            "--state-dir",
            "/tmp/forge",
            "remote",
            "sessions",
            "import",
            id,
            "--confirm",
            &digest,
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        confirmed.command,
        Command::Remote(RemoteCommand::SessionsImport {
            conversation_id: id.into(),
            confirm: Some(digest),
        })
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "sessions",
                "import",
                id,
                "--confirm",
                &"A".repeat(64)
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
}

#[test]
fn other_remote_session_commands_reject_a_local_state_directory() {
    assert!(
        parse_tokens(
            ["--state-dir", "/tmp/forge", "remote", "sessions", "list"].map(str::to_owned)
        )
        .is_err()
    );
}

#[test]
fn remote_change_command_parses_bounded_cursor() {
    assert_eq!(
        parse_tokens(["remote", "changes", "list"].map(str::to_owned))
            .unwrap()
            .command,
        Command::Remote(RemoteCommand::ChangesList { after_cursor: None })
    );
    assert_eq!(
        parse_tokens(["remote", "changes", "list", "--after-cursor", "19"].map(str::to_owned))
            .unwrap()
            .command,
        Command::Remote(RemoteCommand::ChangesList {
            after_cursor: Some(19),
        })
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "changes",
                "list",
                "--after-cursor",
                "9223372036854775808"
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
}

#[test]
fn remote_run_list_parses_default_and_cursor_forms() {
    assert_eq!(
        parse_remote_command(&["remote", "runs", "list", "conversation-1"]).unwrap(),
        RemoteCommand::RunsList {
            conversation_id: "conversation-1".into(),
            limit: 25,
            before_created_at_ms: None,
            before_run_id: None,
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runs",
            "list",
            "conversation-1",
            "--limit",
            "5",
            "--before-created-at-ms",
            "22",
            "--before-run-id",
            "run-3",
        ])
        .unwrap(),
        RemoteCommand::RunsList {
            conversation_id: "conversation-1".into(),
            limit: 5,
            before_created_at_ms: Some(22),
            before_run_id: Some("run-3".into()),
        }
    );
}

#[test]
fn remote_run_list_rejects_partial_cursors_and_invalid_limits() {
    assert!(
        parse_remote_command(&[
            "remote",
            "runs",
            "list",
            "conversation-1",
            "--before-created-at-ms",
            "22",
        ])
        .is_err()
    );
    for invalid in ["0", "26", "not-a-number"] {
        assert!(
            parse_remote_command(&[
                "remote",
                "runs",
                "list",
                "conversation-1",
                "--limit",
                invalid
            ])
            .is_err()
        );
    }
    assert!(
        parse_remote_command(&[
            "remote",
            "runs",
            "list",
            "conversation-1",
            "--limit",
            "5",
            "--limit",
            "6",
        ])
        .is_err()
    );
}

#[test]
fn remote_run_timeline_parses_default_and_custom_cursors() {
    assert_eq!(
        parse_remote_command(&["remote", "runs", "timeline", "conversation-1", "run-1"]).unwrap(),
        RemoteCommand::RunTimeline {
            conversation_id: "conversation-1".into(),
            run_id: "run-1".into(),
            after_sequence: 0,
            limit: 128,
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runs",
            "timeline",
            "conversation-1",
            "run-1",
            "--after-sequence",
            "17",
            "--limit",
            "20",
        ])
        .unwrap(),
        RemoteCommand::RunTimeline {
            conversation_id: "conversation-1".into(),
            run_id: "run-1".into(),
            after_sequence: 17,
            limit: 20,
        }
    );
}

#[test]
fn remote_run_timeline_rejects_invalid_cursors_and_limits() {
    for invalid in ["-1", "9223372036854775808", "not-a-number"] {
        assert!(
            parse_remote_command(&[
                "remote",
                "runs",
                "timeline",
                "conversation-1",
                "run-1",
                "--after-sequence",
                invalid,
            ])
            .is_err()
        );
    }
    assert!(
        parse_remote_command(&[
            "remote",
            "runs",
            "timeline",
            "conversation-1",
            "run-1",
            "--limit",
            "129",
        ])
        .is_err()
    );
}

fn parse_remote_command(tokens: &[&str]) -> Result<RemoteCommand, String> {
    parse_tokens(tokens.iter().map(|token| (*token).to_owned())).map(|args| match args.command {
        Command::Remote(command) => command,
        _ => unreachable!("remote argument helper parsed a non-remote command"),
    })
}

#[test]
fn remote_mutations_require_explicit_idempotency_keys() {
    for tokens in [
        vec!["remote", "sessions", "create"],
        vec![
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "hello",
        ],
    ] {
        let error = parse_tokens(tokens.into_iter().map(str::to_owned)).unwrap_err();
        assert!(error.contains("explicit --idempotency-key"));
    }
}

#[test]
fn remote_prompt_commands_keep_prompt_text_and_reject_local_scope_selection() {
    let args = parse_tokens(
        [
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "hello",
            "world",
        ]
        .map(str::to_owned),
    );
    assert!(args.is_err());

    let args = parse_tokens(
        [
            "--idempotency-key",
            "prompt-1",
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "hello",
            "world",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::PromptsAdd {
            conversation_id: "conversation-1".into(),
            expected_version: 1,
            content: "hello world".into(),
        })
    );

    let scoped =
        parse_tokens(["-C", "/workspace", "remote", "sessions", "list"].map(str::to_owned))
            .unwrap_err();
    assert!(scoped.contains("project/group selectors"));
}

#[test]
fn remote_prompt_history_cursor_accepts_absent_or_complete_pair() {
    assert_eq!(
        parse_remote_command(&["remote", "prompts", "list", "c-1"]).unwrap(),
        RemoteCommand::PromptsList {
            conversation_id: "c-1".into(),
            before_created_at_ms: None,
            before_prompt_id: None,
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "list",
            "c-1",
            "--before-prompt-id",
            "p-7",
            "--before-created-at-ms",
            "170",
        ])
        .unwrap(),
        RemoteCommand::PromptsList {
            conversation_id: "c-1".into(),
            before_created_at_ms: Some(170),
            before_prompt_id: Some("p-7".into()),
        }
    );
}

#[test]
fn remote_prompt_history_cursor_rejects_incomplete_or_invalid_pairs() {
    for arguments in [
        vec![
            "remote",
            "prompts",
            "list",
            "c-1",
            "--before-created-at-ms",
            "170",
        ],
        vec![
            "remote",
            "prompts",
            "list",
            "c-1",
            "--before-prompt-id",
            "p-7",
        ],
        vec![
            "remote",
            "prompts",
            "list",
            "c-1",
            "--before-created-at-ms",
            "-1",
            "--before-prompt-id",
            "p-7",
        ],
        vec![
            "remote",
            "prompts",
            "list",
            "c-1",
            "--before-created-at-ms",
            "170",
            "--before-prompt-id",
            "bad/id",
        ],
    ] {
        assert!(parse_remote_command(&arguments).is_err(), "{arguments:?}");
    }
}
