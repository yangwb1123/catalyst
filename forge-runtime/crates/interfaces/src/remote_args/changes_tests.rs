use super::*;

#[test]
fn remote_change_command_parses_bounded_cursor() {
    assert_eq!(
        parse_tokens(["remote", "changes", "list"].map(str::to_owned))
            .unwrap()
            .command,
        Command::Remote(RemoteCommand::ChangesList {
            after_cursor: None,
            instance_id: None,
            instance_view: None,
        })
    );
    assert_eq!(
        parse_tokens(["remote", "changes", "list", "--after-cursor", "19"].map(str::to_owned))
            .unwrap()
            .command,
        Command::Remote(RemoteCommand::ChangesList {
            after_cursor: Some(19),
            instance_id: None,
            instance_view: None,
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
            instance_id: None,
            instance_view: None,
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
fn remote_change_stream_parses_bounded_wait_and_cursor_options() {
    assert_eq!(
        parse_tokens(
            [
                "remote",
                "changes",
                "stream",
                "--after-cursor",
                "19",
                "--wait-ms",
                "10000"
            ]
            .map(str::to_owned)
        )
        .unwrap()
        .command,
        Command::Remote(RemoteCommand::ChangesStream {
            after_cursor: Some(19),
            wait_ms: 10_000,
            instance_id: None,
            instance_view: None,
        })
    );
    assert_eq!(
        parse_tokens(["remote", "changes", "stream"].map(str::to_owned))
            .unwrap()
            .command,
        Command::Remote(RemoteCommand::ChangesStream {
            after_cursor: None,
            wait_ms: 5_000,
            instance_id: None,
            instance_view: None,
        })
    );
    assert_change_stream_rejects_invalid_wait();
}

fn assert_change_stream_rejects_invalid_wait() {
    assert!(
        parse_tokens(["remote", "changes", "stream", "--wait-ms", "10001"].map(str::to_owned))
            .is_err()
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "changes",
                "stream",
                "--wait-ms",
                "1",
                "--wait-ms",
                "2"
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
}

#[test]
fn remote_change_feeds_parse_instance_projection_options() {
    assert_change_list_instance_projection();
    assert_change_watch_instance_projection();
    assert_change_stream_instance_projection();
}

fn assert_change_list_instance_projection() {
    assert_eq!(
        parse_tokens(
            [
                "remote",
                "changes",
                "list",
                "--instance",
                "client-web-001",
                "--instance-view",
                "view.json",
                "--after-cursor",
                "19"
            ]
            .map(str::to_owned)
        )
        .unwrap()
        .command,
        Command::Remote(RemoteCommand::ChangesList {
            after_cursor: Some(19),
            instance_id: Some("client-web-001".into()),
            instance_view: Some("view.json".into()),
        })
    );
}

fn assert_change_watch_instance_projection() {
    assert_eq!(
        parse_tokens(
            [
                "remote",
                "changes",
                "watch",
                "--instance",
                "client-web-001",
                "--polls",
                "1"
            ]
            .map(str::to_owned)
        )
        .unwrap()
        .command,
        Command::Remote(RemoteCommand::ChangesWatch {
            after_cursor: None,
            polls: 1,
            min_delay_ms: 250,
            max_delay_ms: 5_000,
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        })
    );
}

fn assert_change_stream_instance_projection() {
    assert_eq!(
        parse_tokens(
            [
                "remote",
                "changes",
                "stream",
                "--instance-view",
                "view.json",
                "--instance",
                "client-web-001"
            ]
            .map(str::to_owned)
        )
        .unwrap()
        .command,
        Command::Remote(RemoteCommand::ChangesStream {
            after_cursor: None,
            wait_ms: 5_000,
            instance_id: Some("client-web-001".into()),
            instance_view: Some("view.json".into()),
        })
    );
    assert!(
        parse_tokens(
            [
                "remote",
                "changes",
                "stream",
                "--instance-view",
                "view.json"
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
}
