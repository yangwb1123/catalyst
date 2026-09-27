pub(super) fn serve_ambiguous_prompt_retry(listener: &TcpListener) {
    serve_conversation_page(
        listener,
        &json!({
            "conversations": [{
                "conversation": conversation_projection("c-1", "Shared"),
                "aggregate_version": 7
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
    let (mut first_stream, first_request, first_headers, first_body) =
        receive_prompt_request(listener);
    respond(
        &mut first_stream,
        "503 Service Unavailable",
        &json!({"api_version": "forgeos.app-server/v1", "code": "temporarily_unavailable", "message": "retry"}),
    );
    drop(first_stream);
    let (mut retry_stream, retry_request, retry_headers, retry_body) = accept_request(listener);
    assert_eq!(retry_request, first_request);
    assert_eq!(retry_headers, first_headers);
    assert_eq!(retry_body, first_body);
    respond(
        &mut retry_stream,
        "201 Created",
        &prompt_append_response("c-1", "p-1", "finish the shared task", 8, true),
    );
    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-1", "conversation_id": "c-1", "role": "user", "content": "finish the shared task", "created_at_ms": 10}],
            "has_more": false
        }),
    );
}

pub(super) fn receive_prompt_request(
    listener: &TcpListener,
) -> (std::net::TcpStream, String, String, Vec<u8>) {
    let (stream, request, headers, body) = accept_request(listener);
    assert!(request.starts_with("POST /api/v1/conversations/c-1/prompts "));
    assert!(headers.contains("authorization: bearer eyj"));
    assert!(headers.contains("idempotency-key: forge-tui-"));
    let prompt: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(prompt["content"], "finish the shared task");
    assert_eq!(prompt["expected_version"], 7);
    (stream, request, headers, body)
}

pub(super) fn serve_ambiguous_create_retry(listener: &TcpListener) {
    serve_conversation_page(
        listener,
        &json!({"conversations": [], "next_after_id": null, "has_more": false}),
    );
    serve_idempotent_create(listener);
    serve_conversation_page(
        listener,
        &json!({"conversations": [], "next_after_id": null, "has_more": false}),
    );
    let (mut detail, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-2 "));
    respond(
        &mut detail,
        "200 OK",
        &json!({
            "conversation": {
                "id": "c-2",
                "scope": {"kind": "project", "id": "prj_1"},
                "title": "Shared session",
                "created_at_ms": 10,
                "updated_at_ms": 10
            },
            "aggregate_version": 1
        }),
    );
    serve_prompt_for_created_session(listener);
}

pub(super) fn serve_idempotent_create(listener: &TcpListener) {
    let (mut first_stream, first_request, first_headers, first_body) = accept_request(listener);
    assert!(first_request.starts_with("POST /api/v1/conversations "));
    assert!(first_headers.contains("idempotency-key: forge-tui-"));
    let first_json: Value = serde_json::from_slice(&first_body).unwrap();
    assert_eq!(first_json["title"], "Shared session");
    assert_eq!(first_json["scope"]["kind"], "project");
    assert_eq!(first_json["scope"]["id"], "prj_1");
    respond(
        &mut first_stream,
        "503 Service Unavailable",
        &json!({"api_version": "forgeos.app-server/v1", "code": "temporarily_unavailable", "message": "retry"}),
    );
    drop(first_stream);
    let (mut retry_stream, retry_request, retry_headers, retry_body) = accept_request(listener);
    assert_eq!(retry_request, first_request);
    assert_eq!(retry_headers, first_headers);
    assert_eq!(retry_body, first_body);
    respond(
        &mut retry_stream,
        "201 Created",
        &json!({
            "id": "c-2",
            "scope": {"kind": "project", "id": "prj_1"},
            "title": "Shared session",
            "created_at_ms": 10,
            "updated_at_ms": 10
        }),
    );
}

pub(super) fn serve_prompt_for_created_session(listener: &TcpListener) {
    let (mut stream, request, _, body) = accept_request(listener);
    assert!(request.starts_with("POST /api/v1/conversations/c-2/prompts "));
    let prompt: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(prompt["content"], "continue newly created session");
    assert_eq!(prompt["expected_version"], 1);
    respond(
        &mut stream,
        "201 Created",
        &prompt_append_response("c-2", "p-1", "continue newly created session", 2, false),
    );
    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-2/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "c-2",
            "prompts": [{"id": "p-1", "conversation_id": "c-2", "role": "user", "content": "continue newly created session", "created_at_ms": 10}],
            "has_more": false
        }),
    );
}

pub(super) fn prompt_append_response(
    conversation_id: &str,
    prompt_id: &str,
    content: &str,
    aggregate_version: u64,
    replayed: bool,
) -> Value {
    json!({
        "prompt": {
            "id": prompt_id,
            "conversation_id": conversation_id,
            "role": "user",
            "content": content,
            "created_at_ms": 10,
        },
        "aggregate_version": aggregate_version,
        "replayed": replayed,
    })
}
