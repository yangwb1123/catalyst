use super::*;

#[tokio::test]
async fn transient_prompt_write_is_not_automatically_replayed() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "write once", "expected_version": 7}),
        response_status: "503 Service Unavailable",
        response: json!({"code": "temporarily_unavailable"}),
    }]);
    let error = client
        .append_prompt("c-1", 7, "write once", "prompt-key")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned HTTP 503 (temporarily_unavailable)"
    );
    server.join().unwrap();
}
