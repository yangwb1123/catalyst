use super::*;

#[tokio::test]
async fn visible_instance_prompt_list_reads_after_projection_check() {
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
            request_prefix: "GET /api/v1/conversations/conversation-001/prompts?",
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-001",
                "prompts": [],
                "has_more": false
            }),
        },
    ]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-001".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let page = execute_prompt_command(&client, &command, None, None)
        .await
        .expect("visible Conversation Prompt list");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_prompt_list_uses_the_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/prompts?",
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-001",
            "prompts": [],
            "has_more": false
        }),
    }]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-001".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let page = execute_prompt_command(&client, &command, None, None)
        .await
        .expect("local view should permit visible Conversation");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_convergence_view_permits_visible_prompt_append_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
        response_status: "201 Created",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-001",
                "role": "user",
                "content": "visible prompt from paired view",
                "created_at_ms": 300,
            },
            "aggregate_version": 4,
            "replayed": false
        }),
    }]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "visible prompt from paired view".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
    };
    let receipt = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect("paired convergence view should permit visible Prompt append");
    assert_eq!(receipt["aggregate_version"], 4);
    server.join().expect("mock server");
}

#[tokio::test]
async fn unfiltered_prompt_add_keeps_the_single_prompt_post() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
        response_status: "201 Created",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-001",
                "role": "user",
                "content": "unfiltered prompt",
                "created_at_ms": 300,
            },
            "aggregate_version": 4,
            "replayed": false
        }),
    }]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "unfiltered prompt".into(),
        instance_id: None,
        instance_view: None,
    };
    let receipt = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect("unfiltered Prompt append");
    assert_eq!(receipt["aggregate_version"], 4);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_prompt_append_from_convergence_view_is_rejected_before_post() {
    let (client, server) = spawn_mock_server(Vec::new());
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-002".into(),
        expected_version: 3,
        content: "must remain local to the selected instance".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
    };
    let error = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect_err("hidden Conversation must be rejected before Prompt POST");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_prompt_add_is_rejected_before_prompt_request() {
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
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-002".into(),
        expected_version: 3,
        content: "must not cross instance projection".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_prompt_add_posts_after_projection_check() {
    let (client, server) = converged_instance_write(ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
        response_status: "201 Created",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-001",
                "role": "user",
                "content": "visible prompt",
                "created_at_ms": 300,
            },
            "aggregate_version": 4,
            "replayed": false
        }),
    });
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "visible prompt".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let receipt = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect("visible Conversation Prompt append");
    assert_eq!(receipt["aggregate_version"], 4);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_prompt_receipt_reads_inventory_after_projection_check() {
    let (mut client, server) = converged_instance_write(ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
        response_status: "200 OK",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-001",
                "role": "user",
                "content": "visible receipt prompt",
                "created_at_ms": 300,
            },
            "aggregate_version": 4,
            "replayed": false
        }),
    });
    client.access_token = prompt_receipt_test_token();
    let command = RemoteCommand::PromptsReceipt {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "visible receipt prompt".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let receipt = execute_prompt_command(&client, &command, Some("receipt-key"), None)
        .await
        .expect("visible Prompt receipt append");
    assert_eq!(receipt["receipt"]["aggregate_version"], 4);
    assert_eq!(receipt["receipt"]["content_included"], false);
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_visible_instance_prompt_add_is_rejected_before_post() {
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
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "200 OK",
            response: inventory_resource_convergence()["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: {
                let mut value = inventory_resource_convergence()["resource_view"].clone();
                value["devices"][0]["heartbeat_sequence"] = json!(99);
                value
            },
        },
    ]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "inventory drift must block this Prompt".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect_err("inventory/resource drift must block the Prompt POST");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}
