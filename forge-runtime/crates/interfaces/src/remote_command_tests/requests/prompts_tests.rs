use super::*;

#[tokio::test]
async fn list_prompts_sends_bearer_and_reads_conversation_page() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/c-1/prompts?",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    }]);
    assert_eq!(
        client.list_prompts("c-1", None).await.unwrap()["conversation_id"],
        "c-1"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn transient_read_response_is_retried_with_the_same_authenticated_request() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/c-1/prompts?",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "429 Too Many Requests",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/c-1/prompts?",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        },
    ]);
    assert_eq!(
        client.list_prompts("c-1", None).await.unwrap()["conversation_id"],
        "c-1"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn older_prompt_page_uses_both_cursor_fields_and_stays_before_cursor() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/c-1/prompts?limit=128&before_created_at_ms=200&before_prompt_id=p-new ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-old", "conversation_id": "c-1", "role": "user",
                "content": "older prompt", "created_at_ms": 100}],
            "has_more": false
        }),
    }]);
    let page = client
        .list_prompts(
            "c-1",
            Some(&crate::args::PromptPageCursor {
                created_at_ms: 200,
                prompt_id: "p-new".into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(page["prompts"][0]["id"], "p-old");
    server.join().unwrap();
}
#[tokio::test]
async fn append_prompt_sends_bearer_cas_and_idempotency_contract() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "run this prompt", "expected_version": 7}),
        response_status: "200 OK",
        response: prompt_append_response("c-1", "run this prompt", 8, false),
    }]);
    assert_eq!(
        client
            .append_prompt("c-1", 7, "run this prompt", "prompt-key")
            .await
            .unwrap()["aggregate_version"],
        8
    );
    server.join().unwrap();
}

#[tokio::test]
async fn append_prompt_rejects_aggregate_version_above_json_safe_integer() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "run this prompt", "expected_version": 7}),
        response_status: "201 Created",
        response: prompt_append_response(
            "c-1",
            "run this prompt",
            9_007_199_254_740_992_u64,
            false,
        ),
    }]);
    let error = client
        .append_prompt("c-1", 7, "run this prompt", "prompt-key")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Prompt receipt"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn append_prompt_rejects_a_non_sequential_aggregate_version() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "run this prompt", "expected_version": 7}),
        response_status: "200 OK",
        response: prompt_append_response("c-1", "run this prompt", 9, false),
    }]);
    let error = client
        .append_prompt("c-1", 7, "run this prompt", "prompt-key")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Prompt receipt"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn append_prompt_rejects_a_missing_or_non_boolean_replay_marker() {
    for response in [
        json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "c-1",
                "role": "user",
                "content": "run this prompt",
                "created_at_ms": 300,
            },
            "aggregate_version": 8,
        }),
        json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "c-1",
                "role": "user",
                "content": "run this prompt",
                "created_at_ms": 300,
            },
            "aggregate_version": 8,
            "replayed": "false",
        }),
    ] {
        let (client, server) = spawn_mock_server(vec![ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/c-1/prompts ",
            required_headers: &["idempotency-key: prompt-key"],
            body_fields: json!({"content": "run this prompt", "expected_version": 7}),
            response_status: "200 OK",
            response,
        }]);
        let error = client
            .append_prompt("c-1", 7, "run this prompt", "prompt-key")
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Forge API returned an invalid Prompt receipt"
        );
        server.join().unwrap();
    }
}

#[tokio::test]
async fn append_prompt_preserves_multiline_content() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-multiline"],
        body_fields: json!({
            "content": "first line\nsecond line\n",
            "expected_version": 7
        }),
        response_status: "201 Created",
        response: prompt_append_response("c-1", "first line\nsecond line\n", 8, false),
    }]);
    client
        .append_prompt("c-1", 7, "first line\nsecond line\n", "prompt-multiline")
        .await
        .unwrap();
    server.join().unwrap();
}
