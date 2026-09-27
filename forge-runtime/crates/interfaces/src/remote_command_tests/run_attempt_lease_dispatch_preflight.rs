use super::*;

use serde_json::{Value, json};

fn request() -> Value {
    super::super::run_attempt_lease_dispatch_preflight::test_request()
}

fn response() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
    ))
    .expect("Run/Attempt/lease preflight fixture")
}

#[tokio::test]
async fn run_attempt_lease_dispatch_preflight_posts_exact_bound_request_once() {
    let request = request();
    let expected_response = response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview ",
        required_headers: &[],
        body_fields: json!({
            "owner": request["owner"].clone(),
            "conversation_id": "conversation-001",
            "run_id": "run-001",
            "run_status": "nonterminal",
            "dispatch_plan": request["dispatch_plan"].clone(),
        }),
        response_status: "200 OK",
        response: expected_response.clone(),
    }]);
    let returned = client
        .preview_run_attempt_lease_dispatch_preflight("conversation-001", "run-001", &request)
        .await
        .expect("preflight response");
    super::super::run_attempt_lease_dispatch_preflight::validate_response(
        &returned,
        &request,
        "conversation-001",
        "run-001",
    )
    .expect("strict preflight response");
    server.join().unwrap();
}

#[tokio::test]
async fn run_attempt_lease_dispatch_preflight_does_not_retry_server_failure() {
    let request = request();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview ",
        required_headers: &[],
        body_fields: json!({
            "conversation_id": "conversation-001",
            "run_id": "run-001",
        }),
        response_status: "500 Internal Server Error",
        response: json!({"code":"candidate_failed"}),
    }]);
    let error = client
        .preview_run_attempt_lease_dispatch_preflight("conversation-001", "run-001", &request)
        .await
        .expect_err("server failure");
    assert!(error.to_string().contains("HTTP 500"));
    server.join().unwrap();
}

#[tokio::test]
async fn run_attempt_lease_dispatch_preflight_rejects_request_url_drift_before_post() {
    let request = request();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_run_attempt_lease_dispatch_preflight("conversation-foreign", "run-001", &request)
        .await
        .expect_err("a request bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "remote Run/Attempt/lease preflight request does not match the URL path"
    );
}

#[tokio::test]
async fn run_attempt_lease_dispatch_preflight_rejects_response_binding_drift_at_client_boundary() {
    let request = request();
    let mut forged = response();
    forged["command_id"] = json!("command-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview ",
        required_headers: &[],
        body_fields: json!({
            "owner": request["owner"].clone(),
            "conversation_id": "conversation-001",
            "run_id": "run-001",
            "run_status": "nonterminal",
            "dispatch_plan": request["dispatch_plan"].clone(),
        }),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_run_attempt_lease_dispatch_preflight("conversation-001", "run-001", &request)
        .await
        .expect_err("a foreign preflight response must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned a Run/Attempt/lease preflight with a different binding"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn run_attempt_lease_dispatch_preflight_rejects_authority_drift_at_client_boundary() {
    let request = request();
    let mut forged = response();
    forged["authority"]["dispatch_performed"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview ",
        required_headers: &[],
        body_fields: json!({
            "owner": request["owner"].clone(),
            "conversation_id": "conversation-001",
            "run_id": "run-001",
            "run_status": "nonterminal",
            "dispatch_plan": request["dispatch_plan"].clone(),
        }),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_run_attempt_lease_dispatch_preflight("conversation-001", "run-001", &request)
        .await
        .expect_err("authority-bearing preflight must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Run/Attempt/lease preflight"
    );
    server.join().unwrap();
}

#[test]
fn run_attempt_lease_dispatch_preflight_response_rejects_request_binding_drift() {
    let request = request();
    let mut response = response();
    response["command_id"] = json!("foreign-command");
    assert!(
        super::super::run_attempt_lease_dispatch_preflight::validate_response(
            &response,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
}
