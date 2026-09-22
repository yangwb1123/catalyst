use std::{net::TcpListener, thread};

use serde_json::json;

use super::super::{OwnedConversationEntry, commands::dispatch_command, state::TuiState};
use super::helpers::{accept_request, respond, test_client};

#[tokio::test]
async fn remote_tui_changes_watch_updates_state_and_uses_only_get_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        for response in [
            json!({
                "after_cursor": 4,
                "scanned_through_cursor": 4,
                "has_more": false,
                "changes": []
            }),
            json!({
                "after_cursor": 4,
                "scanned_through_cursor": 5,
                "has_more": false,
                "changes": [{
                    "cursor": 5,
                    "schema_version": 1,
                    "conversation_id": "c-1",
                    "entity_id": "p-1",
                    "aggregate_version": 2,
                    "kind": "prompt_appended",
                    "created_at_ms": 20
                }]
            }),
        ] {
            let (mut stream, request, headers, body) = accept_request(&listener);
            assert!(
                request.starts_with("GET /api/v1/conversation-changes?after_cursor=4&limit=128 ")
            );
            assert!(headers.contains("authorization: bearer test-token"));
            assert!(body.is_empty(), "watch must not send a network write body");
            respond(&mut stream, "200 OK", &response);
        }
    });

    let client = test_client(address);
    let mut state = TuiState {
        change_cursor: 4,
        conversations: vec![OwnedConversationEntry {
            conversation: json!({
                "id": "c-1",
                "scope": {"kind": "global"},
                "title": "Shared",
                "created_at_ms": 1,
                "updated_at_ms": 1
            }),
            aggregate_version: 1,
        }],
        ..TuiState::default()
    };
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes watch --after-cursor 4 --polls 2 --min-delay-ms 0 --max-delay-ms 0",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert_eq!(state.change_cursor, 5);
    assert_eq!(state.conversations[0].aggregate_version, 2);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Changes watch start_cursor=4 scanned_through_cursor=5 polls=2 changes=1 has_more=false"
        ),
        "{output}"
    );
    assert!(
        output.contains("one-off; saved checkpoint unchanged"),
        "{output}"
    );
    assert!(
        output.contains("Change cursor=5 conversation=\"c-1\" entity=\"p-1\""),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_changes_watch_rejects_invalid_bounds_without_request() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes watch --polls 0",
        &mut writer,
    )
    .await
    .unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("--polls must be between 1 and 64"),
        "{output}"
    );
    assert_eq!(state.change_cursor, 0);
}
