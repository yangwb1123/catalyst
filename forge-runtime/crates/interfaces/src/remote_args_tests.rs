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
fn remote_credentials_status_is_a_local_read_only_command() {
    let args = parse_tokens(["remote", "credentials", "status"].map(str::to_owned)).unwrap();
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::CredentialStorageStatus)
    );
    assert!(parse_tokens(["remote", "credentials"].map(str::to_owned)).is_err());
    assert!(parse_tokens(["remote", "credentials", "status", "extra"].map(str::to_owned)).is_err());
}

#[test]
fn remote_inventory_show_is_a_bounded_read_without_options() {
    assert_eq!(
        parse_remote_command(&["remote", "inventory", "show"]).unwrap(),
        RemoteCommand::InventoryShow
    );
    assert!(parse_remote_command(&["remote", "inventory"]).is_err());
    assert!(parse_remote_command(&["remote", "inventory", "show", "extra"]).is_err());
}

#[test]
fn remote_runner_dispatch_plan_preview_requires_one_bounded_input() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runner-dispatch-plan-preview",
            "--input",
            "request.json",
        ])
        .unwrap(),
        RemoteCommand::RunnerDispatchPlanPreview {
            input: "request.json".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "runner-dispatch-plan-preview"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "runner-dispatch-plan-preview",
            "--input",
            "a.json",
            "--input",
            "b.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&["remote", "runner-dispatch-plan-preview", "--input", " "]).is_err()
    );
}

#[test]
fn remote_lifecycle_registry_is_an_explicit_read_only_candidate() {
    assert_eq!(
        parse_remote_command(&["remote", "lifecycle-registry", "show"]).unwrap(),
        RemoteCommand::LifecycleRegistryShow
    );
    assert_eq!(
        parse_remote_command(&["remote", "lifecycle-registry", "read"]).unwrap(),
        RemoteCommand::LifecycleRegistryShow
    );
    assert!(parse_remote_command(&["remote", "lifecycle-registry"]).is_err());
    assert!(parse_remote_command(&["remote", "lifecycle-registry", "put"]).is_err());
    assert!(parse_remote_command(&["remote", "lifecycle-registry", "show", "extra"]).is_err());
}

#[test]
fn remote_inventory_show_v2_is_an_explicit_candidate_read() {
    assert_eq!(
        parse_remote_command(&["remote", "inventory", "show-v2"]).unwrap(),
        RemoteCommand::InventoryShowV2
    );
    assert!(parse_remote_command(&["remote", "inventory", "show-v2", "extra"]).is_err());
}

#[test]
fn remote_client_instance_views_parse_as_exact_read_only_commands() {
    assert_eq!(
        parse_remote_command(&["remote", "client-instances", "session-view"]).unwrap(),
        RemoteCommand::ClientInstanceSessionView
    );
    assert_eq!(
        parse_remote_command(&["remote", "client-instances", "resource-view"]).unwrap(),
        RemoteCommand::ClientInstanceResourceView
    );
    assert!(parse_remote_command(&["remote", "client-instances"]).is_err());
    assert!(parse_remote_command(&["remote", "client-instances", "unknown"]).is_err());
    assert!(
        parse_remote_command(&["remote", "client-instances", "session-view", "extra",]).is_err()
    );
}

#[test]
fn remote_credential_candidate_preview_requires_one_bounded_input() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "credential-candidate",
            "preview",
            "--input",
            "candidate.json",
        ])
        .unwrap(),
        RemoteCommand::CredentialCandidatePreview {
            input: "candidate.json".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "credential-candidate"]).is_err());
    assert!(parse_remote_command(&["remote", "credential-candidate", "preview",]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "credential-candidate",
            "preview",
            "--input",
            "candidate.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "credential-candidate",
            "preview",
            "--input",
            "candidate.json",
            "--unknown",
        ])
        .is_err()
    );
}

