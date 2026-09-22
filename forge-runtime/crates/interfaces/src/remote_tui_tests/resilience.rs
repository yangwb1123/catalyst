use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};

use super::{
    helpers::{
        accept_request, accept_stream, conversation_page, conversation_projection, read_request,
        respond, serve_ambiguous_create_retry, serve_ambiguous_prompt_retry,
        serve_conversation_page, serve_failed_refresh_keeps_session,
        serve_transient_history_failure, test_client,
    },
    run_with_io,
};

#[tokio::test]
async fn ambiguous_prompt_retry_reuses_the_same_key_version_and_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_ambiguous_prompt_retry(&listener));

    let client = test_client(address);
    let mut reader = Cursor::new("prompt finish the shared task\nquit\nretry\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("outcome is not confirmed"));
    assert!(output.contains("A write may have been accepted"));
    assert!(output.contains("Prompt retry replayed the existing message. No Run was started."));
    assert!(output.contains("Prompt history refreshed."));
    assert!(output.contains("finish the shared task"));
    assert!(output.contains("version 8"));
}

#[tokio::test]
async fn ambiguous_create_retry_reuses_the_same_key_and_title() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_ambiguous_create_retry(&listener));

    let client = test_client(address);
    let mut reader = Cursor::new(
        "create --scope project:prj_1 Shared session\nretry\nprompt continue newly created session\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Session creation outcome is not confirmed"));
    assert!(output.contains("Created session \"c-2\"."));
    assert!(output.contains("project:\"prj_1\""));
    assert!(output.contains("outside loaded pages"));
    assert!(output.contains("version 2"));
    assert!(output.contains("continue newly created session"));
}

#[tokio::test]
async fn discarding_pending_prompt_prints_recovery_metadata_without_prompt_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut list_stream, _) = accept_stream(&listener);
        let _ = read_request(&mut list_stream);
        respond(
            &mut list_stream,
            "200 OK",
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 7
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        drop(list_stream);

        let (mut prompt_stream, _) = accept_stream(&listener);
        let (request, _, _) = read_request(&mut prompt_stream);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
        respond(
            &mut prompt_stream,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("prompt private prompt body\nquit --discard-pending\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("\"operation\":\"append_prompt\""));
    assert!(output.contains("\"conversation_id\":\"c-1\""));
    assert!(output.contains("\"expected_version\":7"));
    assert!(output.contains("\"idempotency_key\":\"forge-tui-"));
    assert!(!output.contains("private prompt body"));
}

#[tokio::test]
async fn oversized_command_preserves_pending_write_recovery() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut list_stream, _) = accept_stream(&listener);
        let _ = read_request(&mut list_stream);
        respond(
            &mut list_stream,
            "200 OK",
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 7
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        drop(list_stream);

        let (mut prompt_stream, _) = accept_stream(&listener);
        let (request, _, _) = read_request(&mut prompt_stream);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
        respond(
            &mut prompt_stream,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
    });

    let mut input = String::from("prompt private prompt body\n");
    input.push_str(&"x".repeat(256 * 1024 + 1));
    input.push('\n');
    let client = test_client(address);
    let mut reader = Cursor::new(input);
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Input exceeds the 256 KiB limit."));
    assert!(output.contains("Pending write recovery:"));
    assert!(output.contains("\"expected_version\":7"));
    assert!(!output.contains("private prompt body"));
}

#[tokio::test]
async fn failed_refresh_keeps_the_loaded_session_available() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_failed_refresh_keeps_session(&listener));

    let client = test_client(address);
    let mut reader = Cursor::new("list\nopen c-1\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Refresh failed: Forge API returned HTTP 503"));
    assert!(output.contains("Prompt history for \"c-1\":"));
    assert!(output.contains("No prompts."));
}

#[tokio::test]
async fn successful_prompt_write_survives_history_refresh_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut history, _, _, _) = accept_request(&listener);
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "previous confirmed prompt", "created_at_ms": 10}],
                "has_more": false
            }),
        );
        let (mut post, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
        let prompt: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(prompt["content"], "new confirmed prompt");
        respond(&mut post, "201 Created", &json!({"aggregate_version": 2}));
        serve_transient_history_failure(&listener);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("open c-1\nprompt new confirmed prompt\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Prompt stored. No Run was started."));
    assert!(output.contains("Prompt was stored, but history refresh failed"));
    assert!(output.contains("previous confirmed prompt"));
    assert!(!output.contains("outcome is not confirmed"));
}
