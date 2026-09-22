use super::*;

#[tokio::test]
async fn pending_run_intent_list_sends_owner_scoped_cursor_and_returns_metadata_only() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/run-intents?limit=2&before_submitted_at_ms=30&before_intent_id=intent-9 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-1",
            "intents": [{
                "intent_id": "intent-8",
                "conversation_id": "conversation-1",
                "prompt_id": "prompt-8",
                "project_id": "project-1",
                "profile_id": "profile-1",
                "submitted_at_ms": 20,
                "aggregate_version": 2,
                "latest_sequence": 1,
                "status": "pending"
            }],
            "next_cursor": {"submitted_at_ms": 20, "intent_id": "intent-8"},
            "has_more": true
        }),
    }]);
    let page = client
        .list_pending_run_intents("conversation-1", 2, Some(30), Some("intent-9"))
        .await
        .unwrap();
    assert_eq!(page["conversation_id"], "conversation-1");
    assert_eq!(page["intents"][0]["status"], "pending");
    assert!(page["intents"][0].get("content").is_none());
    server.join().unwrap();
}

#[tokio::test]
async fn pending_run_intent_timeline_sends_cursor_and_rejects_payload_fields() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/run-intents/intent-1/timeline?after_sequence=0&limit=1 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-1",
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
    }]);
    let page = client
        .pending_run_intent_timeline("conversation-1", "intent-1", 0, 1)
        .await
        .unwrap();
    assert_eq!(page["events"][0]["type"], "submitted");
    assert!(page["events"][0].get("content").is_none());
    server.join().unwrap();
}

#[tokio::test]
async fn pending_run_intent_submit_posts_idempotent_prompt_and_validates_receipt() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/run-intents ",
        required_headers: &["idempotency-key: intent-key"],
        body_fields: json!({"content": "run this", "expected_version": 7}),
        response_status: "201 Created",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-1",
                "role": "user",
                "content": "run this",
                "created_at_ms": 20
            },
            "intent": {
                "intent_id": "intent-1",
                "conversation_id": "conversation-1",
                "prompt_id": "prompt-1",
                "project_id": "project-1",
                "profile_id": "profile-1",
                "submitted_at_ms": 20,
                "aggregate_version": 8,
                "latest_sequence": 1,
                "status": "pending"
            },
            "initial_event": {
                "event_id": "event-1",
                "seq": 1,
                "emitted_at_ms": 20,
                "type": "submitted"
            },
            "replayed": false
        }),
    }]);
    let result = client
        .submit_pending_run_intent("conversation-1", 7, "run this", "intent-key")
        .await
        .unwrap();
    assert_eq!(result["intent"]["status"], "pending");
    assert_eq!(result["prompt"]["content"], "run this");
    assert_eq!(result["replayed"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn pending_run_intent_submit_rejects_receipt_binding_drift() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/run-intents ",
        required_headers: &["idempotency-key: intent-key"],
        body_fields: json!({"content": "run this", "expected_version": 7}),
        response_status: "200 OK",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-1",
                "role": "user",
                "content": "another prompt",
                "created_at_ms": 20
            },
            "intent": {
                "intent_id": "intent-1",
                "conversation_id": "conversation-1",
                "prompt_id": "prompt-1",
                "project_id": "project-1",
                "profile_id": "profile-1",
                "submitted_at_ms": 20,
                "aggregate_version": 8,
                "latest_sequence": 1,
                "status": "pending"
            },
            "initial_event": {
                "event_id": "event-1",
                "seq": 1,
                "emitted_at_ms": 20,
                "type": "submitted"
            },
            "replayed": true
        }),
    }]);
    let error = client
        .submit_pending_run_intent("conversation-1", 7, "run this", "intent-key")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid pending Run-intent submission"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn pending_run_intent_reads_reject_unknown_fields() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/run-intents?limit=25 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-1",
            "intents": [],
            "has_more": false,
            "prompt_content": "must not cross the read contract"
        }),
    }]);
    assert!(
        client
            .list_pending_run_intents("conversation-1", 25, None, None)
            .await
            .is_err()
    );
    server.join().unwrap();
}
