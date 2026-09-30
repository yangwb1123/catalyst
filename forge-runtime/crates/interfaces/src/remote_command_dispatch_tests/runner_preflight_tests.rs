use super::*;
use crate::remote_command::run_attempt_lease_dispatch_preflight as run_attempt_lease_dispatch_preflight_contract;
use crate::remote_command::runner_dispatch_plan_preview as runner_dispatch_plan_preview_contract;

#[tokio::test]
async fn visible_instance_runner_dispatch_plan_reads_converged_pair_before_candidate_post() {
    let request = runner_dispatch_plan_request();
    let response = runner_dispatch_plan_response(&request);
    let (client, server) = converged_instance_write(ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview ",
        response_status: "200 OK",
        response: response.clone(),
    });
    ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        None,
        "Runner dispatch-plan",
    )
    .await
    .expect("visible Conversation should pass the pair guard");
    let (conversation_id, run_id) =
        runner_dispatch_plan_preview_contract::conversation_and_run(&request).unwrap();
    let dispatch_plan = runner_dispatch_plan_preview_contract::dispatch_plan(&request).unwrap();
    let returned = client
        .preview_runner_dispatch_plan(&conversation_id, &run_id, &dispatch_plan)
        .await
        .expect("visible Runner dispatch-plan preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_runner_dispatch_plan_is_rejected_before_candidate_post() {
    let mut request = runner_dispatch_plan_request();
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
        "Runner dispatch-plan",
    )
    .await
    .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no Runner dispatch-plan request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_runner_dispatch_plan_uses_view_without_pair_reads() {
    let request = runner_dispatch_plan_request();
    let response = runner_dispatch_plan_response(&request);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        Some(&session_view_path()),
        "Runner dispatch-plan",
    )
    .await
    .expect("local visible view should pass the pair guard");
    let (conversation_id, run_id) =
        runner_dispatch_plan_preview_contract::conversation_and_run(&request).unwrap();
    let dispatch_plan = runner_dispatch_plan_preview_contract::dispatch_plan(&request).unwrap();
    let returned = client
        .preview_runner_dispatch_plan(&conversation_id, &run_id, &dispatch_plan)
        .await
        .expect("local-view Runner dispatch-plan preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_run_attempt_preflight_reads_converged_pair_before_candidate_post() {
    let request = runner_dispatch_plan_request();
    let response = run_attempt_lease_dispatch_preflight_response();
    let (client, server) = converged_instance_write(ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview ",
        response_status: "200 OK",
        response: response.clone(),
    });
    ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        None,
        "Run/Attempt/lease preflight",
    )
    .await
    .expect("visible Conversation should pass the pair guard");
    let (conversation_id, run_id) =
        run_attempt_lease_dispatch_preflight_contract::conversation_and_run(&request)
            .expect("preflight request binding");
    let returned = client
        .preview_run_attempt_lease_dispatch_preflight(&conversation_id, &run_id, &request)
        .await
        .expect("visible Run/Attempt/lease preflight");
    run_attempt_lease_dispatch_preflight_contract::validate_response(
        &returned,
        &request,
        &conversation_id,
        &run_id,
    )
    .expect("strict preflight response");
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_run_attempt_preflight_is_rejected_before_candidate_post() {
    let mut request = runner_dispatch_plan_request();
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
        "Run/Attempt/lease preflight",
    )
    .await
    .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no Run/Attempt/lease preflight request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_run_attempt_preflight_uses_view_without_pair_reads() {
    let request = runner_dispatch_plan_request();
    let response = run_attempt_lease_dispatch_preflight_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    ensure_runner_metadata_instance_projection(
        &client,
        &request,
        Some("client-web-001"),
        Some(&session_view_path()),
        "Run/Attempt/lease preflight",
    )
    .await
    .expect("local visible view should pass the pair guard");
    let (conversation_id, run_id) =
        run_attempt_lease_dispatch_preflight_contract::conversation_and_run(&request)
            .expect("preflight request binding");
    let returned = client
        .preview_run_attempt_lease_dispatch_preflight(&conversation_id, &run_id, &request)
        .await
        .expect("local-view Run/Attempt/lease preflight");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}
