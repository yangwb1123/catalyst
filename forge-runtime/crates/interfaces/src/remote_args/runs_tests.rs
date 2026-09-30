use super::*;

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
    assert_run_timeline_instance_projection();
}

fn assert_run_timeline_instance_projection() {
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
