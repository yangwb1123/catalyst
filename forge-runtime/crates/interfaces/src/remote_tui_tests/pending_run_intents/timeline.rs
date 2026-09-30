use super::*;

#[tokio::test]
async fn tui_resumes_pending_run_intent_timeline_from_the_bound_in_process_cursor() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_timeline_resume(listener));

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
        "run-intents timeline intent-1",
        &mut writer,
    )
    .await
    .unwrap();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents timeline intent-1 --resume",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert_eq!(
        state.selected_pending_run_intent_conversation_id.as_deref(),
        Some("c-1")
    );
    assert_eq!(
        state.selected_pending_run_intent_id.as_deref(),
        Some("intent-1")
    );
    assert_eq!(state.pending_run_intent_timeline_sequence, 1);
    let output = String::from_utf8(writer).unwrap();
    assert_eq!(output.matches("Run-intent event seq=1").count(), 1);
    assert!(output.contains("No pending Run-intent timeline events on this page."));
}

fn serve_timeline_resume(listener: TcpListener) {
    let (mut initial, request, _, body) = accept_request(&listener);
    assert!(request.starts_with(
        "GET /api/v1/conversations/c-1/run-intents/intent-1/timeline?after_sequence=0&limit=25 "
    ));
    assert!(body.is_empty());
    respond(
        &mut initial,
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

    let (mut resumed, request, _, body) = accept_request(&listener);
    assert!(request.starts_with(
        "GET /api/v1/conversations/c-1/run-intents/intent-1/timeline?after_sequence=1&limit=25 "
    ));
    assert!(body.is_empty());
    respond(
        &mut resumed,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "intent_id": "intent-1",
            "after_sequence": 1,
            "scanned_through_sequence": 1,
            "has_more": false,
            "events": []
        }),
    );
}

#[tokio::test]
async fn tui_rejects_pending_run_intent_resume_without_an_exact_prior_binding() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
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
        "run-intents timeline intent-1 --resume",
        &mut writer,
    )
    .await
    .unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains(
        "Pending Run-intent timeline resume requires a prior read for this session and intent."
    ));
    assert_eq!(state.pending_run_intent_timeline_sequence, 0);
}
