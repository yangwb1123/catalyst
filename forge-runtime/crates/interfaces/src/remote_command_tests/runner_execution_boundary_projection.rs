use forge_runtime_domain::execution::runner_command::RunnerCommand;
use serde_json::{Value, json};

use super::*;

fn request() -> Value {
    json!({
        "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id":"conversation-001","run_id":"run-001","attempt_id":"attempt-1","attempt_state":"accepted",
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
        "owner":request["owner"].clone(),"conversation_id":"conversation-001","run_id":"run-001","attempt_id":"attempt-1","attempt_state":"accepted","command_id":"command-1","command_sha256":command.command_sha256().unwrap(),"target_id":"runner-a","lease_epoch":3,
        "activation_allowed":true,"runner_authority_accepted":true,"dispatch_admission_ready":true,"transport_admission_ready":true,"effect_state":"not_started","effect_state_startable":true,"cancellation_clear":true,"execution_boundary_ready":true,"rejection_reasons":[],"preview_only":true,
        "authority":{"device_identity_verified":false,"command_persisted":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}
    })
}

#[tokio::test]
async fn visible_instance_execution_boundary_refreshes_resource_before_post() {
    let request = request();
    let response = response(&request);
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
            request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-boundary/preview ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    ensure_runner_admission_instance_projection(
        &client,
        "conversation-001",
        "runner-a",
        Some("client-web-001"),
        None,
        "Runner execution boundary",
    )
    .await
    .expect("matching Runner target must pass the resource gate");
    let returned = client
        .preview_runner_execution_boundary("conversation-001", "run-001", &request)
        .await
        .expect("execution boundary candidate");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn foreign_instance_execution_boundary_stops_before_candidate_post() {
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
        "Runner execution boundary",
    )
    .await
    .expect_err("foreign target must fail before execution-boundary POST");
    assert!(
        error
            .to_string()
            .contains("no Runner execution boundary request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_session_only_execution_boundary_view_stops_before_candidate_post() {
    let (client, server) = spawn_mock_server(vec![]);
    let error = ensure_runner_admission_instance_projection(
        &client,
        "conversation-001",
        "runner-a",
        Some("client-web-001"),
        Some(&session_view_path()),
        "Runner execution boundary",
    )
    .await
    .expect_err("session-only local view cannot bind an execution target");
    assert!(
        error
            .to_string()
            .contains("requires a resource or converged view")
    );
    server.join().expect("mock server");
}
