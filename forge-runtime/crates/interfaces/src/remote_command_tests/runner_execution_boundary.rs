use forge_runtime_domain::execution::runner_command::RunnerCommand;
use serde_json::{Value, json};

use super::*;

fn request() -> Value {
    json!({
        "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted",
        "command": {"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":3,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task","--prompt-ref","prompt-1"],"timeout_ms":5000,"max_output_bytes":65536},
        "transport":{"schema_version":"forge.runner-transport-admission/v1","evaluation_mode":"pure_runner_transport_admission","method":"POST","path":"/api/v1/runners/runner-a/dispatch","timestamp":300,"nonce":"nonce-a","payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","payload_bytes":256,"replay_checked":true,"preview_only":true,"authority":{"identity_verified":false,"heartbeat_accepted":false,"lease_issued":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}},
        "expected_payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "controls":{"effect_state":"not_started","cancellation_requested":false}
    })
}

fn response(request: &Value) -> Value {
    let command: RunnerCommand = serde_json::from_value(request["command"].clone()).unwrap();
    json!({
        "schema_version":"forge.runner-execution-boundary/v1","evaluation_mode":"p4_runner_authority_execution_boundary_preview","mode":"execute",
        "owner":request["owner"].clone(),"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted","command_id":"command-1","command_sha256":command.command_sha256().unwrap(),"target_id":"runner-a","lease_epoch":3,
        "activation_allowed":true,"runner_authority_accepted":true,"dispatch_admission_ready":true,"transport_admission_ready":true,"effect_state":"not_started","effect_state_startable":true,"cancellation_clear":true,"execution_boundary_ready":true,"rejection_reasons":[],"preview_only":true,
        "authority":{"device_identity_verified":false,"command_persisted":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}
    })
}

#[tokio::test]
async fn runner_execution_boundary_preview_posts_server_owned_request_once() {
    let request = request();
    let response = response(&request);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_runner_execution_boundary("conversation-1", "run-1", &request)
        .await
        .unwrap();
    super::super::runner_execution_boundary::validate_response(
        &returned,
        &request,
        "conversation-1",
        "run-1",
    )
    .unwrap();
    assert_eq!(returned["execution_boundary_ready"], true);
    assert!(!returned.to_string().contains("token-a"));
    assert!(!returned.to_string().contains("forge-task"));
    server.join().unwrap();
}

#[tokio::test]
async fn runner_execution_boundary_preview_rejects_request_url_drift_before_post() {
    let request = request();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_runner_execution_boundary("conversation-foreign", "run-1", &request)
        .await
        .expect_err("a request bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Runner execution boundary request does not match its URL"
    );
}

#[tokio::test]
async fn runner_execution_boundary_preview_rejects_response_binding_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["target_id"] = json!("runner-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_execution_boundary("conversation-1", "run-1", &request)
        .await
        .expect_err("a foreign execution boundary must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner execution boundary"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn runner_execution_boundary_preview_rejects_readiness_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["execution_boundary_ready"] = Value::Bool(false);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_execution_boundary("conversation-1", "run-1", &request)
        .await
        .expect_err("a drifted readiness result must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner execution boundary"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn runner_execution_boundary_preview_rejects_authority_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["authority"]["execution_authorized"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_execution_boundary("conversation-1", "run-1", &request)
        .await
        .expect_err("authority-bearing execution boundary must not escape the client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner execution boundary"
    );
    server.join().unwrap();
}

#[test]
fn runner_execution_boundary_response_rejects_authority_or_readiness_drift() {
    let request = request();
    let response = response(&request);
    let mut authority = response.clone();
    authority["authority"]["execution_authorized"] = json!(true);
    assert!(
        super::super::runner_execution_boundary::validate_response(
            &authority,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
    let mut readiness = response;
    readiness["execution_boundary_ready"] = json!(false);
    assert!(
        super::super::runner_execution_boundary::validate_response(
            &readiness,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
}
