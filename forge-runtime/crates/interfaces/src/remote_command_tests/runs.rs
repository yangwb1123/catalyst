use super::*;

#[tokio::test]
async fn run_list_sends_owner_scoped_cursor_and_validates_summary_page() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/runs?limit=2&before_created_at_ms=30&before_run_id=run-9 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-1",
            "runs": [{
                "run_id": "run-8",
                "prompt_id": "prompt-8",
                "created_at_ms": 20,
                "latest_sequence": 4,
                "status": "nonterminal"
            }],
            "next_cursor": {"created_at_ms": 20, "run_id": "run-8"},
            "has_more": true
        }),
    }]);
    let page = client
        .list_runs("conversation-1", 2, Some(30), Some("run-9"))
        .await
        .unwrap();
    assert_eq!(page["conversation_id"], "conversation-1");
    assert_eq!(page["runs"][0]["status"], "nonterminal");
    assert_eq!(page["next_cursor"]["run_id"], "run-8");
    server.join().unwrap();
}
#[tokio::test]
async fn run_list_rejects_unallowlisted_execution_metadata() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/runs?limit=25 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-1",
            "runs": [{
                "run_id": "run-8",
                "prompt_id": "prompt-8",
                "created_at_ms": 20,
                "latest_sequence": 4,
                "status": "nonterminal",
                "execution_json": "must not be exposed"
            }],
            "has_more": false
        }),
    }]);
    assert!(
        client
            .list_runs("conversation-1", 25, None, None)
            .await
            .is_err()
    );
    server.join().unwrap();
}

#[tokio::test]
async fn run_observed_sends_one_authenticated_get_without_a_body() {
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-observed-v1.json"
    ))
    .unwrap();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/runs/run-001/observation ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response,
    }]);
    let observed = client
        .read_run_observation("conversation-001", "run-001")
        .await
        .unwrap();
    assert_eq!(observed.conversation_id, "conversation-001");
    assert_eq!(observed.run_id, "run-001");
    assert!(observed.metadata_observed);
    assert!(!observed.content_included);
    assert!(!observed.authority.execution_authorized);
    assert!(!observed.authority.dispatch_performed);
    server.join().unwrap();
}

#[tokio::test]
async fn run_observed_rejects_binding_and_authority_mutations() {
    for (conversation_id, run_id, mutation) in [
        ("conversation-001", "run-001", "binding"),
        ("conversation-001", "run-001", "authority"),
        ("conversation-001", "run-001", "content"),
    ] {
        let mut response: Value = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-run-observed-v1.json"
        ))
        .unwrap();
        match mutation {
            "binding" => response["run_id"] = Value::String("run-other".into()),
            "authority" => response["authority"]["dispatch_performed"] = Value::Bool(true),
            "content" => response["content_included"] = Value::Bool(true),
            _ => unreachable!(),
        }
        let (client, server) = spawn_mock_server(vec![ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-001/runs/run-001/observation ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response,
        }]);
        assert!(
            client
                .read_run_observation(conversation_id, run_id)
                .await
                .is_err(),
            "mutation {mutation} must be rejected"
        );
        server.join().unwrap();
    }
}
#[tokio::test]
async fn run_timeline_sends_cursor_and_returns_only_metadata_markers() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/runs/run-8/timeline?after_sequence=3&limit=3 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-1",
            "run_id": "run-8",
            "after_sequence": 3,
            "scanned_through_sequence": 4,
            "has_more": true,
            "events": [{"seq": 4, "emitted_at_ms": 77, "type": "activity"}]
        }),
    }]);
    let page = client
        .run_timeline("conversation-1", "run-8", 3, 3)
        .await
        .unwrap();
    assert_eq!(page["events"][0]["seq"], 4);
    assert_eq!(page["events"][0]["type"], "activity");
    assert!(page["events"][0].get("content").is_none());
    server.join().unwrap();
}
#[tokio::test]
async fn run_timeline_rejects_detailed_event_types_and_payloads() {
    for event in [
        json!({"seq": 4, "emitted_at_ms": 77, "type": "assistant_delta"}),
        json!({
            "seq": 4,
            "emitted_at_ms": 77,
            "type": "activity",
            "tool_name": "private"
        }),
    ] {
        let (client, server) = spawn_mock_server(vec![ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-1/runs/run-8/timeline?after_sequence=3&limit=2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-1",
                "run_id": "run-8",
                "after_sequence": 3,
                "scanned_through_sequence": 4,
                "has_more": false,
                "events": [event]
            }),
        }]);
        assert!(
            client
                .run_timeline("conversation-1", "run-8", 3, 2)
                .await
                .is_err()
        );
        server.join().unwrap();
    }
}
#[tokio::test]
async fn run_timeline_rejects_sequence_gaps() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/runs/run-8/timeline?after_sequence=3&limit=2 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-1",
            "run_id": "run-8",
            "after_sequence": 3,
            "scanned_through_sequence": 5,
            "has_more": false,
            "events": [{"seq": 5, "emitted_at_ms": 77, "type": "activity"}]
        }),
    }]);
    assert!(
        client
            .run_timeline("conversation-1", "run-8", 3, 2)
            .await
            .is_err()
    );
    server.join().unwrap();
}
