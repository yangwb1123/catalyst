use crate::args::parse_tokens;

use super::{Command, RemoteCommand};

fn parse_remote_command(tokens: &[&str]) -> Result<RemoteCommand, String> {
    parse_tokens(tokens.iter().map(|token| (*token).to_owned())).map(|args| match args.command {
        Command::Remote(command) => command,
        _ => unreachable!("remote argument helper parsed a non-remote command"),
    })
}

#[test]
fn remote_prompt_add_accepts_a_sole_stdin_marker_and_rejects_mixed_input() {
    let args = parse_tokens(
        [
            "--idempotency-key",
            "prompt-stdin",
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "-",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(
        args.command,
        Command::Remote(RemoteCommand::PromptsAdd {
            conversation_id: "conversation-1".into(),
            expected_version: 1,
            content: "-".into(),
            instance_id: None,
            instance_view: None,
        })
    );

    let mixed = parse_tokens(
        [
            "--idempotency-key",
            "prompt-stdin",
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "1",
            "-",
            "extra",
        ]
        .map(str::to_owned),
    )
    .unwrap_err();
    assert!(mixed.contains("stdin prompt marker '-' must be the only prompt token"));
}

#[test]
fn remote_prompt_add_rejects_expected_version_above_json_safe_integer() {
    let error = parse_remote_command(&[
        "remote",
        "prompts",
        "add",
        "conversation-1",
        "--expected-version",
        "9007199254740992",
        "hello",
    ])
    .unwrap_err();
    assert!(error.contains("invalid --expected-version"));
}

#[test]
fn remote_prompt_receipt_is_an_explicit_content_free_append_command() {
    assert_eq!(
        parse_remote_command(&[
            "--idempotency-key",
            "prompt-key",
            "remote",
            "prompts",
            "receipt",
            "conversation-1",
            "--expected-version",
            "3",
            "hello",
            "world",
        ])
        .unwrap(),
        RemoteCommand::PromptsReceipt {
            conversation_id: "conversation-1".into(),
            expected_version: 3,
            content: "hello world".into(),
            instance_id: None,
            instance_view: None,
        }
    );
}

#[test]
fn remote_prompt_history_cursor_accepts_absent_or_complete_pair() {
    assert_eq!(
        parse_remote_command(&["remote", "prompts", "list", "c-1"]).unwrap(),
        RemoteCommand::PromptsList {
            conversation_id: "c-1".into(),
            before_created_at_ms: None,
            before_prompt_id: None,
            instance_id: None,
            instance_view: None,
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
            instance_id: None,
            instance_view: None,
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

#[test]
fn remote_prompt_history_cursor_uses_json_safe_integer_boundary() {
    assert!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "list",
            "c-1",
            "--before-created-at-ms",
            "9007199254740991",
            "--before-prompt-id",
            "p-7",
        ])
        .is_ok()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "list",
            "c-1",
            "--before-created-at-ms",
            "9007199254740992",
            "--before-prompt-id",
            "p-7",
        ])
        .is_err()
    );
}

#[test]
fn remote_prompt_commands_parse_optional_client_instance_projection() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "list",
            "conversation-1",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-view.json",
        ])
        .unwrap(),
        RemoteCommand::PromptsList {
            conversation_id: "conversation-1".into(),
            before_created_at_ms: None,
            before_prompt_id: None,
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-view.json".into()),
        }
    );
    assert_eq!(
        parse_remote_command(&[
            "--idempotency-key",
            "prompt-key",
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "3",
            "--instance",
            "client-web-001",
            "--instance-view",
            "client-view.json",
            "hello",
            "world",
        ])
        .unwrap(),
        RemoteCommand::PromptsAdd {
            conversation_id: "conversation-1".into(),
            expected_version: 3,
            content: "hello world".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("client-view.json".into()),
        }
    );
}

#[test]
fn remote_prompt_instance_projection_requires_instance_and_rejects_invalid_ids() {
    assert!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "list",
            "conversation-1",
            "--instance-view",
            "client-view.json",
        ])
        .unwrap_err()
        .contains("--instance-view requires --instance")
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "3",
            "--instance-view",
            "client-view.json",
            "hello",
        ])
        .unwrap_err()
        .contains("--instance-view requires --instance")
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "list",
            "conversation-1",
            "--instance",
            "client/web",
        ])
        .is_err()
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "prompts",
            "add",
            "conversation-1",
            "--expected-version",
            "3",
            "--instance",
            "client-web-001",
            "--instance",
            "client-tui-001",
            "hello",
        ])
        .is_err()
    );
}