#[test]
fn remote_execution_consent_preview_parses_as_an_exact_read_only_command() {
    assert_eq!(
        parse_remote_command(&["remote", "execution-consent", "preview", "conversation-17",])
            .unwrap(),
        RemoteCommand::ExecutionConsentPreview {
            conversation_id: "conversation-17".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "execution-consent"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "execution-consent",
            "preview",
            "conversation-17",
            "extra",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&["remote", "execution-consent", "grant", "conversation-17",])
            .is_err()
    );
}

#[test]
fn remote_execution_reconciliation_preview_requires_one_bounded_input() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "execution-reconciliation",
            "preview",
            "--input",
            "restart.json",
        ])
        .unwrap(),
        RemoteCommand::ExecutionReconciliationPreview {
            input: "restart.json".into(),
        }
    );
    assert!(parse_remote_command(&["remote", "execution-reconciliation"]).is_err());
    assert!(parse_remote_command(&["remote", "execution-reconciliation", "preview",]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "execution-reconciliation",
            "preview",
            "--input",
            "restart.json",
            "--input",
            "other.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "execution-reconciliation",
            "preview",
            "--input",
            "",
        ])
        .is_err()
    );
}

#[test]
fn remote_session_list_parses_exact_scope_and_cursor() {
    let first = parse_tokens(["remote", "sessions", "list"].map(str::to_owned)).unwrap();
    assert_eq!(
        first.command,
        Command::Remote(RemoteCommand::SessionsList {
            after_id: None,
            scope: None,
            instance_id: None,
            instance_view: None,
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
            instance_id: None,
            instance_view: None,
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
fn remote_session_detail_parses_id() {
    let detail =
        parse_tokens(["remote", "sessions", "show", "conversation-17"].map(str::to_owned)).unwrap();
    assert_eq!(
        detail.command,
        Command::Remote(RemoteCommand::SessionsShow {
            conversation_id: "conversation-17".into(),
            instance_id: None,
            instance_view: None,
        })
    );
}

#[test]
fn remote_session_detail_parses_optional_client_instance_projection() {
    let detail = parse_tokens(
        [
            "remote",
            "sessions",
            "show",
            "conversation-17",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-view.json",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        detail.command,
        Command::Remote(RemoteCommand::SessionsShow {
            conversation_id: "conversation-17".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-view.json".into()),
        })
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "sessions",
                "show",
                "conversation-17",
                "--instance-view",
                "client-view.json",
            ]
            .map(str::to_owned),
        )
        .unwrap_err()
        .contains("--instance-view requires --instance")
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "sessions",
                "show",
                "conversation-17",
                "--instance",
                "client/web",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
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
            instance_id: None,
            instance_view: None,
            all_pages: true,
        })
    );
    assert!(
        parse_tokens(["remote", "sessions", "list", "--all", "--all"].map(str::to_owned)).is_err()
    );
}

#[test]
fn remote_session_list_parses_local_client_instance_projection() {
    let args = parse_remote_command(&[
        "remote",
        "sessions",
        "list",
        "--instance",
        "client-web-001",
        "--instance-view",
        "client-instance-view.json",
    ])
    .unwrap();
    assert_eq!(
        args,
        RemoteCommand::SessionsList {
            after_id: None,
            scope: None,
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-instance-view.json".into()),
            all_pages: false,
        }
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "sessions",
            "list",
            "--instance-view",
            "client-instance-view.json",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&["remote", "sessions", "list", "--instance", "client/web",]).is_err()
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
fn remote_change_watch_parses_bounded_polling_and_backoff_options() {
    assert_eq!(
        parse_tokens(
            [
                "remote",
                "changes",
                "watch",
                "--after-cursor",
                "19",
                "--polls",
                "4",
                "--min-delay-ms",
                "50",
                "--max-delay-ms",
                "500"
            ]
            .map(str::to_owned)
        )
        .unwrap()
        .command,
        Command::Remote(RemoteCommand::ChangesWatch {
            after_cursor: Some(19),
            polls: 4,
            min_delay_ms: 50,
            max_delay_ms: 500,
        })
    );
    assert!(
        parse_tokens(["remote", "changes", "watch", "--polls", "0"].map(str::to_owned)).is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "changes",
                "watch",
                "--min-delay-ms",
                "500",
                "--max-delay-ms",
                "50"
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
            instance_id: None,
            instance_view: None,
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
            instance_id: None,
            instance_view: None,
        }
    );
}

#[test]
fn remote_run_observed_parses_as_an_exact_owner_bound_read() {
    assert_eq!(
        parse_remote_command(&["remote", "runs", "observed", "conversation-1", "run-8"]).unwrap(),
        RemoteCommand::RunObserved {
            conversation_id: "conversation-1".into(),
            run_id: "run-8".into(),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(parse_remote_command(&["remote", "runs", "observed"]).is_err());
    assert!(
        parse_remote_command(&[
            "remote",
            "runs",
            "observed",
            "conversation-1",
            "run-8",
            "--json",
        ])
        .is_err()
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
fn remote_run_reads_parse_explicit_client_instance_projection_options() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runs",
            "list",
            "conversation-1",
            "--instance-id",
            "client-cli-001",
            "--instance-view",
            "view.json",
        ])
        .unwrap(),
        RemoteCommand::RunsList {
            conversation_id: "conversation-1".into(),
            limit: 25,
            before_created_at_ms: None,
            before_run_id: None,
            instance_id: Some("client-cli-001".into()),
            instance_view: Some("view.json".into()),
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runs",
            "observed",
            "conversation-1",
            "run-1",
            "--instance",
            "client-cli-001",
        ])
        .unwrap(),
        RemoteCommand::RunObserved {
            conversation_id: "conversation-1".into(),
            run_id: "run-1".into(),
            instance_id: Some("client-cli-001".into()),
            instance_view: None,
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runs",
            "timeline",
            "conversation-1",
            "run-1",
            "--resume",
            "--instance",
            "client-cli-001",
            "--instance-view",
            "-",
        ])
        .unwrap(),
        RemoteCommand::RunTimeline {
            conversation_id: "conversation-1".into(),
            run_id: "run-1".into(),
            after_sequence: 0,
            limit: 128,
            resume: true,
            instance_id: Some("client-cli-001".into()),
            instance_view: Some("-".into()),
        }
    );
}

