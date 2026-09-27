use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::{Client, Url, redirect::Policy};
use serde_json::{Value, json};

use super::super::RemoteClient;
use super::{
    RemoteCommand, ensure_runner_admission_instance_projection,
    ensure_runner_metadata_instance_projection, execute_changes_command,
    execute_execution_consent_preview, execute_pending_run_intent_command, execute_prompt_command,
    execute_run_command, execute_scheduler_selection_lease,
    execute_scheduler_selection_lease_release, execute_scheduler_selection_lease_renewal,
    execute_scheduler_selection_preview, execute_session_command,
};
use crate::args::RemoteConversationScope;

#[path = "remote_command_tests/runner_execution_boundary_projection.rs"]
mod runner_execution_boundary_projection;

struct ExpectedRequest {
    request_prefix: &'static str,
    response_status: &'static str,
    response: Value,
}

fn session_view() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .expect("client-instance session view fixture")
}

fn resource_view() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .expect("client-instance resource view fixture")
}

fn session_view_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json")
        .display()
        .to_string()
}

fn resource_view_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json")
        .display()
        .to_string()
}

fn convergence_view_path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(
            "../../../docs/contracts/fixtures/forge-client-instance-session-resource-convergence-v1.json",
        )
        .display()
        .to_string()
}

fn inventory_resource_convergence() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("device inventory/resource convergence fixture")
}

fn prompt_receipt_test_token() -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&json!({
            "iss": "https://id.example",
            "sub": "user-1",
            "tenant_id": "tenant-1",
            "exp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_secs() + 3600,
            "aud": ["forge-api"],
            "scopes": ["forge:conversations:read", "forge:conversations:write"],
        }))
        .expect("encode Prompt receipt token claims"),
    );
    format!("{header}.{payload}.signature")
}

fn execution_consent_response() -> Value {
    json!({
        "conversation_id": "conversation-001",
        "project_id": "project-1",
        "profile_id": "profile-reviewed-v1",
        "profile_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "maximum_ttl_ms": 2_592_000_000_u64,
    })
}

fn runner_execution_intent_request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json"
    ))
    .expect("Runner execution-intent request fixture")
}

fn runner_execution_intent_response() -> Value {
    let request: forge_runtime_domain::execution::runner_execution_intent::RunnerExecutionIntentRequest =
        serde_json::from_value(runner_execution_intent_request())
            .expect("Runner execution-intent request");
    serde_json::to_value(
        forge_runtime_domain::execution::runner_execution_intent::observe_runner_execution_intent(
            request,
        )
        .expect("Runner execution-intent observation"),
    )
    .expect("Runner execution-intent response")
}

fn runner_dispatch_plan_request() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-request-v1.json"
    ))
    .expect("Runner dispatch-plan request fixture")
}

fn run_attempt_lease_dispatch_preflight_response() -> Value {
    serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
    ))
    .expect("Run/Attempt/lease preflight response fixture")
}

fn runner_dispatch_plan_response(request: &Value) -> Value {
    let mut response: Value = serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
    ))
    .expect("Runner dispatch-plan response fixture");
    response["command_sha256"] =
        request["dispatch_plan"]["runner_execution_intent"]["command_sha256"].clone();
    response
}

fn scheduler_selection_request() -> Value {
    json!({
        "conversation_id": "conversation-001",
        "run_id": "run-001",
        "attempt_id": "attempt-001",
        "requirements": {
            "os": "linux",
            "architecture": "amd64",
            "min_cpu_cores": 1,
            "min_memory_bytes": 1,
            "min_storage_bytes": 1,
            "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"],
            "minimum_trust_zone": "standard",
            "sandbox_floor": "container",
            "concurrency_slots": 1
        }
    })
}

