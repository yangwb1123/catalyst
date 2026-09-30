use super::*;

#[tokio::test]
async fn tui_submits_pending_run_intent_with_selected_version_and_retries_history_read() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_intent_submission(listener));

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents submit compute this",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert!(state.pending_run_intent.is_none());
    assert_eq!(state.selected_entry.unwrap().aggregate_version, 3);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Pending Run-intent \"intent-2\" stored. No Run was started."));
    assert!(output.contains("Prompt history refreshed."));
}

fn serve_intent_submission(listener: TcpListener) {
    serve_submitted_intent(&listener);
    serve_submitted_prompt_history(&listener);
}

fn serve_submitted_intent(listener: &TcpListener) {
    let (mut submit, request, headers, body) = accept_request(listener);
    assert!(request.starts_with("POST /api/v1/conversations/c-1/run-intents "));
    assert!(headers.contains("idempotency-key: forge-tui-"));
    let request_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(request_json["content"], "compute this");
    assert_eq!(request_json["expected_version"], 2);
    respond(
        &mut submit,
        "201 Created",
        &json!({
            "prompt": {
                "id": "prompt-2",
                "conversation_id": "c-1",
                "role": "user",
                "content": "compute this",
                "created_at_ms": 30
            },
            "intent": {
                "intent_id": "intent-2",
                "conversation_id": "c-1",
                "prompt_id": "prompt-2",
                "project_id": "project-1",
                "profile_id": "profile-1",
                "submitted_at_ms": 30,
                "aggregate_version": 3,
                "latest_sequence": 1,
                "status": "pending"
            },
            "initial_event": {
                "event_id": "event-2",
                "seq": 1,
                "emitted_at_ms": 30,
                "type": "submitted"
            },
            "replayed": false
        }),
    );
}

fn serve_submitted_prompt_history(listener: &TcpListener) {
    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?limit=128 "));
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{
                "id": "prompt-2",
                "conversation_id": "c-1",
                "role": "user",
                "content": "compute this",
                "created_at_ms": 30
            }],
            "has_more": false
        }),
    );
}

#[tokio::test]
async fn tui_clears_pending_run_intent_and_refreshes_on_cas_conflict() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_cas_conflict(listener));

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents submit compute this",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert!(state.pending_run_intent.is_none());
    assert_eq!(state.selected_conversation().unwrap().aggregate_version, 3);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Pending Run-intent was rejected: Forge API returned HTTP 409"));
    assert!(output.contains("The session was refreshed; submit the pending Run-intent again."));
    assert!(!output.contains("Enter retry to reuse the same version"));
}

fn serve_cas_conflict(listener: TcpListener) {
    let (mut submit, request, headers, body) = accept_request(&listener);
    assert!(request.starts_with("POST /api/v1/conversations/c-1/run-intents "));
    assert!(headers.contains("idempotency-key: forge-tui-"));
    let request_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(request_json["content"], "compute this");
    assert_eq!(request_json["expected_version"], 2);
    respond(
        &mut submit,
        "409 Conflict",
        &json!({"code": "aggregate_version_conflict"}),
    );

    let (mut list, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversations?limit=128"));
    respond(
        &mut list,
        "200 OK",
        &json!({
            "conversations": [{
                "conversation": conversation_projection("c-1", "Shared"),
                "aggregate_version": 3
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
}
