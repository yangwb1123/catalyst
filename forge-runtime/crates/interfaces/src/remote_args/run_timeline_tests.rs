use super::{RemoteCommand, parse_remote_command};

#[test]
fn remote_run_timeline_parses_default_and_custom_cursors() {
    assert_eq!(
        parse_remote_command(&["remote", "runs", "timeline", "conversation-1", "run-1"]).unwrap(),
        RemoteCommand::RunTimeline {
            conversation_id: "conversation-1".into(),
            run_id: "run-1".into(),
            after_sequence: 0,
            limit: 128,
            resume: false,
            instance_id: None,
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
            resume: false,
            instance_id: None,
            instance_view: None,
        }
    );
}

#[test]
fn remote_run_timeline_resume_is_explicit_and_excludes_manual_cursor() {
    assert_eq!(
        parse_remote_command(&[
            "remote",
            "runs",
            "timeline",
            "conversation-1",
            "run-1",
            "--resume",
        ])
        .unwrap(),
        RemoteCommand::RunTimeline {
            conversation_id: "conversation-1".into(),
            run_id: "run-1".into(),
            after_sequence: 0,
            limit: 128,
            resume: true,
            instance_id: None,
            instance_view: None,
        }
    );
    assert!(
        parse_remote_command(&[
            "remote",
            "runs",
            "timeline",
            "conversation-1",
            "run-1",
            "--resume",
            "--after-sequence",
            "3",
        ])
        .is_err()
    );
}

#[test]
fn remote_run_timeline_rejects_invalid_cursors_and_limits() {
    for invalid in ["-1", "9007199254740992", "not-a-number"] {
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
            "--after-sequence",
            "9007199254740991",
        ])
        .is_ok()
    );
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
