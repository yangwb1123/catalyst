use std::{io::Write, net::TcpListener, thread};

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

#[tokio::test]
async fn remote_tui_changes_stream_applies_sse_page_and_keeps_explicit_cursor_one_off() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversation-changes/stream?after_cursor=4&limit=128&wait_ms=0 "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(headers.contains("accept: text/event-stream"));
        assert!(body.is_empty());
        let payload = json!({
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
        });
        let frame = format!(
            "event: conversation_changes\nid: 5\ndata: {}\n\n",
            serde_json::to_string(&payload).unwrap()
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            frame.len(),
            frame
        )
        .unwrap();
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
        "changes stream --after-cursor 4 --wait-ms 0",
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
            "Changes stream start_cursor=4 scanned_through_cursor=5 wait_ms=0 changes=1 has_more=false timed_out=false"
        ),
        "{output}"
    );
    assert!(
        output.contains("one-off; saved checkpoint unchanged"),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_changes_stream_rejects_wait_bound_without_request() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes stream --wait-ms 10001",
        &mut writer,
    )
    .await
    .unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("--wait-ms must be between 0 and 10000"),
        "{output}"
    );
    assert_eq!(state.change_cursor, 0);
}

#[tokio::test]
async fn remote_tui_changes_watch_refreshes_selected_instance_before_feed_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let session_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let resource_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    let served_session_view = session_view.clone();
    let served_resource_view = resource_view.clone();
    let server = thread::spawn(move || {
        let (mut stream, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        respond(&mut stream, "200 OK", &served_session_view);

        let (mut stream, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        respond(&mut stream, "200 OK", &served_resource_view);

        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=4&limit=128"));
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(
            &mut stream,
            "200 OK",
            &json!({
                "after_cursor": 4,
                "scanned_through_cursor": 5,
                "has_more": false,
                "changes": [{
                    "cursor": 5,
                    "schema_version": 1,
                    "conversation_id": "conversation-001",
                    "entity_id": "prompt-001",
                    "aggregate_version": 2,
                    "kind": "prompt_appended",
                    "created_at_ms": 20
                }]
            }),
        );
    });

    let client = test_client(address);
    let mut state = instance_filtered_state(session_view, resource_view);
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes watch --after-cursor 4 --polls 1 --min-delay-ms 0 --max-delay-ms 0",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert_eq!(state.change_cursor, 5);
    assert_eq!(state.conversations[0].aggregate_version, 2);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Selected client-instance/session-view refreshed."),
        "{output}"
    );
    assert!(
        output.contains(
            "Changes watch start_cursor=4 scanned_through_cursor=5 polls=1 changes=1 has_more=false"
        ),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_changes_stream_blocks_revoked_instance_before_sse_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut session_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let mut resource_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    for view in [&mut session_view, &mut resource_view] {
        let instances = view["instances"].as_array_mut().unwrap();
        let web = instances
            .iter_mut()
            .find(|instance| instance["instance_id"] == "client-web-001")
            .unwrap();
        web["session_ids"] = json!(["conversation-002"]);
    }
    let server = thread::spawn(move || {
        let (mut stream, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        respond(&mut stream, "200 OK", &session_view);

        let (mut stream, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        respond(&mut stream, "200 OK", &resource_view);
    });

    let client = test_client(address);
    let mut state = instance_filtered_state(
        serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
        ))
        .unwrap(),
    );
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes stream --after-cursor 4 --wait-ms 0",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert_eq!(state.change_cursor, 4);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Conversation read blocked by client-instance display filter: conversation \"conversation-001\" is not declared for instance \"client-web-001\". No request was sent."
        ),
        "{output}"
    );
    assert!(!output.contains("Changes stream start_cursor="), "{output}");
}

#[tokio::test]
async fn remote_tui_changes_stream_blocks_nonconverged_instance_before_sse_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let session_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let mut resource_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    let web = resource_view["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["instance_id"] == "client-web-001")
        .unwrap();
    web["observed_at_ms"] = json!(200501);
    let server = thread::spawn(move || {
        let (mut stream, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        respond(&mut stream, "200 OK", &session_view);

        let (mut stream, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        respond(&mut stream, "200 OK", &resource_view);
    });

    let client = test_client(address);
    let mut state = instance_filtered_state(
        serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
        ))
        .unwrap(),
        serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
        ))
        .unwrap(),
    );
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "changes stream --after-cursor 4 --wait-ms 0",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert_eq!(state.change_cursor, 4);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Changes feed client-instance projection refresh failed: Forge API client-instance session/resource observations did not converge. The in-memory TUI cursor was not advanced."
        ),
        "{output}"
    );
    assert!(!output.contains("Changes stream start_cursor="), "{output}");
}

fn instance_filtered_state(
    session_view: serde_json::Value,
    resource_view: serde_json::Value,
) -> TuiState {
    TuiState {
        change_cursor: 4,
        conversations: vec![OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-001",
                "scope": {"kind": "global"},
                "title": "Shared",
                "created_at_ms": 1,
                "updated_at_ms": 1
            }),
            aggregate_version: 1,
        }],
        selected_id: Some("conversation-001".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(session_view),
        client_instance_resource_view_observed: Some(resource_view),
        ..TuiState::default()
    }
}
