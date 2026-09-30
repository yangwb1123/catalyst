use super::*;

#[tokio::test]
async fn hidden_instance_pending_run_intent_list_is_rejected_before_private_read() {
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
    let command = RemoteCommand::PendingRunIntentsList {
        conversation_id: "conversation-002".into(),
        limit: 25,
        before_submitted_at_ms: None,
        before_intent_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_pending_run_intent_command(&client, &command, None, None)
        .await
        .expect_err("hidden Conversation must be rejected before the Run-intent read");
    assert!(
        error
            .to_string()
            .contains("no pending Run-intent request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_pending_run_intent_list_reads_after_projection_check() {
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
            request_prefix: "GET /api/v1/conversations/conversation-001/run-intents?limit=25 ",
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-001",
                "intents": [],
                "has_more": false
            }),
        },
    ]);
    let command = RemoteCommand::PendingRunIntentsList {
        conversation_id: "conversation-001".into(),
        limit: 25,
        before_submitted_at_ms: None,
        before_intent_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let page = execute_pending_run_intent_command(&client, &command, None, None)
        .await
        .expect("visible Conversation pending Run-intent list");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_pending_run_intent_submit_is_rejected_before_post() {
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
    let command = RemoteCommand::PendingRunIntentSubmit {
        conversation_id: "conversation-002".into(),
        expected_version: 3,
        content: "must not cross instance projection".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_pending_run_intent_command(&client, &command, Some("intent-key"), None)
        .await
        .expect_err("hidden Conversation must be rejected before submit");
    assert!(
        error
            .to_string()
            .contains("no pending Run-intent request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_pending_run_intent_submit_reads_inventory_after_pair_before_post() {
    let pair = inventory_resource_convergence();
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
            response: pair["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: pair["resource_view"].clone(),
        },
        ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/conversation-001/run-intents ",
            response_status: "201 Created",
            response: submitted_pending_run_response(),
        },
    ]);
    let command = RemoteCommand::PendingRunIntentSubmit {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "run this".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let result = execute_pending_run_intent_command(&client, &command, Some("intent-key"), None)
        .await
        .expect("visible pending Run-intent submit");
    assert_eq!(result["intent"]["status"], "pending");
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_resource_drift_blocks_visible_pending_run_intent_submit() {
    let pair = inventory_resource_convergence();
    let mut drifted_resource = pair["resource_view"].clone();
    drifted_resource["devices"][0]["heartbeat_sequence"] = json!(99);
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
            response: pair["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let command = RemoteCommand::PendingRunIntentSubmit {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "must stop on drift".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_pending_run_intent_command(&client, &command, Some("intent-key"), None)
        .await
        .expect_err("inventory/resource drift must block pending Run-intent POST");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_pending_run_intent_timeline_reads_after_projection_check() {
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
            request_prefix: "GET /api/v1/conversations/conversation-001/run-intents/intent-1/timeline?after_sequence=0&limit=1 ",
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-001",
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
        },
    ]);
    let command = RemoteCommand::PendingRunIntentTimeline {
        conversation_id: "conversation-001".into(),
        intent_id: "intent-1".into(),
        after_sequence: 0,
        limit: 1,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let page = execute_pending_run_intent_command(&client, &command, None, None)
        .await
        .expect("visible Conversation pending Run-intent timeline");
    assert_eq!(page["intent_id"], "intent-1");
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_run_reads_are_rejected_before_private_requests() {
    let commands = [
        RemoteCommand::RunsList {
            conversation_id: "conversation-002".into(),
            limit: 25,
            before_created_at_ms: None,
            before_run_id: None,
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        },
        RemoteCommand::RunObserved {
            conversation_id: "conversation-002".into(),
            run_id: "run-1".into(),
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        },
        RemoteCommand::RunTimeline {
            conversation_id: "conversation-002".into(),
            run_id: "run-1".into(),
            after_sequence: 0,
            limit: 128,
            resume: false,
            instance_id: Some("client-web-001".into()),
            instance_view: None,
        },
    ];

    for command in commands {
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
        let error = execute_run_command(&client, &command)
            .await
            .expect_err("hidden Conversation must be rejected before the Run request");
        assert!(error.to_string().contains("no Run request was sent"));
        server.join().expect("mock server");
    }
}

#[tokio::test]
async fn local_instance_run_list_uses_the_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/runs?limit=25 ",
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-001",
            "runs": [],
            "has_more": false
        }),
    }]);
    let command = RemoteCommand::RunsList {
        conversation_id: "conversation-001".into(),
        limit: 25,
        before_created_at_ms: None,
        before_run_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let page = execute_run_command(&client, &command)
        .await
        .expect("local view should permit visible Run list");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

fn submitted_pending_run_response() -> Value {
    json!({
        "prompt": {
            "id": "prompt-1",
            "conversation_id": "conversation-001",
            "role": "user",
            "content": "run this",
            "created_at_ms": 20
        },
        "intent": {
            "intent_id": "intent-1",
            "conversation_id": "conversation-001",
            "prompt_id": "prompt-1",
            "project_id": "project-1",
            "profile_id": "profile-1",
            "submitted_at_ms": 20,
            "aggregate_version": 4,
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
    })
}
