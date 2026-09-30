use super::*;

#[tokio::test]
async fn hidden_instance_prompt_list_is_rejected_before_prompt_request() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: resource_view(),
        },
    ]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-002".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, None, None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_session_detail_is_rejected_before_conversation_request() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: resource_view(),
        },
    ]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-002".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_session_command(&client, &command, None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no Conversation request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn online_instance_session_detail_rejects_resource_drift_before_private_read() {
    let mut drifted_resource = resource_view();
    drifted_resource["instances"][4]["observed_at_ms"] = json!(200501);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_session_command(&client, &command, None)
        .await
        .expect_err("resource drift must block the private Conversation read");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_session_detail_reads_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: resource_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-001 ",
            response_status: "200 OK",
            response: json!({
                "conversation": {
                    "id": "conversation-001",
                    "title": "Visible",
                    "scope": {"kind": "global"},
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                },
                "aggregate_version": 1
            }),
        },
    ]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let detail = execute_session_command(&client, &command, None)
        .await
        .expect("visible Conversation detail");
    assert_eq!(detail["conversation"]["id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn unfiltered_session_create_keeps_one_owner_post() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        response_status: "201 Created",
        response: json!({
            "id": "conversation-created",
            "scope": {"kind": "global"},
            "title": "Created",
            "created_at_ms": 1,
            "updated_at_ms": 1
        }),
    }]);
    let command = RemoteCommand::SessionsCreate {
        title: "Created".into(),
        scope: RemoteConversationScope::Global,
        instance_id: None,
        instance_view: None,
    };
    let created = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect("unfiltered session create");
    assert_eq!(created["id"], "conversation-created");
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_session_create_reads_converged_pair_before_owner_post() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: resource_view(),
        },
        ExpectedRequest {
            request_prefix: "POST /api/v1/conversations ",
            response_status: "201 Created",
            response: json!({
                "id": "conversation-created",
                "scope": {"kind": "global"},
                "title": "Created",
                "created_at_ms": 1,
                "updated_at_ms": 1
            }),
        },
    ]);
    let command = RemoteCommand::SessionsCreate {
        title: "Created".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let created = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect("visible instance session create");
    assert_eq!(created["id"], "conversation-created");
    server.join().expect("mock server");
}

#[tokio::test]
async fn drifted_instance_session_create_is_rejected_before_owner_post() {
    let mut drifted_resource = resource_view();
    drifted_resource["instances"][4]["observed_at_ms"] = json!(200501);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let command = RemoteCommand::SessionsCreate {
        title: "Must stay blocked".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect_err("resource drift must block the owner create");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_session_create_uses_view_without_candidate_reads() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        response_status: "201 Created",
        response: json!({
            "id": "conversation-created",
            "scope": {"kind": "global"},
            "title": "Created",
            "created_at_ms": 1,
            "updated_at_ms": 1
        }),
    }]);
    let command = RemoteCommand::SessionsCreate {
        title: "Created".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
    };
    let created = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect("local instance session create");
    assert_eq!(created["id"], "conversation-created");
    server.join().expect("mock server");
}

#[tokio::test]
async fn unknown_instance_session_create_is_rejected_before_owner_post() {
    let (client, server) = spawn_mock_server(Vec::new());
    let command = RemoteCommand::SessionsCreate {
        title: "Must stay blocked".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-unknown".into()),
        instance_view: Some(convergence_view_path()),
    };
    let error = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect_err("unknown instance must block the owner create");
    assert!(error.to_string().contains("does not declare instance"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_session_detail_uses_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001 ",
        response_status: "200 OK",
        response: json!({
            "conversation": {
                "id": "conversation-001",
                "title": "Visible",
                    "scope": {"kind": "global"},
                "created_at_ms": 1,
                "updated_at_ms": 1
            },
        "aggregate_version": 1
        }),
    }]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let detail = execute_session_command(&client, &command, None)
        .await
        .expect("local view should permit visible Conversation");
    assert_eq!(detail["conversation"]["id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_convergence_view_filters_session_list_before_returning_rows() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations?limit=128 ",
        response_status: "200 OK",
        response: visible_and_hidden_session_page(),
    }]);
    let command = RemoteCommand::SessionsList {
        after_id: None,
        scope: None,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
        all_pages: false,
    };
    let page = execute_session_command(&client, &command, None)
        .await
        .expect("paired convergence view should filter session rows locally");
    assert_eq!(
        page["conversations"].as_array().unwrap().len(),
        1,
        "only the selected instance's session should remain"
    );
    assert_eq!(
        page["conversations"][0]["conversation"]["id"],
        "conversation-001"
    );
    server.join().expect("mock server");
}

fn visible_and_hidden_session_page() -> Value {
    json!({
        "conversations": [
            {
                "conversation": {
                    "id": "conversation-001",
                    "title": "Visible",
                    "scope": {"kind": "global"},
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                },
                "aggregate_version": 1
            },
            {
                "conversation": {
                    "id": "conversation-002",
                    "title": "Hidden",
                    "scope": {"kind": "global"},
                    "created_at_ms": 2,
                    "updated_at_ms": 2
                },
                "aggregate_version": 1
            }
        ],
        "next_after_id": null,
        "has_more": false
    })
}
