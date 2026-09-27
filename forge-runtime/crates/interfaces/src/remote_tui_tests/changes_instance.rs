use std::{io::Write, net::TcpListener, thread};

use serde_json::{Value, json};

use super::super::{OwnedConversationEntry, commands::dispatch_command, state::TuiState};
use super::helpers::{accept_request, respond, test_client};

fn client_instance_views() -> (Value, Value) {
    (
        serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
        ))
        .unwrap(),
    )
}

fn state_with_two_conversations() -> TuiState {
    TuiState {
        change_cursor: 4,
        conversations: vec![
            OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-001",
                    "scope": {"kind": "global"},
                    "title": "Visible",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }),
                aggregate_version: 1,
            },
            OwnedConversationEntry {
                conversation: json!({
                    "id": "conversation-002",
                    "scope": {"kind": "global"},
                    "title": "Hidden",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                }),
                aggregate_version: 1,
            },
        ],
        ..TuiState::default()
    }
}

fn change_page() -> Value {
    json!({
        "after_cursor": 4,
        "scanned_through_cursor": 6,
        "has_more": false,
        "changes": [
            {
                "cursor": 5,
                "schema_version": 1,
                "conversation_id": "conversation-001",
                "entity_id": "prompt-001",
                "aggregate_version": 2,
                "kind": "prompt_appended",
                "created_at_ms": 20
            },
            {
                "cursor": 6,
                "schema_version": 1,
                "conversation_id": "conversation-002",
                "entity_id": "prompt-002",
                "aggregate_version": 2,
                "kind": "prompt_appended",
                "created_at_ms": 21
            }
        ]
    })
}

fn serve_pair(listener: &TcpListener, session: &Value, resource: &Value) {
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    respond(&mut stream, "200 OK", session);
    let (mut stream, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    respond(&mut stream, "200 OK", resource);
}

#[tokio::test]
async fn remote_tui_changes_list_instance_filters_rows_but_advances_cursor() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (session, resource) = client_instance_views();
    let server = thread::spawn(move || {
        serve_pair(&listener, &session, &resource);
        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=4&limit=128 "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &change_page());
    });
    let client = test_client(address);
    let mut state = state_with_two_conversations();
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes list --after-cursor 4 --instance client-web-001",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();
    assert_eq!(state.change_cursor, 6);
    assert_eq!(state.conversations[0].aggregate_version, 2);
    assert_eq!(state.conversations[1].aggregate_version, 1);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Changes list start_cursor=4 scanned_through_cursor=6 changes=1"),
        "{output}"
    );
    assert!(
        output.contains("conversation=\"conversation-001\""),
        "{output}"
    );
    assert!(!output.contains("conversation-002"), "{output}");
}

#[tokio::test]
async fn remote_tui_changes_watch_instance_filters_hidden_rows_but_advances_cursor() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (session, resource) = client_instance_views();
    let server = thread::spawn(move || {
        serve_pair(&listener, &session, &resource);
        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=4&limit=128 "));
        assert!(body.is_empty());
        respond(
            &mut stream,
            "200 OK",
            &json!({
                "after_cursor": 4,
                "scanned_through_cursor": 6,
                "has_more": false,
                "changes": change_page()["changes"].clone()
            }),
        );
    });
    let client = test_client(address);
    let mut state = state_with_two_conversations();
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes watch --after-cursor 4 --polls 1 --min-delay-ms 0 --max-delay-ms 0 --instance client-web-001",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();
    assert_eq!(state.change_cursor, 6);
    assert_eq!(state.conversations[0].aggregate_version, 2);
    assert_eq!(state.conversations[1].aggregate_version, 1);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Changes watch start_cursor=4 scanned_through_cursor=6 polls=1 changes=1"),
        "{output}"
    );
    assert!(!output.contains("conversation-002"), "{output}");
}

#[tokio::test]
async fn remote_tui_changes_stream_instance_filters_hidden_rows_but_advances_cursor() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (session, resource) = client_instance_views();
    let server = thread::spawn(move || {
        serve_pair(&listener, &session, &resource);
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversation-changes/stream?after_cursor=4&limit=128&wait_ms=0 "
        ));
        assert!(headers.contains("accept: text/event-stream"));
        assert!(body.is_empty());
        let payload = format!(
            "event: conversation_changes\nid: 6\ndata: {}\n\n",
            serde_json::to_string(&change_page()).unwrap()
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            payload.len(),
            payload
        )
        .unwrap();
    });
    let client = test_client(address);
    let mut state = state_with_two_conversations();
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes stream --after-cursor 4 --wait-ms 0 --instance client-web-001",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();
    assert_eq!(state.change_cursor, 6);
    assert_eq!(state.conversations[0].aggregate_version, 2);
    assert_eq!(state.conversations[1].aggregate_version, 1);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output
            .contains("Changes stream start_cursor=4 scanned_through_cursor=6 wait_ms=0 changes=1"),
        "{output}"
    );
    assert!(!output.contains("conversation-002"), "{output}");
}
