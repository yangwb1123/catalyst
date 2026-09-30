use super::*;
use crate::remote_command::runner_execution_intent as runner_execution_intent_contract;

#[tokio::test]
async fn visible_instance_runner_execution_intent_reads_converged_pair_before_candidate_post() {
    let request = runner_execution_intent_request();
    let response = runner_execution_intent_response();
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
            response: inventory_resource_convergence()["resource_view"].clone(),
        },
        ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        None,
        "Runner execution-intent",
    )
    .await
    .expect("visible Conversation should pass the pair guard");
    let (conversation_id, run_id) =
        runner_execution_intent_contract::conversation_and_run(&request).unwrap();
    let returned = client
        .preview_runner_execution_intent(&conversation_id, &run_id, &request)
        .await
        .expect("visible Runner execution-intent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_runner_admission_accepts_device_or_runner_target() {
    for target_id in ["device-a", "runner-a"] {
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
                response: inventory_resource_convergence()["resource_view"].clone(),
            },
        ]);
        ensure_runner_admission_instance_projection(
            &client,
            "conversation-001",
            target_id,
            Some("client-web-001"),
            None,
            "Runner dispatch admission",
        )
        .await
        .expect("resource target must be accepted");
        server.join().expect("mock server");
    }
}

#[tokio::test]
async fn visible_instance_runner_admission_rejects_foreign_target_before_candidate_post() {
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
            response: inventory_resource_convergence()["resource_view"].clone(),
        },
    ]);
    let error = ensure_runner_admission_instance_projection(
        &client,
        "conversation-001",
        "runner-foreign",
        Some("client-web-001"),
        None,
        "Runner transport admission",
    )
    .await
    .expect_err("foreign target must fail before candidate POST");
    assert!(
        error
            .to_string()
            .contains("no Runner transport admission request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_runner_admission_resource_projection_rejects_foreign_target_without_post() {
    let (client, server) = spawn_mock_server(vec![]);
    let error = ensure_runner_admission_instance_projection(
        &client,
        "conversation-001",
        "runner-foreign",
        Some("client-web-001"),
        Some(&resource_view_path()),
        "Runner dispatch admission",
    )
    .await
    .expect_err("foreign local resource target must fail closed");
    assert!(
        error
            .to_string()
            .contains("no Runner dispatch admission request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_runner_admission_convergence_projection_accepts_matching_target() {
    let (client, server) = spawn_mock_server(vec![]);
    ensure_runner_admission_instance_projection(
        &client,
        "conversation-001",
        "runner-a",
        Some("client-web-001"),
        Some(&convergence_view_path()),
        "Runner transport admission",
    )
    .await
    .expect("converged local resource target must pass");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_runner_admission_session_only_view_rejects_before_post() {
    let (client, server) = spawn_mock_server(vec![]);
    let error = ensure_runner_admission_instance_projection(
        &client,
        "conversation-001",
        "runner-a",
        Some("client-web-001"),
        Some(&session_view_path()),
        "Runner dispatch admission",
    )
    .await
    .expect_err("session-only local view cannot bind a target");
    assert!(
        error
            .to_string()
            .contains("requires a resource or converged view")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_runner_execution_intent_blocks_inventory_resource_drift_before_candidate_post()
 {
    let request = runner_execution_intent_request();
    let mut drifted_resource = inventory_resource_convergence()["resource_view"].clone();
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
            response: inventory_resource_convergence()["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let error = ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        None,
        "Runner execution-intent",
    )
    .await
    .expect_err("inventory/resource drift must fail before the candidate POST");
    assert!(
        error
            .to_string()
            .contains("inventory/resource observations did not converge")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_runner_execution_intent_is_rejected_before_candidate_post() {
    let mut request = runner_execution_intent_request();
    request["conversation_id"] = json!("conversation-002");
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
    let error = ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        None,
        "Runner execution-intent",
    )
    .await
    .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no Runner execution-intent request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_runner_execution_intent_uses_view_without_pair_reads() {
    let request = runner_execution_intent_request();
    let response = runner_execution_intent_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        Some(&session_view_path()),
        "Runner execution-intent",
    )
    .await
    .expect("local visible view should pass the pair guard");
    let (conversation_id, run_id) =
        runner_execution_intent_contract::conversation_and_run(&request).unwrap();
    let returned = client
        .preview_runner_execution_intent(&conversation_id, &run_id, &request)
        .await
        .expect("local-view Runner execution-intent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}
