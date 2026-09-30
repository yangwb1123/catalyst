use super::*;

#[tokio::test]
async fn tui_reads_pending_run_intent_metadata_and_payload_free_timeline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_metadata_timeline(listener));

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(&client, &mut state, None, "run-intents", &mut writer)
        .await
        .unwrap();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents timeline intent-1",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Run-intent \"intent-1\" status=\"pending\" profile=\"profile-1\""));
    assert!(output.contains("Run-intent event seq=1 emitted_at_ms=20 type=\"submitted\""));
    assert!(!output.contains("prompt_content"));
    assert!(!output.contains("private"));
}

fn serve_metadata_timeline(listener: TcpListener) {
    let (mut list, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/run-intents?limit=25 "));
    respond(
        &mut list,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "intents": [{
                "intent_id": "intent-1",
                "conversation_id": "c-1",
                "prompt_id": "prompt-1",
                "project_id": "project-1",
                "profile_id": "profile-1",
                "submitted_at_ms": 20,
                "aggregate_version": 2,
                "latest_sequence": 1,
                "status": "pending"
            }],
            "has_more": false
        }),
    );

    let (mut timeline, request, _, _) = accept_request(&listener);
    assert!(request.starts_with(
        "GET /api/v1/conversations/c-1/run-intents/intent-1/timeline?after_sequence=0&limit=25 "
    ));
    respond(
        &mut timeline,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "intent_id": "intent-1",
            "after_sequence": 0,
            "scanned_through_sequence": 1,
            "has_more": false,
            "events": [{
                "event_id": "event-1",
                "seq": 1,
                "emitted_at_ms": 20,
                "type": "submitted"
            }]
        }),
    );
}

#[tokio::test]
async fn tui_sync_refreshes_an_explicitly_opened_pending_run_intent_page() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_metadata_refresh(listener));

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 1,
    });
    let mut writer = Vec::new();
    dispatch_command(&client, &mut state, None, "run-intents", &mut writer)
        .await
        .unwrap();
    dispatch_command(&client, &mut state, None, "sync", &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_eq!(output.matches("Run-intent \"intent-1\"").count(), 2);
    assert!(output.contains("profile=\"profile-2\""), "{output}");
    assert!(output.contains("Selected pending Run-intent metadata refreshed."));
    assert!(output.contains("Synced 0 owner-visible changes through cursor 0"));
}

fn serve_metadata_refresh(listener: TcpListener) {
    serve_initial_metadata_and_owner_refresh(&listener);
    serve_history_and_refreshed_metadata(&listener);
}

fn serve_initial_metadata_and_owner_refresh(listener: &TcpListener) {
    let (mut initial, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/run-intents?limit=25 "));
    assert!(body.is_empty());
    respond(
        &mut initial,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "intents": [{
                "intent_id": "intent-1",
                "conversation_id": "c-1",
                "prompt_id": "prompt-1",
                "project_id": "project-1",
                "profile_id": "profile-1",
                "submitted_at_ms": 20,
                "aggregate_version": 2,
                "latest_sequence": 1,
                "status": "pending"
            }],
            "has_more": false
        }),
    );

    let (mut changes, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
    assert!(body.is_empty());
    respond(
        &mut changes,
        "200 OK",
        &json!({
            "after_cursor": 0,
            "scanned_through_cursor": 0,
            "has_more": false,
            "changes": []
        }),
    );

    super::super::helpers::serve_conversation_page(
        listener,
        &super::super::helpers::conversation_page(2),
    );
}

fn serve_history_and_refreshed_metadata(listener: &TcpListener) {
    let (mut history, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    assert!(body.is_empty());
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "prompts": [],
            "has_more": false
        }),
    );

    let (mut refreshed, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/run-intents?limit=25 "));
    assert!(body.is_empty());
    respond(
        &mut refreshed,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "intents": [{
                "intent_id": "intent-1",
                "conversation_id": "c-1",
                "prompt_id": "prompt-1",
                "project_id": "project-1",
                "profile_id": "profile-2",
                "submitted_at_ms": 20,
                "aggregate_version": 2,
                "latest_sequence": 1,
                "status": "pending"
            }],
            "has_more": false
        }),
    );
}
