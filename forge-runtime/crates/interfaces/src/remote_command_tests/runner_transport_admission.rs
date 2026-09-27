use forge_runtime_domain::execution::runner_command::RunnerCommand;
use serde_json::{Value, json};

use super::*;

fn request() -> Value {
    json!({
        "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted","evaluated_at_ms":300,
        "command": {"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":1,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task","--prompt-ref","prompt-1"],"timeout_ms":5000,"max_output_bytes":65536},
        "lease":{"target_id":"runner-a","epoch":1,"issued_at_ms":100,"expires_at_ms":10100,"current":true,"active":true},
        "transport":{"schema_version":"forge.runner-transport-admission/v1","evaluation_mode":"pure_runner_transport_admission","method":"POST","path":"/api/v1/runners/runner-a/dispatch","timestamp":1700000000,"nonce":"transport-admission-1","payload_sha256":"6ad570170fca79ef62dad30b16f37314ea65f13895d4e0ea873b6512d1c99333","payload_bytes":74,"replay_checked":true,"preview_only":true,"authority":{"identity_verified":false,"heartbeat_accepted":false,"lease_issued":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}},
        "expected_payload_sha256":"6ad570170fca79ef62dad30b16f37314ea65f13895d4e0ea873b6512d1c99333"
    })
}

fn response(request: &Value) -> Value {
    let command: RunnerCommand = serde_json::from_value(request["command"].clone()).unwrap();
    json!({
        "schema_version":"forge.runner-transport-admission/v1",
        "evaluation_mode":"fenced_runner_transport_admission_preview",
        "owner":request["owner"].clone(), "conversation_id":"conversation-1", "run_id":"run-1", "attempt_id":"attempt-1",
        "attempt_state":"accepted", "attempt_state_admissible":true, "command_id":"command-1", "command_sha256":command.command_sha256().unwrap(),
        "target_id":"runner-a", "lease_epoch":1, "lease_issued_at_ms":100, "lease_expires_at_ms":10100, "evaluated_at_ms":300,
        "transport_method":"POST", "transport_path":"/api/v1/runners/runner-a/dispatch", "transport_timestamp":1700000000, "transport_nonce":"transport-admission-1",
        "transport_payload_sha256":"6ad570170fca79ef62dad30b16f37314ea65f13895d4e0ea873b6512d1c99333", "transport_payload_bytes":74, "transport_replay_checked":true,
        "lease_proof_current":true, "lease_active":true, "command_binding_valid":true, "transport_binding_valid":true, "admission_ready":true,
        "rejection_reasons":[], "preview_only":true,
        "authority":{"device_identity_verified":false,"transport_authenticated":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}
    })
}

#[tokio::test]
async fn runner_transport_admission_preview_posts_the_bound_request_once() {
    let request = request();
    let response = response(&request);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-transport-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_runner_transport_admission("conversation-1", "run-1", &request)
        .await
        .unwrap();
    super::super::runner_transport_admission::validate_response(
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
async fn runner_transport_admission_preview_rejects_request_url_drift_before_post() {
    let request = request();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_runner_transport_admission("conversation-foreign", "run-1", &request)
        .await
        .expect_err("a request bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Runner transport admission request does not match its URL"
    );
}

#[tokio::test]
async fn runner_transport_admission_preview_rejects_response_binding_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["run_id"] = json!("run-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-transport-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_transport_admission("conversation-1", "run-1", &request)
        .await
        .expect_err("a foreign transport admission must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner transport admission"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn runner_transport_admission_preview_rejects_authority_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["authority"]["transport_authenticated"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-transport-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_transport_admission("conversation-1", "run-1", &request)
        .await
        .expect_err("authority-bearing transport admission must not escape the client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner transport admission"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn runner_transport_admission_preview_rejects_path_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["transport_path"] = json!("/api/v1/runners/runner-other/dispatch");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/runner-transport-admission/preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_runner_transport_admission("conversation-1", "run-1", &request)
        .await
        .expect_err("a drifted transport path must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Runner transport admission"
    );
    server.join().unwrap();
}

#[test]
fn runner_transport_admission_preview_rejects_authority_or_path_drift() {
    let request = request();
    let response = response(&request);
    let mut authority = response.clone();
    authority["authority"]["transport_authenticated"] = json!(true);
    assert!(
        super::super::runner_transport_admission::validate_response(
            &authority,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
    let mut path = response;
    path["transport_path"] = json!("/api/v1/runners/runner-other/dispatch");
    assert!(
        super::super::runner_transport_admission::validate_response(
            &path,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
}

#[test]
fn canonical_transport_admission_fixture_is_strict_and_display_only() {
    let fixture = include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-transport-admission-v1.json"
    );
    let value: Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        value["schema_version"],
        "forge.runner-transport-admission/v1"
    );
    assert_eq!(value["preview_only"], true);
    assert_eq!(value["authority"]["transport_authenticated"], false);
    crate::device_json_unique::reject_duplicate_keys(fixture.as_bytes()).unwrap();
}
