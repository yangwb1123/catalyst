use super::*;

#[tokio::test]
async fn runner_dispatch_plan_preview_posts_the_bound_plan_once() {
    let request = super::super::run_attempt_lease_dispatch_preflight::test_request();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
    ))
    .unwrap();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview ",
        required_headers: &[],
        body_fields: request["dispatch_plan"].clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_runner_dispatch_plan("conversation-001", "run-001", &request["dispatch_plan"])
        .await
        .unwrap();
    super::super::runner_dispatch_plan_preview::validate_response(
        &returned,
        &request,
        "conversation-001",
        "run-001",
    )
    .unwrap();
    server.join().unwrap();
}

#[tokio::test]
async fn runner_dispatch_plan_preview_does_not_retry_a_401() {
    let request = super::super::run_attempt_lease_dispatch_preflight::test_request();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview ",
        required_headers: &[],
        body_fields: request["dispatch_plan"].clone(),
        response_status: "401 Unauthorized",
        response: json!({"code":"invalid_token"}),
    }]);
    let error = client
        .preview_runner_dispatch_plan("conversation-001", "run-001", &request["dispatch_plan"])
        .await
        .expect_err("401 must fail without a retry");
    assert!(error.to_string().contains("HTTP 401"), "{error}");
    server.join().unwrap();
}

#[test]
fn runner_dispatch_plan_preview_rejects_binding_and_authority_drift() {
    let request = super::super::run_attempt_lease_dispatch_preflight::test_request();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
    ))
    .unwrap();
    let mut foreign = response.clone();
    foreign["run_id"] = json!("run-002");
    assert!(
        super::super::runner_dispatch_plan_preview::validate_response(
            &foreign,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
    let mut selected = response.clone();
    selected["selected_target_id"] = json!("runner-1");
    assert!(
        super::super::runner_dispatch_plan_preview::validate_response(
            &selected,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
    let mut authority = response;
    authority["authority"]["dispatch_performed"] = json!(true);
    assert!(
        super::super::runner_dispatch_plan_preview::validate_response(
            &authority,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
}