fn scheduler_selection_response() -> Value {
    json!({
        "schema_version": "forge.scheduler-selection-preview/v1",
        "evaluation_mode": "pure_scheduler_selection_preview",
        "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id": "conversation-001",
        "run_id": "run-001",
        "attempt_id": "attempt-001",
        "evaluated_at_ms": 1800000000000_i64,
        "candidate_count": 1,
        "eligible_candidate_count": 0,
        "selection_available": false,
        "selection_reason": "no_eligible_candidate",
        "selected_device_id": null,
        "selected_instance_id": null,
        "preview_only": true,
        "authority": {
            "placement_selected": false,
            "reservation_created": false,
            "lease_issued": false,
            "execution_authorized": false,
            "dispatch_performed": false,
            "audit_published": false
        }
    })
}

fn scheduler_lease_request(conversation_id: &str) -> Value {
    json!({
        "conversation_id": conversation_id,
        "run_id": "run-001",
        "attempt_id": "attempt-001",
        "requirements": {
            "os": "linux",
            "architecture": "amd64",
            "min_cpu_cores": 1,
            "min_memory_bytes": 1,
            "min_storage_bytes": 1,
            "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"],
            "minimum_trust_zone": "standard",
            "sandbox_floor": "container",
            "concurrency_slots": 1
        },
        "ttl_ms": 30000
    })
}

fn scheduler_lease_renewal_request(conversation_id: &str) -> Value {
    json!({
        "conversation_id": conversation_id,
        "run_id": "run-001",
        "attempt_id": "attempt-001",
        "target_id": "runner-a",
        "epoch": 1,
        "fencing_token": "fence-token-a",
        "ttl_ms": 30000
    })
}

fn scheduler_lease_response() -> Value {
    let mut response: Value = serde_json::from_str(include_str!(
        "../../../../docs/contracts/fixtures/forge-execution-lease-registry-v1.json"
    ))
    .expect("scheduler lease response fixture");
    response["conversation_id"] = json!("conversation-001");
    response["run_id"] = json!("run-001");
    response["attempt_id"] = json!("attempt-001");
    response["grant"]["attempt_id"] = json!("attempt-001");
    response
}

fn scheduler_lease_release_request() -> Value {
    json!({
        "conversation_id": "conversation-001",
        "run_id": "run-001",
        "attempt_id": "attempt-001",
        "target_id": "runner-a",
        "epoch": 2,
        "fencing_token": "fence-token-b"
    })
}

fn scheduler_lease_release_response() -> Value {
    json!({
        "schema_version": "forge.execution-lease-registry/v1",
        "evaluation_mode": "durable_scheduler_lease_release",
        "owner": {
            "issuer": "https://id.example",
            "subject": "user-a",
            "tenant_id": "tenant-a"
        },
        "conversation_id": "conversation-001",
        "run_id": "run-001",
        "attempt_id": "attempt-001",
        "device_id": "device-a",
        "instance_id": "runner-a",
        "epoch": 2,
        "released_at_ms": 1800000040000_u64,
        "replayed": false,
        "authority": {
            "placement_selected": false,
            "reservation_created": false,
            "lease_issued": false,
            "execution_authorized": false,
            "dispatch_performed": false,
            "audit_published": false
        }
    })
}

fn spawn_mock_server(responses: Vec<ExpectedRequest>) -> (RemoteClient, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let address = listener.local_addr().expect("mock server address");
    let server = thread::spawn(move || {
        for expected in responses {
            let (mut stream, _) = listener.accept().expect("accept mock request");
            let request = capture_request(&mut stream);
            assert!(
                request.starts_with(expected.request_prefix),
                "request={request:?} expected prefix={:?}",
                expected.request_prefix
            );
            write_json_response(&mut stream, expected.response_status, &expected.response);
        }
    });
    let http = Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(2))
        .build()
        .expect("mock HTTP client");
    let base_url = Url::parse(&format!("http://{address}")).expect("mock server URL");
    let client = RemoteClient {
        http,
        base_url,
        access_token: "test-token".into(),
        change_cursor: None,
        token_refresh: None,
    };
    (client, server)
}

