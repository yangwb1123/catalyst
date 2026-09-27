use forge_runtime_domain::execution::runner_command::RunnerCommand;
use serde_json::{Value, json};

use super::*;

fn request() -> Value {
    json!({
        "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted","evaluated_at_ms":300,
        "command": {"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":1,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task","--prompt-ref","prompt-1"],"timeout_ms":5000,"max_output_bytes":65536}
    })
}

fn response(request: &Value) -> Value {
    let command: RunnerCommand = serde_json::from_value(request["command"].clone()).unwrap();
    let digest = command.command_sha256().unwrap();
    json!({
        "schema_version":"forge.runner-dispatch-admission/v1",
        "evaluation_mode":"durable_lease_bound_dispatch_admission_preview",
        "owner":request["owner"].clone(), "conversation_id":"conversation-1", "run_id":"run-1", "attempt_id":"attempt-1",
        "attempt_state":"accepted", "attempt_state_admissible":true, "command_id":"command-1", "command_sha256":digest,
        "target_id":"runner-a", "lease_epoch":1, "lease_issued_at_ms":100, "lease_expires_at_ms":1000, "evaluated_at_ms":300,
        "lease_proof_current":true, "lease_active":true, "command_binding_valid":true, "admission_ready":true,
        "rejection_reasons":[], "preview_only":true,
        "authority":{"device_identity_verified":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}
    })
}

#[tokio::test]
async fn runner_dispatch_admission_preview_posts_the_bound_request_once() {
    let request = request();
    let response = response(&request);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_runner_dispatch_admission("conversation-1", "run-1", &request)
        .await
        .unwrap();
    super::super::runner_dispatch_admission::validate_response(
        &returned,
        &request,
        "conversation-1",
        "run-1",
    )
    .unwrap();
    assert_eq!(returned["admission_ready"], true);
    assert!(!returned.to_string().contains("token-a"));
    assert!(!returned.to_string().contains("forge-task"));
    server.join().unwrap();
}

#[tokio::test]
async fn runner_dispatch_admission_preview_does_not_retry_a_401() {
    let request = request();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "401 Unauthorized",
        response: json!({"code":"invalid_token"}),
    }]);
    let error = client
        .preview_runner_dispatch_admission("conversation-1", "run-1", &request)
        .await
        .expect_err("401 must fail without a retry");
    assert!(error.to_string().contains("HTTP 401"), "{error}");
    server.join().unwrap();
}

#[tokio::test]
async fn runner_dispatch_admission_preview_rejects_request_url_drift_before_post() {
    let request = request();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_runner_dispatch_admission("conversation-foreign", "run-1", &request)
        .await
        .expect_err("a request bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Runner dispatch admission request does not match its URL"
    );
}

#[tokio::test]
async fn runner_dispatch_admission_preview_rejects_response_binding_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["run_id"] = json!("run-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_dispatch_admission("conversation-1", "run-1", &request)
        .await
        .expect_err("a foreign admission response must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner dispatch admission"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn runner_dispatch_admission_preview_rejects_authority_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["authority"]["execution_authorized"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-dispatch-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_dispatch_admission("conversation-1", "run-1", &request)
        .await
        .expect_err("authority-bearing admission must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner dispatch admission"
    );
    server.join().unwrap();
}

#[test]
fn runner_dispatch_admission_preview_rejects_binding_or_authority_drift() {
    let request = request();
    let response = response(&request);
    let mut foreign = response.clone();
    foreign["run_id"] = json!("run-2");
    assert!(
        super::super::runner_dispatch_admission::validate_response(
            &foreign,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
    let mut authority = response.clone();
    authority["authority"]["execution_authorized"] = json!(true);
    assert!(
        super::super::runner_dispatch_admission::validate_response(
            &authority,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
    let mut digest = response;
    digest["command_sha256"] = json!("b".repeat(64));
    assert!(
        super::super::runner_dispatch_admission::validate_response(
            &digest,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
}
