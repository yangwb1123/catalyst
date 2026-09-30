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

#[path = "remote_command_dispatch_tests/changes_tests.rs"]
mod changes_tests;
#[path = "remote_command_dispatch_tests/execution_consent_tests.rs"]
mod execution_consent_tests;
#[path = "remote_command_dispatch_tests/pending_runs_tests.rs"]
mod pending_runs_tests;
#[path = "remote_command_dispatch_tests/prompts_tests.rs"]
mod prompts_tests;
#[path = "remote_command_dispatch_tests/runner_intent_tests.rs"]
mod runner_intent_tests;
#[path = "remote_command_dispatch_tests/runner_preflight_tests.rs"]
mod runner_preflight_tests;
#[path = "remote_command_dispatch_tests/scheduler_lease_tests.rs"]
mod scheduler_lease_tests;
#[path = "remote_command_dispatch_tests/scheduler_selection_tests.rs"]
mod scheduler_selection_tests;
#[path = "remote_command_dispatch_tests/sessions_tests.rs"]
mod sessions_tests;

fn converged_instance_reads() -> Vec<ExpectedRequest> {
    vec![
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
    ]
}

fn converged_instance_write(response: ExpectedRequest) -> (RemoteClient, thread::JoinHandle<()>) {
    let mut requests = converged_instance_reads();
    requests.push(response);
    spawn_mock_server(requests)
}