fn capture_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set mock read timeout");
    let mut reader = BufReader::new(stream.try_clone().expect("clone mock stream"));
    let mut line = String::new();
    reader.read_line(&mut line).expect("read request line");
    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).expect("read request header");
        if header == "\r\n" || header.is_empty() {
            break;
        }
        if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
            content_length = value.trim().parse().expect("content length");
        }
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).expect("read request body");
    line.trim().to_owned()
}

fn write_json_response(stream: &mut TcpStream, status: &str, response: &Value) {
    let body = serde_json::to_vec(response).expect("encode mock response");
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write mock response headers");
    stream.write_all(&body).expect("write mock response body");
}

#[tokio::test]
async fn unfiltered_execution_consent_preview_keeps_the_exact_candidate_read() {
    let response = execution_consent_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/execution-consents ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_execution_consent_preview(&client, "conversation-001", None, None)
        .await
        .expect("unfiltered execution-consent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_execution_consent_preview_reads_converged_pair_before_candidate_get() {
    let response = execution_consent_response();
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
            request_prefix: "GET /api/v1/conversations/conversation-001/execution-consents ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_execution_consent_preview(
        &client,
        "conversation-001",
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance execution-consent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_execution_consent_preview_is_rejected_before_candidate_get() {
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
    let error = execute_execution_consent_preview(
        &client,
        "conversation-002",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no execution-consent request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn drifted_instance_execution_consent_preview_is_rejected_before_candidate_get() {
    let mut drifted_resource = resource_view();
    drifted_resource["instances"][4]["observed_at_ms"] = json!(200501);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let error = execute_execution_consent_preview(
        &client,
        "conversation-001",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("resource drift must block the candidate read");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_execution_consent_preview_uses_view_without_candidate_observations() {
    let response = execution_consent_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/execution-consents ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_execution_consent_preview(
        &client,
        "conversation-001",
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local view should permit visible execution-consent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn unfiltered_scheduler_selection_preview_keeps_the_exact_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let response = scheduler_selection_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        None,
        None,
    )
    .await
    .expect("unfiltered scheduler selection preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_scheduler_selection_preview_reads_inventory_after_pair_before_candidate_post()
 {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let response = scheduler_selection_response();
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
            request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance scheduler selection preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_scheduler_selection_preview_is_rejected_before_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    let mut request = scheduler_selection_request();
    request["conversation_id"] = json!("conversation-002");
    serde_json::to_writer(input.as_file(), &request).expect("write scheduler input");
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
    let error = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no scheduler-selection request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn drifted_instance_scheduler_selection_preview_is_rejected_before_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let mut drifted_resource = resource_view();
    drifted_resource["instances"][4]["observed_at_ms"] = json!(200501);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let error = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("resource drift must block the candidate post");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_scheduler_selection_preview_is_rejected_before_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
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
    let error = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory/resource drift must block the candidate post");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_scheduler_selection_preview_uses_view_without_candidate_observations() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let response = scheduler_selection_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local view should permit visible scheduler selection preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_scheduler_lease_claim_reads_converged_pair_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
    let response = scheduler_lease_response();
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
            request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-instance-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance scheduler lease claim");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_scheduler_lease_renewal_reads_inventory_after_pair_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
    let mut response = scheduler_lease_response();
    response["grant"]["epoch"] = json!(2);
    response["grant"]["fencing_token"] = json!("fence-token-b");
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
            request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-instance-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance scheduler lease renewal");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_visible_instance_scheduler_lease_claim_is_rejected_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
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
    let error = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-drift-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory/resource drift must block the lease claim POST");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_visible_instance_scheduler_lease_renewal_is_rejected_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
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
    let error = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-drift-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory/resource drift must block the lease renewal POST");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_read_failure_blocks_visible_instance_scheduler_lease_claim_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
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
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
    ]);
    let error = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-read-failure-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory read failure must block the lease claim POST");
    assert!(error.to_string().contains("Forge API returned HTTP 503"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_read_failure_blocks_visible_instance_scheduler_lease_renewal_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
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
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
    ]);
    let error = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-read-failure-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory read failure must block the lease renewal POST");
    assert!(error.to_string().contains("Forge API returned HTTP 503"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_view_scheduler_lease_claim_keeps_the_single_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
    let response = scheduler_lease_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-local-view-1",
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local instance view scheduler lease claim");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_view_scheduler_lease_renewal_keeps_the_single_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
    let mut response = scheduler_lease_response();
    response["grant"]["epoch"] = json!(2);
    response["grant"]["fencing_token"] = json!("fence-token-b");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-local-view-1",
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local instance view scheduler lease renewal");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_scheduler_lease_claim_is_rejected_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-002"),
    )
    .expect("write scheduler lease input");
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
    let error = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-hidden-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("hidden Conversation must be rejected before the lease POST");
    assert!(
        error
            .to_string()
            .contains("no scheduler lease request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn unfiltered_scheduler_lease_release_preserves_legacy_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease release input");
    let request = scheduler_lease_release_request();
    serde_json::to_writer(input.as_file(), &request).expect("write scheduler lease release input");
    let response = scheduler_lease_release_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/release ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_lease_release(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-release-legacy-1",
        None,
        None,
    )
    .await
    .expect("unfiltered scheduler lease release");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_prompt_list_is_rejected_before_prompt_request() {
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
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-002".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, None, None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_session_detail_is_rejected_before_conversation_request() {
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
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-002".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_session_command(&client, &command, None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no Conversation request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn online_instance_session_detail_rejects_resource_drift_before_private_read() {
    let mut drifted_resource = resource_view();
    drifted_resource["instances"][4]["observed_at_ms"] = json!(200501);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_session_command(&client, &command, None)
        .await
        .expect_err("resource drift must block the private Conversation read");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_session_detail_reads_after_projection_check() {
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
            request_prefix: "GET /api/v1/conversations/conversation-001 ",
            response_status: "200 OK",
            response: json!({
                "conversation": {
                    "id": "conversation-001",
                    "title": "Visible",
                    "scope": {"kind": "global"},
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                },
                "aggregate_version": 1
            }),
        },
    ]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let detail = execute_session_command(&client, &command, None)
        .await
        .expect("visible Conversation detail");
    assert_eq!(detail["conversation"]["id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn unfiltered_session_create_keeps_one_owner_post() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        response_status: "201 Created",
        response: json!({
            "id": "conversation-created",
            "scope": {"kind": "global"},
            "title": "Created",
            "created_at_ms": 1,
            "updated_at_ms": 1
        }),
    }]);
    let command = RemoteCommand::SessionsCreate {
        title: "Created".into(),
        scope: RemoteConversationScope::Global,
        instance_id: None,
        instance_view: None,
    };
    let created = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect("unfiltered session create");
    assert_eq!(created["id"], "conversation-created");
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_session_create_reads_converged_pair_before_owner_post() {
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
            request_prefix: "POST /api/v1/conversations ",
            response_status: "201 Created",
            response: json!({
                "id": "conversation-created",
                "scope": {"kind": "global"},
                "title": "Created",
                "created_at_ms": 1,
                "updated_at_ms": 1
            }),
        },
    ]);
    let command = RemoteCommand::SessionsCreate {
        title: "Created".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let created = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect("visible instance session create");
    assert_eq!(created["id"], "conversation-created");
    server.join().expect("mock server");
}

#[tokio::test]
async fn drifted_instance_session_create_is_rejected_before_owner_post() {
    let mut drifted_resource = resource_view();
    drifted_resource["instances"][4]["observed_at_ms"] = json!(200501);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let command = RemoteCommand::SessionsCreate {
        title: "Must stay blocked".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect_err("resource drift must block the owner create");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_session_create_uses_view_without_candidate_reads() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        response_status: "201 Created",
        response: json!({
            "id": "conversation-created",
            "scope": {"kind": "global"},
            "title": "Created",
            "created_at_ms": 1,
            "updated_at_ms": 1
        }),
    }]);
    let command = RemoteCommand::SessionsCreate {
        title: "Created".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
    };
    let created = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect("local instance session create");
    assert_eq!(created["id"], "conversation-created");
    server.join().expect("mock server");
}

#[tokio::test]
async fn unknown_instance_session_create_is_rejected_before_owner_post() {
    let (client, server) = spawn_mock_server(Vec::new());
    let command = RemoteCommand::SessionsCreate {
        title: "Must stay blocked".into(),
        scope: RemoteConversationScope::Global,
        instance_id: Some("client-unknown".into()),
        instance_view: Some(convergence_view_path()),
    };
    let error = execute_session_command(&client, &command, Some("create-key"))
        .await
        .expect_err("unknown instance must block the owner create");
    assert!(error.to_string().contains("does not declare instance"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_session_detail_uses_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001 ",
        response_status: "200 OK",
        response: json!({
            "conversation": {
                "id": "conversation-001",
                "title": "Visible",
                    "scope": {"kind": "global"},
                "created_at_ms": 1,
                "updated_at_ms": 1
            },
        "aggregate_version": 1
        }),
    }]);
    let command = RemoteCommand::SessionsShow {
        conversation_id: "conversation-001".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let detail = execute_session_command(&client, &command, None)
        .await
        .expect("local view should permit visible Conversation");
    assert_eq!(detail["conversation"]["id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_convergence_view_filters_session_list_before_returning_rows() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations?limit=128 ",
        response_status: "200 OK",
        response: json!({
            "conversations": [
                {
                    "conversation": {
                        "id": "conversation-001",
                        "title": "Visible",
                        "scope": {"kind": "global"},
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 1
                },
                {
                    "conversation": {
                        "id": "conversation-002",
                        "title": "Hidden",
                        "scope": {"kind": "global"},
                        "created_at_ms": 2,
                        "updated_at_ms": 2
                    },
                    "aggregate_version": 1
                }
            ],
            "next_after_id": null,
            "has_more": false
        }),
    }]);
    let command = RemoteCommand::SessionsList {
        after_id: None,
        scope: None,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
        all_pages: false,
    };
    let page = execute_session_command(&client, &command, None)
        .await
        .expect("paired convergence view should filter session rows locally");
    assert_eq!(
        page["conversations"].as_array().unwrap().len(),
        1,
        "only the selected instance's session should remain"
    );
    assert_eq!(
        page["conversations"][0]["conversation"]["id"],
        "conversation-001"
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_prompt_list_reads_after_projection_check() {
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
            request_prefix: "GET /api/v1/conversations/conversation-001/prompts?",
            response_status: "200 OK",
            response: json!({
                "conversation_id": "conversation-001",
                "prompts": [],
                "has_more": false
            }),
        },
    ]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-001".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let page = execute_prompt_command(&client, &command, None, None)
        .await
        .expect("visible Conversation Prompt list");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_prompt_list_uses_the_supplied_view_without_candidate_request() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/prompts?",
        response_status: "200 OK",
        response: json!({
            "conversation_id": "conversation-001",
            "prompts": [],
            "has_more": false
        }),
    }]);
    let command = RemoteCommand::PromptsList {
        conversation_id: "conversation-001".into(),
        before_created_at_ms: None,
        before_prompt_id: None,
        instance_id: Some("client-web-001".into()),
        instance_view: Some(session_view_path()),
    };
    let page = execute_prompt_command(&client, &command, None, None)
        .await
        .expect("local view should permit visible Conversation");
    assert_eq!(page["conversation_id"], "conversation-001");
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_convergence_view_permits_visible_prompt_append_after_projection_check() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
        response_status: "201 Created",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-001",
                "role": "user",
                "content": "visible prompt from paired view",
                "created_at_ms": 300,
            },
            "aggregate_version": 4,
            "replayed": false
        }),
    }]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "visible prompt from paired view".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
    };
    let receipt = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect("paired convergence view should permit visible Prompt append");
    assert_eq!(receipt["aggregate_version"], 4);
    server.join().expect("mock server");
}

