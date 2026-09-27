use super::super::OwnedConversationEntry;
use super::*;

fn client_instance_session_view() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap()
}

#[tokio::test]
async fn remote_tui_instance_filter_commits_only_declared_sessions_to_state() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        super::serve_conversation_page(
            &listener,
            &json!({
                "conversations": [
                    {
                        "conversation": {
                            "id": "conversation-001",
                            "scope": {"kind": "global"},
                            "title": "Shared",
                            "created_at_ms": 1,
                            "updated_at_ms": 1
                        },
                        "aggregate_version": 1
                    },
                    {
                        "conversation": {
                            "id": "conversation-002",
                            "scope": {"kind": "global"},
                            "title": "CLI only",
                            "created_at_ms": 2,
                            "updated_at_ms": 2
                        },
                        "aggregate_version": 1
                    }
                ],
                "next_after_id": null,
                "has_more": false
            }),
        );
    });
    let client = super::test_client(address);
    let mut state = TuiState {
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(client_instance_session_view()),
        ..TuiState::default()
    };

    super::super::state::refresh_sessions(&client, &mut state, false)
        .await
        .unwrap();
    let ids: Vec<_> = state
        .conversations
        .iter()
        .filter_map(|entry| entry.conversation.get("id").and_then(Value::as_str))
        .collect();
    assert_eq!(ids, vec!["conversation-001"]);
    server.join().unwrap();
}

#[tokio::test]
async fn remote_tui_instance_refresh_does_not_detail_read_a_hidden_selection() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        super::serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "conversation-001",
                        "scope": {"kind": "global"},
                        "title": "Visible",
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 2
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
    });
    let client = super::test_client(address);
    let hidden = json!({
        "id": "conversation-002",
        "scope": {"kind": "global"},
        "title": "Hidden",
        "created_at_ms": 1,
        "updated_at_ms": 1
    });
    let mut state = TuiState {
        conversations: vec![OwnedConversationEntry {
            conversation: hidden.clone(),
            aggregate_version: 1,
        }],
        selected_id: Some("conversation-002".into()),
        selected_entry: Some(OwnedConversationEntry {
            conversation: hidden,
            aggregate_version: 1,
        }),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(client_instance_session_view()),
        ..TuiState::default()
    };

    super::super::state::refresh_sessions(&client, &mut state, false)
        .await
        .unwrap();
    server.join().unwrap();

    assert_eq!(state.selected_id.as_deref(), Some("conversation-001"));
    assert!(state.selected_entry.is_none());
    assert_eq!(state.conversations.len(), 1);
}

#[tokio::test]
async fn remote_tui_instance_filter_without_a_view_fails_before_session_request() {
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState {
        client_instance_filter: Some("client-web-001".into()),
        ..TuiState::default()
    };

    let error = super::super::state::refresh_sessions(&client, &mut state, false)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "remote TUI client-instance filter \"client-web-001\" has no validated view"
    );
    assert!(state.conversations.is_empty());
}

#[tokio::test]
async fn remote_tui_refresh_drops_deleted_selected_session_after_owner_page_omits_it() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        super::serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "conversation-old",
                        "scope": {"kind": "global"},
                        "title": "Deleted session",
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        super::serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "conversation-new",
                        "scope": {"kind": "global"},
                        "title": "Remaining session",
                        "created_at_ms": 2,
                        "updated_at_ms": 2
                    },
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut detail, request, _, _) = super::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/conversation-old "));
        super::respond(
            &mut detail,
            "404 Not Found",
            &json!({"code": "not_found", "message": "deleted"}),
        );
    });

    let client = super::test_client(address);
    let mut state = TuiState::default();
    super::super::state::refresh_sessions(&client, &mut state, false)
        .await
        .unwrap();
    state.record_prompt_history(
        "conversation-old",
        &json!({
            "conversation_id": "conversation-old",
            "prompts": [{
                "id": "prompt-old",
                "conversation_id": "conversation-old",
                "role": "user",
                "content": "stale private prompt",
                "created_at_ms": 3
            }],
            "has_more": false
        }),
    );

    let error = super::super::state::refresh_sessions(&client, &mut state, false)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("HTTP 404"));
    assert_eq!(state.selected_id, None);
    assert!(state.selected_entry.is_none());
    assert!(state.prompt_history.is_empty());
    assert_eq!(
        state
            .conversations
            .iter()
            .filter_map(|entry| entry.conversation.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec!["conversation-new"]
    );
    server.join().unwrap();
}
