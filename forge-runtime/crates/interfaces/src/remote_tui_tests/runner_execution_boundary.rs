use std::{
    io::{Cursor, ErrorKind},
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use forge_runtime_domain::execution::runner_command::RunnerCommand;
use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{accept_request, helpers::test_client, respond, run_with_io};

fn request() -> Value {
    json!({
        "owner":{"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted",
        "command":{"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":3,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task","--prompt-ref","prompt-1"],"timeout_ms":5000,"max_output_bytes":65536},
        "transport":{"schema_version":"forge.runner-transport-admission/v1","evaluation_mode":"pure_runner_transport_admission","method":"POST","path":"/api/v1/runners/runner-a/dispatch","timestamp":300,"nonce":"nonce-a","payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","payload_bytes":256,"replay_checked":true,"preview_only":true,"authority":{"identity_verified":false,"heartbeat_accepted":false,"lease_issued":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}},
        "expected_payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","controls":{"effect_state":"not_started","cancellation_requested":false}
    })
}

fn response(request: &Value) -> Value {
    let command: RunnerCommand = serde_json::from_value(request["command"].clone()).unwrap();
    json!({
        "schema_version":"forge.runner-execution-boundary/v1","evaluation_mode":"p4_runner_authority_execution_boundary_preview","mode":"execute","owner":request["owner"].clone(),"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted","command_id":"command-1","command_sha256":command.command_sha256().unwrap(),"target_id":"runner-a","lease_epoch":3,"activation_allowed":true,"runner_authority_accepted":true,"dispatch_admission_ready":true,"transport_admission_ready":true,"effect_state":"not_started","effect_state_startable":true,"cancellation_clear":true,"execution_boundary_ready":true,"rejection_reasons":[],"preview_only":true,"authority":{"device_identity_verified":false,"command_persisted":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}
    })
}

#[tokio::test]
async fn remote_tui_posts_runner_execution_boundary_for_selected_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = request();
    let expected_request = request.clone();
    let expected_response = response(&request);
    let server = thread::spawn(move || {
        let (mut first, request_line, _headers, _body) = accept_request(&listener);
        assert!(request_line.starts_with("GET /api/v1/conversations?limit=128 "));
        respond(
            &mut first,
            "200 OK",
            &json!({"conversations":[{"conversation":{"id":"conversation-1","scope":{"kind":"global"},"title":"Shared","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}],"next_after_id":null,"has_more":false}),
        );
        drop(first);
        let (mut second, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with("POST /api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview "));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut second, "200 OK", &expected_response);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-execution-boundary-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Runner execution boundary"), "{output}");
    assert!(output.contains("ready=true"), "{output}");
    assert!(!output.contains("token-a"), "{output}");
    assert!(!output.contains("forge-task"), "{output}");
}

fn selected_resource_view() -> Value {
    let mut resource: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    resource["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["instance_id"] == json!("client-web-001"))
        .unwrap()["session_ids"] = json!(["conversation-1"]);
    resource
}

fn converged_inventory_resource() -> (Value, Value) {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let mut resource = fixture["resource_view"].clone();
    resource["instances"] = selected_resource_view()["instances"].clone();
    (fixture["inventory"].clone(), resource)
}

fn serve_selected_resource(listener: &TcpListener, resource: &Value) {
    let (mut stream, request_line, _, body) = accept_request(listener);
    assert!(request_line.starts_with("GET /api/v1/conversations?limit=128 "));
    assert!(body.is_empty());
    respond(
        &mut stream,
        "200 OK",
        &json!({
            "conversations": [{"conversation": {"id":"conversation-1","scope":{"kind":"global"},"title":"Shared","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}],
            "next_after_id": null,
            "has_more": false
        }),
    );
    drop(stream);
    let (mut stream, request_line, _, body) = accept_request(listener);
    assert!(request_line.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", resource);
}

fn assert_no_boundary_post(listener: &TcpListener) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        match listener.accept() {
            Ok((_stream, address)) => panic!("unexpected execution-boundary request: {address}"),
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => return,
            Err(error) => panic!("unexpected listener failure: {error}"),
        }
    }
}

#[tokio::test]
async fn remote_tui_execution_boundary_requires_selected_resource_pair() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = request();
    let server = thread::spawn(move || {
        serve_selected_resource(&listener, &selected_resource_view());
        assert_no_boundary_post(&listener);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances resource-view\nfilter instance:client-web-001\nrunner-execution-boundary-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains(
        "Runner execution boundary blocked by selected client-instance resource projection: open converged inventory/resource observations first. No request was sent."
    ), "{output}");
}

#[tokio::test]
async fn remote_tui_execution_boundary_blocks_foreign_resource_target() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut request = request();
    request["command"]["lease_proof"]["target_id"] = json!("runner-foreign");
    let initial_resource = selected_resource_view();
    let (inventory, resource) = converged_inventory_resource();
    let server = thread::spawn(move || {
        serve_selected_resource(&listener, &initial_resource);
        for (path, payload) in [
            ("GET /api/v1/devices/observations/v2 ", &inventory),
            ("GET /api/v1/client-instances/resource-view ", &resource),
            ("GET /api/v1/devices/observations/v2 ", &inventory),
            ("GET /api/v1/client-instances/resource-view ", &resource),
        ] {
            let (mut stream, request_line, _, body) = accept_request(&listener);
            assert!(request_line.starts_with(path), "{request_line}");
            assert!(body.is_empty());
            respond(&mut stream, "200 OK", payload);
        }
        assert_no_boundary_post(&listener);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances resource-view\nfilter instance:client-web-001\ninventory show-converged\nrunner-execution-boundary-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains(
        "Runner execution boundary blocked by selected client-instance resource projection: target \"runner-foreign\" is absent from the current resource observation. No request was sent."
    ), "{output}");
}

#[tokio::test]
async fn remote_tui_execution_boundary_accepts_resource_device_target_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = request();
    let expected_request = request.clone();
    let expected_response = response(&request);
    let initial_resource = selected_resource_view();
    let (inventory, resource) = converged_inventory_resource();
    let server = thread::spawn(move || {
        serve_selected_resource(&listener, &initial_resource);
        for (path, payload) in [
            ("GET /api/v1/devices/observations/v2 ", &inventory),
            ("GET /api/v1/client-instances/resource-view ", &resource),
            ("GET /api/v1/devices/observations/v2 ", &inventory),
            ("GET /api/v1/client-instances/resource-view ", &resource),
        ] {
            let (mut stream, request_line, _, body) = accept_request(&listener);
            assert!(request_line.starts_with(path), "{request_line}");
            assert!(body.is_empty());
            respond(&mut stream, "200 OK", payload);
        }
        let (mut stream, request_line, _, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-1/runs/run-1/runner-execution-boundary/preview "
        ));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &expected_response);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances resource-view\nfilter instance:client-web-001\ninventory show-converged\nrunner-execution-boundary-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Runner execution boundary"), "{output}");
    assert!(output.contains("ready=true"), "{output}");
}
