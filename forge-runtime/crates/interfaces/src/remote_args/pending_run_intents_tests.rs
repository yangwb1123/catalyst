use super::*;

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
    assert_pending_run_intent_list_cursor();
    assert_pending_run_intent_list_rejects_invalid_options();
}

fn assert_pending_run_intent_list_cursor() {
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
}

fn assert_pending_run_intent_list_rejects_invalid_options() {
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
    assert_pending_run_intent_timeline_rejects_unsafe_cursor();
}

fn assert_pending_run_intent_timeline_rejects_unsafe_cursor() {
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
    assert_pending_run_intent_submit_instance_projection();
    assert_pending_run_intent_submit_requires_instance();
    assert_pending_run_intent_submit_requires_key_and_version();
}

fn assert_pending_run_intent_submit_instance_projection() {
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
}

fn assert_pending_run_intent_submit_requires_instance() {
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
}

fn assert_pending_run_intent_submit_requires_key_and_version() {
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
