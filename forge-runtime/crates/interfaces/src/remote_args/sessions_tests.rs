use super::*;

#[test]
fn remote_session_list_parses_exact_scope_and_cursor() {
    assert_session_list_defaults();
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

fn assert_session_list_defaults() {
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
    assert_session_detail_rejects_invalid_instance();
}

fn assert_session_detail_rejects_invalid_instance() {
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
            instance_id: None,
            instance_view: None,
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
fn remote_session_create_parses_optional_client_instance_projection() {
    let scoped = parse_tokens(
        [
            "--idempotency-key",
            "instance-create",
            "remote",
            "sessions",
            "create",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-instance-view.json",
            "--title",
            "Visible create",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        scoped.command,
        Command::Remote(RemoteCommand::SessionsCreate {
            title: "Visible create".into(),
            scope: RemoteConversationScope::Global,
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-instance-view.json".into()),
        })
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "sessions",
                "create",
                "--instance-view",
                "client-instance-view.json"
            ]
            .map(str::to_owned),
        )
        .is_err()
    );
    assert!(
        parse_tokens(
            ["remote", "sessions", "create", "--instance", "client/web"].map(str::to_owned),
        )
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
