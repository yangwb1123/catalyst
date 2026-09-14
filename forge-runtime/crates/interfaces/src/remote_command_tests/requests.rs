use super::*;

fn session_entry(id: &str, scope: &Value, created_at_ms: u64) -> Value {
    json!({
        "conversation": {
            "id": id,
            "scope": scope,
            "title": "Shared",
            "created_at_ms": created_at_ms,
            "updated_at_ms": created_at_ms,
        },
        "aggregate_version": 1
    })
}

fn session_page_request(
    request_prefix: &'static str,
    conversations: &Value,
    next_after_id: Option<&str>,
    has_more: bool,
) -> ExpectedRequest {
    ExpectedRequest {
        request_prefix,
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversations": conversations,
            "next_after_id": next_after_id,
            "has_more": has_more
        }),
    }
}

fn full_conversation_pages(page_count: usize) -> Vec<ExpectedRequest> {
    (0..page_count)
        .map(|page_index| {
            let first_row = page_index * 128;
            let conversations = (0..128)
                .map(|offset| {
                    let row = first_row + offset;
                    session_entry(
                        &format!("c-{row:05}"),
                        &json!({"kind": "global"}),
                        u64::try_from(row).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            let cursor = format!("c-{:05}", first_row + 127);
            session_page_request(
                "GET /api/v1/conversations",
                &json!(conversations),
                Some(&cursor),
                true,
            )
        })
        .collect()
}

#[tokio::test]
async fn create_conversation_sends_bearer_and_idempotency_contract() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        required_headers: &["idempotency-key: create-key"],
        body_fields: json!({"scope": {"kind": "global"}, "title": "Shared"}),
        response_status: "201 Created",
        response: json!({"id": "c-1"}),
    }]);
    assert_eq!(
        client
            .create_conversation("Shared", &RemoteConversationScope::Global, "create-key")
            .await
            .unwrap()["id"],
        "c-1"
    );
    server.join().unwrap();
}
#[tokio::test]
async fn create_project_conversation_sends_only_scope_metadata() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        required_headers: &["idempotency-key: project-key"],
        body_fields: json!({
            "scope": {"kind": "project", "id": "prj_1"},
            "title": "Project work"
        }),
        response_status: "201 Created",
        response: json!({"id": "c-1", "scope": {"kind": "project", "id": "prj_1"}}),
    }]);
    assert_eq!(
        client
            .create_conversation(
                "Project work",
                &RemoteConversationScope::Project("prj_1".into()),
                "project-key",
            )
            .await
            .unwrap()["scope"]["id"],
        "prj_1"
    );
    server.join().unwrap();
}
#[tokio::test]
async fn session_list_scope_filter_keeps_server_cursor_and_filters_exact_scope() {
    let conversations = (0..128)
        .map(|index| {
            let id = format!("c-{index:03}");
            let scope = if index == 0 {
                json!({"kind": "project", "id": "prj_1"})
            } else {
                json!({"kind": "global"})
            };
            session_entry(&id, &scope, index)
        })
        .collect::<Vec<_>>();
    let (client, server) = spawn_mock_server(vec![session_page_request(
        "GET /api/v1/conversations?limit=128 ",
        &json!(conversations),
        Some("c-127"),
        true,
    )]);
    let page = client
        .list_conversations_json(
            None,
            Some(&RemoteConversationScope::Project("prj_1".into())),
            false,
        )
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(page["conversations"][0]["conversation"]["id"], "c-000");
    assert_eq!(page["next_after_id"], "c-127");
    assert_eq!(page["has_more"], true);
    server.join().unwrap();
}

#[tokio::test]
async fn session_list_all_scans_later_pages_before_applying_scope_filter() {
    let first_page = (0..128)
        .map(|index| session_entry(&format!("c-{index:03}"), &json!({"kind": "global"}), index))
        .collect::<Vec<_>>();
    let second_page = vec![session_entry(
        "c-128",
        &json!({"kind": "project", "id": "prj_1"}),
        128,
    )];
    let (client, server) = spawn_mock_server(vec![
        session_page_request(
            "GET /api/v1/conversations?limit=128 ",
            &json!(first_page),
            Some("c-127"),
            true,
        ),
        session_page_request(
            "GET /api/v1/conversations?limit=128&after_id=c-127 ",
            &json!(second_page),
            None,
            false,
        ),
    ]);

    let page = client
        .list_conversations_json(
            None,
            Some(&RemoteConversationScope::Project("prj_1".into())),
            true,
        )
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(page["conversations"][0]["conversation"]["id"], "c-128");
    assert_eq!(page["next_after_id"], Value::Null);
    assert_eq!(page["has_more"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn session_list_all_stops_at_the_page_bound_and_returns_a_continuation() {
    let (client, server) = spawn_mock_server(full_conversation_pages(64));
    let page = client
        .list_conversations_json(None, None, true)
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 64 * 128);
    assert_eq!(page["next_after_id"], "c-08191");
    assert_eq!(page["has_more"], true);
    server.join().unwrap();
}
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
        response: json!({"aggregate_version": 8, "replayed": false}),
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