#[tokio::test]
async fn unfiltered_prompt_add_keeps_the_single_prompt_post() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
        response_status: "201 Created",
        response: json!({
            "prompt": {
                "id": "prompt-1",
                "conversation_id": "conversation-001",
                "role": "user",
                "content": "unfiltered prompt",
                "created_at_ms": 300,
            },
            "aggregate_version": 4,
            "replayed": false
        }),
    }]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "unfiltered prompt".into(),
        instance_id: None,
        instance_view: None,
    };
    let receipt = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect("unfiltered Prompt append");
    assert_eq!(receipt["aggregate_version"], 4);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_prompt_append_from_convergence_view_is_rejected_before_post() {
    let (client, server) = spawn_mock_server(Vec::new());
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-002".into(),
        expected_version: 3,
        content: "must remain local to the selected instance".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: Some(convergence_view_path()),
    };
    let error = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect_err("hidden Conversation must be rejected before Prompt POST");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_prompt_add_is_rejected_before_prompt_request() {
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
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-002".into(),
        expected_version: 3,
        content: "must not cross instance projection".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect_err("hidden Conversation must be rejected locally");
    assert!(error.to_string().contains("no Prompt request was sent"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_prompt_add_posts_after_projection_check() {
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
            request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
            response_status: "201 Created",
            response: json!({
                "prompt": {
                    "id": "prompt-1",
                    "conversation_id": "conversation-001",
                    "role": "user",
                    "content": "visible prompt",
                    "created_at_ms": 300,
                },
                "aggregate_version": 4,
                "replayed": false
            }),
        },
    ]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "visible prompt".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let receipt = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect("visible Conversation Prompt append");
    assert_eq!(receipt["aggregate_version"], 4);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_prompt_receipt_reads_inventory_after_projection_check() {
    let (mut client, server) = spawn_mock_server(vec![
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
            request_prefix: "POST /api/v1/conversations/conversation-001/prompts ",
            response_status: "200 OK",
            response: json!({
                "prompt": {
                    "id": "prompt-1",
                    "conversation_id": "conversation-001",
                    "role": "user",
                    "content": "visible receipt prompt",
                    "created_at_ms": 300,
                },
                "aggregate_version": 4,
                "replayed": false
            }),
        },
    ]);
    client.access_token = prompt_receipt_test_token();
    let command = RemoteCommand::PromptsReceipt {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "visible receipt prompt".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let receipt = execute_prompt_command(&client, &command, Some("receipt-key"), None)
        .await
        .expect("visible Prompt receipt append");
    assert_eq!(receipt["receipt"]["aggregate_version"], 4);
    assert_eq!(receipt["receipt"]["content_included"], false);
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_visible_instance_prompt_add_is_rejected_before_post() {
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
            response: {
                let mut value = inventory_resource_convergence()["resource_view"].clone();
                value["devices"][0]["heartbeat_sequence"] = json!(99);
                value
            },
        },
    ]);
    let command = RemoteCommand::PromptsAdd {
        conversation_id: "conversation-001".into(),
        expected_version: 3,
        content: "inventory drift must block this Prompt".into(),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let error = execute_prompt_command(&client, &command, Some("prompt-key"), None)
        .await
        .expect_err("inventory/resource drift must block the Prompt POST");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

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
            response: json!({
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
            }),
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
        super::super::runner_execution_intent::conversation_and_run(&request).unwrap();
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
        super::super::runner_execution_intent::conversation_and_run(&request).unwrap();
    let returned = client
        .preview_runner_execution_intent(&conversation_id, &run_id, &request)
        .await
        .expect("local-view Runner execution-intent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_runner_dispatch_plan_reads_converged_pair_before_candidate_post() {
    let request = runner_dispatch_plan_request();
    let response = runner_dispatch_plan_response(&request);
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
            request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
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
        super::super::runner_dispatch_plan_preview::conversation_and_run(&request).unwrap();
    let dispatch_plan =
        super::super::runner_dispatch_plan_preview::dispatch_plan(&request).unwrap();
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
        super::super::runner_dispatch_plan_preview::conversation_and_run(&request).unwrap();
    let dispatch_plan =
        super::super::runner_dispatch_plan_preview::dispatch_plan(&request).unwrap();
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
            request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
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
        super::super::run_attempt_lease_dispatch_preflight::conversation_and_run(&request)
            .expect("preflight request binding");
    let returned = client
        .preview_run_attempt_lease_dispatch_preflight(&conversation_id, &run_id, &request)
        .await
        .expect("visible Run/Attempt/lease preflight");
    super::super::run_attempt_lease_dispatch_preflight::validate_response(
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
        super::super::run_attempt_lease_dispatch_preflight::conversation_and_run(&request)
            .expect("preflight request binding");
    let returned = client
        .preview_run_attempt_lease_dispatch_preflight(&conversation_id, &run_id, &request)
        .await
        .expect("local-view Run/Attempt/lease preflight");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_change_list_reads_converged_pair_and_filters_rows() {
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
            request_prefix: "GET /api/v1/conversation-changes?after_cursor=4&limit=128 ",
            response_status: "200 OK",
            response: json!({
                "after_cursor": 4,
                "scanned_through_cursor": 6,
                "has_more": false,
                "changes": [
                    {
                        "cursor": 5,
                        "schema_version": 1,
                        "conversation_id": "conversation-001",
                        "entity_id": "prompt-001",
                        "aggregate_version": 2,
                        "kind": "prompt_appended",
                        "created_at_ms": 20
                    },
                    {
                        "cursor": 6,
                        "schema_version": 1,
                        "conversation_id": "conversation-002",
                        "entity_id": "prompt-002",
                        "aggregate_version": 2,
                        "kind": "prompt_appended",
                        "created_at_ms": 21
                    }
                ]
            }),
        },
    ]);
    let command = RemoteCommand::ChangesList {
        after_cursor: Some(4),
        instance_id: Some("client-web-001".into()),
        instance_view: None,
    };
    let returned = execute_changes_command(&client, &command)
        .await
        .expect("visible instance change list");
    assert_eq!(returned["scanned_through_cursor"], 6);
    assert_eq!(returned["changes"].as_array().unwrap().len(), 1);
    assert_eq!(
        returned["changes"][0]["conversation_id"],
        "conversation-001"
    );
    server.join().expect("mock server");
}

#[test]
fn instance_change_projection_hides_rows_without_rewriting_owner_cursor() {
    let scope = crate::client_instance_session_scope::ClientInstanceSessionScope {
        instance_id: "client-web-001".into(),
        client_kind: "web".into(),
        session_ids: ["conversation-001".to_owned()].into_iter().collect(),
    };
    let response = super::project_change_feed_response(
        json!({
            "start_cursor": 4,
            "scanned_through_cursor": 6,
            "has_more": true,
            "changes": [
                {"cursor": 5, "conversation_id": "conversation-001"},
                {"cursor": 6, "conversation_id": "conversation-002"}
            ]
        }),
        Some(&scope),
    )
    .expect("project change response");
    assert_eq!(response["scanned_through_cursor"], 6);
    assert_eq!(response["has_more"], true);
    assert_eq!(response["changes"].as_array().unwrap().len(), 1);
    assert_eq!(response["changes"][0]["cursor"], 5);
}
