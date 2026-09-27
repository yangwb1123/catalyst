use std::{io::Cursor, net::TcpListener, thread};

use serde_json::json;

use super::super::state::TuiState;
use super::{
    helpers::{
        accept_request, conversation_projection, respond, serve_conversation_page, test_client,
    },
    run_with_io,
};

#[tokio::test]
async fn remote_tui_filtered_out_selected_session_remains_openable_without_permission_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [],
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("filter group:grp_1\nopen c-1\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Opened session \"c-1\" and refreshed Prompt history."));
    assert!(!output.contains("That session is not in the loaded pages."));
    assert!(!output.to_lowercase().contains("permission"));
}

#[tokio::test]
async fn remote_tui_opens_owner_session_outside_loaded_page_via_detail_fallback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-first", "First page"),
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );

        let (mut detail, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-linked "));
        respond(
            &mut detail,
            "200 OK",
            &json!({
                "conversation": conversation_projection("c-linked", "Linked session"),
                "aggregate_version": 4
            }),
        );

        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-linked/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-linked",
                "prompts": [{
                    "id": "p-linked",
                    "conversation_id": "c-linked",
                    "role": "user",
                    "content": "prompt from linked session",
                    "created_at_ms": 10
                }],
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("open c-linked\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Opened session \"c-linked\" and refreshed Prompt history."));
    assert!(output.contains("\"c-linked\"  \"Linked session\""));
    assert!(output.contains("prompt from linked session"));
    assert!(!output.contains("That session is not in the loaded pages."));
}

#[tokio::test]
async fn remote_tui_shows_owner_session_detail_outside_loaded_page_via_detail_fallback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-first", "First page"),
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );

        let (mut detail, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-linked "));
        respond(
            &mut detail,
            "200 OK",
            &json!({
                "conversation": conversation_projection("c-linked", "Linked detail"),
                "aggregate_version": 7
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("detail c-linked\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Conversation detail:"));
    assert!(output.contains("c-linked"));
    assert!(output.contains("Linked detail"));
    assert!(output.contains("outside loaded pages"));
    assert!(!output.contains("That session is not in the loaded pages."));
}

#[tokio::test]
async fn remote_tui_failed_outside_page_lookup_preserves_current_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-first", "First page"),
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );

        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-first/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-first",
                "prompts": [{
                    "id": "p-first",
                    "conversation_id": "c-first",
                    "role": "user",
                    "content": "current session prompt",
                    "created_at_ms": 10
                }],
                "has_more": false
            }),
        );

        let (mut missing, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-missing "));
        respond(
            &mut missing,
            "404 Not Found",
            &json!({"code": "not_found", "detail": "private missing-session detail"}),
        );

        let (mut foreign, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-foreign "));
        respond(
            &mut foreign,
            "200 OK",
            &json!({
                "conversation": conversation_projection("c-other", "Foreign session"),
                "aggregate_version": 9
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("open c-first\nopen c-missing\nopen c-foreign\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Conversation detail request failed: Forge API returned HTTP 404"));
    assert!(output.contains("Prompt history for \"c-first\":"));
    assert!(output.contains("current session prompt"));
    assert!(!output.contains("c-missing\"  \""));
    assert!(!output.contains("private missing-session detail"));
    assert!(
        output.contains(
            "Conversation detail request failed: Forge API returned another conversation"
        )
    );
    assert!(!output.contains("Foreign session"));
}

#[tokio::test]
async fn remote_tui_drops_selected_session_after_a_later_history_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Deleted later"),
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );

        let (mut initial_history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut initial_history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{
                    "id": "p-private",
                    "conversation_id": "c-1",
                    "role": "user",
                    "content": "private prompt before deletion",
                    "created_at_ms": 10
                }],
                "has_more": false
            }),
        );

        let (mut rejected_history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut rejected_history,
            "404 Not Found",
            &json!({"code": "not_found", "message": "deleted"}),
        );
    });

    let client = test_client(address);
    let mut state = TuiState::default();
    super::super::state::refresh_sessions(&client, &mut state, false)
        .await
        .unwrap();
    super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "open c-1",
        &mut Vec::new(),
    )
    .await
    .unwrap();
    assert_eq!(state.selected_id.as_deref(), Some("c-1"));
    assert_eq!(state.prompt_history.len(), 1);

    super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "open c-1",
        &mut Vec::new(),
    )
    .await
    .unwrap();

    assert!(state.selected_id.is_none());
    assert!(state.selected_entry.is_none());
    assert!(state.prompt_history.is_empty());
    assert!(state.selected_run_id.is_none());
    assert!(state.conversations.iter().all(|entry| {
        entry
            .conversation
            .get("id")
            .and_then(serde_json::Value::as_str)
            != Some("c-1")
    }));
    server.join().unwrap();
}
