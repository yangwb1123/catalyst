use serde_json::Value;

use super::*;

fn request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json"
    ))
    .unwrap()
}

fn response() -> Value {
    let request: forge_runtime_domain::execution::runner_execution_intent::RunnerExecutionIntentRequest =
        serde_json::from_value(request()).unwrap();
    serde_json::to_value(
        forge_runtime_domain::execution::runner_execution_intent::observe_runner_execution_intent(
            request,
        )
        .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn runner_execution_intent_preview_posts_the_full_bound_request_once() {
    let request = request();
    let response = response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_runner_execution_intent("conversation-001", "run-001", &request)
        .await
        .unwrap();
    super::super::runner_execution_intent::validate_response(
        &returned,
        &request,
        "conversation-001",
        "run-001",
    )
    .unwrap();
    server.join().unwrap();
}

#[tokio::test]
async fn runner_execution_intent_preview_rejects_request_url_drift_before_post() {
    let request = request();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_runner_execution_intent("conversation-foreign", "run-001", &request)
        .await
        .expect_err("a request bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Runner execution-intent preview request does not match its URL"
    );
}

#[tokio::test]
async fn runner_execution_intent_preview_rejects_response_binding_drift_at_client_boundary() {
    let request = request();
    let mut forged = response();
    forged["target_id"] = Value::String("runner-foreign".into());
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_execution_intent("conversation-001", "run-001", &request)
        .await
        .expect_err("a foreign execution intent must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner execution-intent preview"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn runner_execution_intent_preview_rejects_authority_drift_at_client_boundary() {
    let request = request();
    let mut forged = response();
    forged["authority"]["execution_authorized"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_execution_intent("conversation-001", "run-001", &request)
        .await
        .expect_err("authority-bearing execution intent must not escape the client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner execution-intent preview"
    );
    server.join().unwrap();
}

#[test]
fn runner_execution_intent_preview_rejects_authority_or_binding_drift() {
    let request = request();
    let response = response();
    let mut authority = response.clone();
    authority["authority"]["execution_authorized"] = Value::Bool(true);
    assert!(
        super::super::runner_execution_intent::validate_response(
            &authority,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
    let mut foreign = response;
    foreign["target_id"] = Value::String("runner-foreign".into());
    assert!(
        super::super::runner_execution_intent::validate_response(
            &foreign,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut foreign_run = request;
    foreign_run["run_reference"]["run_id"] = Value::String("run-foreign".into());
    let decoded = serde_json::from_value::<
        forge_runtime_domain::execution::runner_execution_intent::RunnerExecutionIntentRequest,
    >(foreign_run)
    .map_err(|_| ())
    .and_then(|request| {
        forge_runtime_domain::execution::runner_execution_intent::observe_runner_execution_intent(
            request,
        )
        .map(|_| ())
        .map_err(|_| ())
    });
    assert!(decoded.is_err());
}