#[test]
fn remote_run_reads_reject_instance_view_without_instance() {
    for command in [
        vec![
            "remote",
            "runs",
            "list",
            "conversation-1",
            "--instance-view",
            "view.json",
        ],
        vec![
            "remote",
            "runs",
            "observed",
            "conversation-1",
            "run-1",
            "--instance-view",
            "view.json",
        ],
        vec![
            "remote",
            "runs",
            "timeline",
            "conversation-1",
            "run-1",
            "--instance-view",
            "view.json",
        ],
    ] {
        let args: Vec<&str> = command;
        assert!(parse_remote_command(&args).is_err());
    }
}

#[test]
fn remote_pending_run_intent_list_parses_bounded_cursor() {
    assert_eq!(
        parse_remote_command(&["remote", "run-intents", "list", "conversation-1"]).unwrap(),
        RemoteCommand::PendingRunIntentsList {
            conversation_id: "conversation-1".into(),
            limit: 25,
            before_submitted_at_ms: None,
            before_intent_id: None,
            instance_id: None,
            instance_view: None,
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "list",
            "conversation-1",
            "--instance",
            "client-web-001",
            "--instance-view",
            "view.json",
        ])
        .unwrap(),
        RemoteCommand::PendingRunIntentsList {
            conversation_id: "conversation-1".into(),
            limit: 25,
            before_submitted_at_ms: None,
            before_intent_id: None,
            instance_id: Some("client-web-001".into()),
            instance_view: Some("view.json".into()),
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "list",
            "conversation-1",
            "--limit",
            "2",
            "--before-submitted-at-ms",
            "22",
            "--before-intent-id",
            "intent-3",
        ])
        .unwrap(),
        RemoteCommand::PendingRunIntentsList {
            conversation_id: "conversation-1".into(),
            limit: 2,
            before_submitted_at_ms: Some(22),
            before_intent_id: Some("intent-3".into()),
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "list",
            "conversation-1",
            "--before-intent-id",
            "intent-3",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "list",
            "conversation-1",
            "--limit",
            "26",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "list",
            "conversation-1",
            "--instance-view",
            "view.json",
        ])
        .is_err()
    );
}

