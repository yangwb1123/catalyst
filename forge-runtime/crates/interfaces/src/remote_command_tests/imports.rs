use super::*;

#[tokio::test]
async fn import_conversation_posts_only_the_transcript_with_idempotency() {
    let prompts = [
        ConversationImportPrompt {
            role: "user".into(),
            content: "source question".into(),
        },
        ConversationImportPrompt {
            role: "assistant".into(),
            content: "source answer".into(),
        },
    ];
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/import ",
        required_headers: &["idempotency-key: import-key"],
        body_fields: json!({
            "title": "Imported transcript",
            "prompts": &prompts
        }),
        response_status: "201 Created",
        response: json!({
            "conversation": {
                "id": "remote-1",
                "scope": {"kind": "global"},
                "title": "Imported transcript",
                "created_at_ms": 1,
                "updated_at_ms": 2
            },
            "aggregate_version": 3,
            "imported_prompt_count": 2,
            "replayed": false
        }),
    }]);
    let response = client
        .import_owned_conversation("Imported transcript", &prompts, "import-key")
        .await
        .unwrap();
    assert_eq!(response["conversation"]["id"], "remote-1");
    server.join().unwrap();
}

#[tokio::test]
async fn import_conversation_rejects_malformed_response_at_client_boundary() {
    let prompts = [ConversationImportPrompt {
        role: "user".into(),
        content: "source question".into(),
    }];
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/import ",
        required_headers: &["idempotency-key: import-key"],
        body_fields: json!({
            "title": "Imported transcript",
            "prompts": &prompts
        }),
        response_status: "201 Created",
        response: json!({
            "conversation": {
                "id": "remote-1",
                "scope": {"kind": "global"},
                "title": "Imported transcript",
                "created_at_ms": 1,
                "updated_at_ms": 2
            },
            "aggregate_version": 3,
            "imported_prompt_count": 1,
            "replayed": false,
            "unexpected": true
        }),
    }]);

    let error = client
        .import_owned_conversation("Imported transcript", &prompts, "import-key")
        .await
        .expect_err("unknown import response fields must fail closed");
    assert!(error.to_string().contains("invalid"), "{error}");
    server.join().unwrap();
}

#[tokio::test]
async fn import_conversation_rejects_foreign_response_at_client_boundary() {
    let prompts = [ConversationImportPrompt {
        role: "user".into(),
        content: "source question".into(),
    }];
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/import ",
        required_headers: &["idempotency-key: import-key"],
        body_fields: json!({
            "title": "Imported transcript",
            "prompts": &prompts
        }),
        response_status: "201 Created",
        response: json!({
            "conversation": {
                "id": "foreign-1",
                "scope": {"kind": "project", "id": "project-1"},
                "title": "Imported transcript",
                "created_at_ms": 1,
                "updated_at_ms": 2
            },
            "aggregate_version": 3,
            "imported_prompt_count": 1,
            "replayed": false
        }),
    }]);

    let error = client
        .import_owned_conversation("Imported transcript", &prompts, "import-key")
        .await
        .expect_err("foreign import responses must fail closed");
    assert!(error.to_string().contains("invalid"), "{error}");
    server.join().unwrap();
}
