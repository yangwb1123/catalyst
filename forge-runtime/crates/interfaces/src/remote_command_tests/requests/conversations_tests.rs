use super::*;

#[tokio::test]
async fn create_conversation_sends_bearer_and_idempotency_contract() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        required_headers: &["idempotency-key: create-key"],
        body_fields: json!({"scope": {"kind": "global"}, "title": "Shared"}),
        response_status: "201 Created",
        response: json!({
            "id": "c-1",
            "scope": {"kind": "global"},
            "title": "Shared",
            "created_at_ms": 10,
            "updated_at_ms": 10
        }),
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
async fn create_conversation_rejects_a_response_bound_to_another_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        required_headers: &["idempotency-key: create-key"],
        body_fields: json!({"scope": {"kind": "global"}, "title": "Shared"}),
        response_status: "201 Created",
        response: json!({
            "id": "c-1",
            "scope": {"kind": "global"},
            "title": "Another title",
            "created_at_ms": 10,
            "updated_at_ms": 10
        }),
    }]);
    let error = client
        .create_conversation("Shared", &RemoteConversationScope::Global, "create-key")
        .await
        .expect_err("the returned title must bind to the request");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid created conversation"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn get_conversation_detail_uses_exact_owner_scoped_read_path() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/c-1 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: session_entry("c-1", &json!({"kind": "global"}), 10),
    }]);
    let detail = client.get_conversation("c-1").await.unwrap();
    assert_eq!(detail.conversation["id"], "c-1");
    assert_eq!(detail.aggregate_version, 1);
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
        response: json!({
            "id": "c-1",
            "scope": {"kind": "project", "id": "prj_1"},
            "title": "Project work",
            "created_at_ms": 10,
            "updated_at_ms": 10
        }),
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
async fn session_list_instance_projection_filters_locally_without_query_authority() {
    let conversations = vec![
        session_entry("c-001", &json!({"kind": "global"}), 1),
        session_entry("c-002", &json!({"kind": "global"}), 2),
    ];
    let (client, server) = spawn_mock_server(vec![session_page_request(
        "GET /api/v1/conversations?limit=128 ",
        &json!(conversations),
        None,
        false,
    )]);
    let instance_scope = crate::client_instance_session_scope::ClientInstanceSessionScope {
        instance_id: "client-web-001".into(),
        client_kind: "web".into(),
        session_ids: ["c-002".to_owned()].into_iter().collect(),
    };
    let page = client
        .list_conversations_json_with_instance(None, None, Some(&instance_scope), false)
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(page["conversations"][0]["conversation"]["id"], "c-002");
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