#[test]
fn remote_pending_run_intent_timeline_parses_safe_cursor() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "timeline",
            "conversation-1",
            "intent-1",
            "--after-sequence",
            "3",
            "--limit",
            "1",
        ])
        .unwrap(),
        RemoteCommand::PendingRunIntentTimeline {
            conversation_id: "conversation-1".into(),
            intent_id: "intent-1".into(),
            after_sequence: 3,
            limit: 1,
            instance_id: None,
            instance_view: None,
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "timeline",
            "conversation-1",
            "intent-1",
            "--instance",
            "client-web-001",
            "--instance-view",
            "view.json",
        ])
        .unwrap(),
        RemoteCommand::PendingRunIntentTimeline {
            conversation_id: "conversation-1".into(),
            intent_id: "intent-1".into(),
            after_sequence: 0,
            limit: 25,
            instance_id: Some("client-web-001".into()),
            instance_view: Some("view.json".into()),
        }
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "run-intents",
            "timeline",
            "conversation-1",
            "intent-1",
            "--after-sequence",
            "9007199254740992",
        ])
        .is_err()
    );
}

#[test]
fn remote_pending_run_intent_submit_parses_prompt_and_requires_idempotency() {
    let parsed = parse_tokens(
        [
            "--idempotency-key",
            "intent-key",
            "remote",
            "run-intents",
            "submit",
            "conversation-1",
            "--expected-version",
            "7",
            "calculate",
            "this",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        parsed.command,
        Command::Remote(RemoteCommand::PendingRunIntentSubmit {
            conversation_id: "conversation-1".into(),
            expected_version: 7,
            content: "calculate this".into(),
            instance_id: None,
            instance_view: None,
        })
    );
    let scoped = parse_tokens(
        [
            "--idempotency-key",
            "intent-key",
            "remote",
            "run-intents",
            "submit",
            "conversation-1",
            "--expected-version",
            "7",
            "--instance",
            "client-web-001",
            "--instance-view",
            "view.json",
            "calculate",
            "this",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        scoped.command,
        Command::Remote(RemoteCommand::PendingRunIntentSubmit {
            conversation_id: "conversation-1".into(),
            expected_version: 7,
            content: "calculate this".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("view.json".into()),
        })
    );
    assert!(
        parse_tokens(
            [
                "--idempotency-key",
                "intent-key",
                "remote",
                "run-intents",
                "submit",
                "conversation-1",
                "--expected-version",
                "7",
                "--instance-view",
                "view.json",
                "calculate",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "run-intents",
                "submit",
                "conversation-1",
                "--expected-version",
                "7",
                "calculate",
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
    assert!(
        parse_tokens(
            [
                "--idempotency-key",
                "intent-key",
                "remote",
                "run-intents",
                "submit",
                "conversation-1",
                "--expected-version",
                "0",
                "calculate",
            ]
            .map(str::to_owned),
        )
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
            instance_id: None,
            instance_view: None,
        })
    );

    let scoped =
        parse_tokens(["-C", "/workspace", "remote", "sessions", "list"].map(str::to_owned))
            .unwrap_err();
    assert!(scoped.contains("project/group selectors"));
}

#[cfg(test)]
#[path = "remote_args/prompt_tests.rs"]
mod prompt_tests;

#[path = "remote_args/local_runner_preview_tests.rs"]
mod local_runner_preview_tests;
#[cfg(test)]
#[path = "remote_args/placement_tests.rs"]
mod placement_tests;
#[path = "remote_args/run_attempt_lease_dispatch_preflight_tests.rs"]
mod run_attempt_lease_dispatch_preflight_tests;
#[path = "remote_args/run_timeline_tests.rs"]
mod run_timeline_tests;
#[cfg(test)]
#[path = "remote_args/session_observation_tests.rs"]
mod session_observation_tests;
#[cfg(test)]
#[path = "remote_args/session_runner_receipt_tests.rs"]
mod session_runner_receipt_tests;
