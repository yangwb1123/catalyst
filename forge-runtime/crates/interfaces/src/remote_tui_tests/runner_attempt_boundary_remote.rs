use std::{
    fs,
    io::{Cursor, ErrorKind},
    net::{SocketAddr, TcpListener},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::super::state::TuiState;
use super::{accept_request, helpers::test_client, respond, run_with_io};

fn request() -> Value {
    json!({
        "owner":{"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","attempt_state":"accepted",
        "command":{"v":1,"command_id":"command-1","lease_proof":{"attempt_id":"attempt-1","target_id":"runner-a","epoch":3,"fencing_token":"token-a"},"idempotency_key":"run-1:attempt-1:command-1","workspace_ref":"workspace-1","argv":["forge-task","--prompt-ref","prompt-1"],"timeout_ms":5000,"max_output_bytes":65536},
        "transport":{"schema_version":"forge.runner-transport-admission/v1","evaluation_mode":"pure_runner_transport_admission","method":"POST","path":"/api/v1/runners/runner-a/dispatch","timestamp":300,"nonce":"nonce-a","payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","payload_bytes":256,"replay_checked":true,"preview_only":true,"authority":{"identity_verified":false,"heartbeat_accepted":false,"lease_issued":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}},
        "expected_payload_sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","controls":{"effect_state":"not_started","cancellation_requested":false},"transition":"begin_starting"
    })
}

fn response(request: &Value) -> Value {
    json!({
        "schema_version":"forge.runner-attempt-boundary/v1","evaluation_mode":"attempt_lifecycle_dispatch_boundary_preview","owner":request["owner"].clone(),"conversation_id":"conversation-1","run_id":"run-1","attempt_id":"attempt-1","command_id":"command-1","target_id":"runner-a","lease_epoch":3,"current_attempt_state":"accepted","next_attempt_state":"starting","transition":"begin_starting","execution_boundary_ready":true,"attempt_transition_valid":true,"attempt_transition_dispatchable":true,"attempt_boundary_ready":true,"rejection_reasons":[],"preview_only":true,"authority":{"attempt_persisted":false,"reservation_created":false,"execution_authorized":false,"dispatch_performed":false,"audit_published":false}
    })
}

#[tokio::test]
async fn remote_tui_posts_attempt_boundary_for_selected_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = request();
    let expected_response = response(&request);
    let expected_request = request.clone();
    let server = thread::spawn(move || {
        let (mut first, line, _, _) = accept_request(&listener);
        assert!(line.starts_with("GET /api/v1/conversations?limit=128 "));
        respond(
            &mut first,
            "200 OK",
            &json!({"conversations":[{"conversation":{"id":"conversation-1","scope":{"kind":"global"},"title":"Shared","created_at_ms":1,"updated_at_ms":1},"aggregate_version":1}],"next_after_id":null,"has_more":false}),
        );
        drop(first);
        let (mut second, line, headers, body) = accept_request(&listener);
        assert!(line.starts_with(
            "POST /api/v1/conversations/conversation-1/runs/run-1/runner-attempt-boundary/preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut second, "200 OK", &expected_response);
    });
    let input = NamedTempFile::new().unwrap();
    fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-attempt-boundary-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Runner Attempt boundary"), "{output}");
    assert!(output.contains("attempt_boundary_ready=true"), "{output}");
    assert!(!output.contains("token-a"), "{output}");
    assert!(!output.contains("forge-task"), "{output}");
}

#[tokio::test]
async fn remote_tui_attempt_boundary_requires_selected_session() {
    let input = NamedTempFile::new().unwrap();
    fs::write(input.path(), serde_json::to_vec(&request()).unwrap()).unwrap();
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = super::super::state::TuiState::default();
    state.selected_id = Some("different-session".into());
    let mut writer = Vec::new();
    super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "runner-attempt-boundary-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("requires the selected session"), "{output}");
}

fn selected_instance_state() -> TuiState {
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let mut resource = pair["resource_view"].clone();
    resource["instances"][0]["session_ids"] = json!(["conversation-1"]);
    let mut session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    session["instances"] = resource["instances"].clone();
    TuiState {
        selected_id: Some("conversation-1".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(session),
        client_instance_resource_view_observed: Some(resource),
        device_inventory_v2_observed: Some(pair["inventory"].clone()),
        ..TuiState::default()
    }
}

fn request_for_target(target_id: &str) -> Value {
    let mut request = request();
    request["command"]["lease_proof"]["target_id"] = json!(target_id);
    request["transport"]["path"] = json!(format!("/api/v1/runners/{target_id}/dispatch"));
    request
}

async fn dispatch_preview(address: SocketAddr, state: &mut TuiState, request: &Value) -> String {
    let input = NamedTempFile::new().unwrap();
    fs::write(input.path(), serde_json::to_vec(request).unwrap()).unwrap();
    let mut writer = Vec::new();
    super::super::commands::dispatch_command(
        &test_client(address),
        state,
        None,
        &format!(
            "runner-attempt-boundary-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    String::from_utf8(writer).unwrap()
}

fn serve_refresh(listener: &TcpListener, inventory: &Value, resource: &Value) {
    for (path, payload) in [
        ("GET /api/v1/devices/observations/v2 ", inventory),
        ("GET /api/v1/client-instances/resource-view ", resource),
    ] {
        let (mut stream, line, headers, body) = accept_request(listener);
        assert!(line.starts_with(path), "{line}");
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", payload);
    }
}

fn assert_no_request(listener: &TcpListener) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        match listener.accept() {
            Ok((_stream, address)) => panic!("unexpected Attempt-boundary request: {address}"),
            Err(error) if error.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => return,
            Err(error) => panic!("unexpected listener failure: {error}"),
        }
    }
}

fn refresh_server(
    inventory: Value,
    resource: Value,
    expected_request: Option<Value>,
) -> (SocketAddr, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_refresh(&listener, &inventory, &resource);
        if let Some(expected_request) = expected_request {
            let (mut stream, line, headers, body) = accept_request(&listener);
            assert!(line.starts_with(
                "POST /api/v1/conversations/conversation-1/runs/run-1/runner-attempt-boundary/preview "
            ));
            assert!(headers.contains("authorization: bearer test-token"));
            assert_eq!(
                serde_json::from_slice::<Value>(&body).unwrap(),
                expected_request
            );
            let mut response = response(&expected_request);
            response["target_id"] = expected_request["command"]["lease_proof"]["target_id"].clone();
            respond(&mut stream, "200 OK", &response);
        }
        assert_no_request(&listener);
    });
    (address, server)
}

#[tokio::test]
async fn remote_tui_attempt_boundary_refreshes_before_post_for_device_and_runner_targets() {
    for target_id in ["device-a", "runner-a"] {
        let mut state = selected_instance_state();
        let request = request_for_target(target_id);
        let (address, server) = refresh_server(
            state.device_inventory_v2_observed.clone().unwrap(),
            state
                .client_instance_resource_view_observed
                .clone()
                .unwrap(),
            Some(request.clone()),
        );
        let output = dispatch_preview(address, &mut state, &request).await;
        server.join().unwrap();
        assert!(output.contains("attempt_boundary_ready=true"), "{output}");
        assert!(output.contains(&format!("target={target_id}")), "{output}");
        assert!(output.contains("execution_authorized=false"), "{output}");
        assert!(!output.contains("token-a"), "{output}");
        assert!(!output.contains("forge-task"), "{output}");
    }
}

#[tokio::test]
async fn remote_tui_attempt_boundary_blocks_foreign_target_after_refresh() {
    let mut state = selected_instance_state();
    let (address, server) = refresh_server(
        state.device_inventory_v2_observed.clone().unwrap(),
        state
            .client_instance_resource_view_observed
            .clone()
            .unwrap(),
        None,
    );
    let output = dispatch_preview(address, &mut state, &request_for_target("runner-foreign")).await;
    server.join().unwrap();
    assert!(
        output.contains("target \"runner-foreign\" is absent"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
    assert!(!output.contains("attempt_boundary_ready=true"), "{output}");
}

#[tokio::test]
async fn remote_tui_attempt_boundary_requires_both_selected_observation_pairs() {
    for missing in ["session", "resource", "inventory"] {
        let mut state = selected_instance_state();
        match missing {
            "session" => state.client_instance_session_view_observed = None,
            "resource" => state.client_instance_resource_view_observed = None,
            "inventory" => state.device_inventory_v2_observed = None,
            _ => unreachable!(),
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let output = dispatch_preview(listener.local_addr().unwrap(), &mut state, &request()).await;
        assert_no_request(&listener);
        assert!(
            output.contains(
                "open converged session/resource and inventory/resource observations first"
            ),
            "{missing}: {output}"
        );
        assert!(
            output.contains("No request was sent."),
            "{missing}: {output}"
        );
    }
}

#[tokio::test]
async fn remote_tui_attempt_boundary_blocks_hidden_conversation_after_resource_refresh() {
    let mut state = selected_instance_state();
    let mut resource = state
        .client_instance_resource_view_observed
        .clone()
        .unwrap();
    resource["instances"][0]["session_ids"] = json!([]);
    let (address, server) = refresh_server(
        state.device_inventory_v2_observed.clone().unwrap(),
        resource,
        None,
    );
    let output = dispatch_preview(address, &mut state, &request()).await;
    server.join().unwrap();
    assert!(output.contains("Conversation read blocked"), "{output}");
    assert!(output.contains("No request was sent."), "{output}");
    assert!(state.active_client_instance_view().is_none());
    assert!(!output.contains("attempt_boundary_ready=true"), "{output}");
}

#[tokio::test]
async fn remote_tui_attempt_boundary_blocks_inventory_resource_drift_before_post() {
    let mut state = selected_instance_state();
    let mut resource = state
        .client_instance_resource_view_observed
        .clone()
        .unwrap();
    resource["devices"][0]["revision"] = json!(4);
    let (address, server) = refresh_server(
        state.device_inventory_v2_observed.clone().unwrap(),
        resource,
        None,
    );
    let output = dispatch_preview(address, &mut state, &request()).await;
    server.join().unwrap();
    assert!(
        output.contains("inventory/resource refresh failed"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
    assert!(!output.contains("attempt_boundary_ready=true"), "{output}");
}

#[tokio::test]
async fn remote_tui_attempt_boundary_rechecks_target_against_refreshed_resource() {
    let mut state = selected_instance_state();
    let mut inventory = state.device_inventory_v2_observed.clone().unwrap();
    inventory["devices"][0]["instance_id"] = json!("runner-replaced");
    let mut resource = state
        .client_instance_resource_view_observed
        .clone()
        .unwrap();
    resource["devices"][0]["runner_instance_id"] = json!("runner-replaced");
    let (address, server) = refresh_server(inventory, resource, None);
    let output = dispatch_preview(address, &mut state, &request()).await;
    server.join().unwrap();
    assert!(output.contains("target \"runner-a\" is absent"), "{output}");
    assert!(output.contains("No request was sent."), "{output}");
    assert!(!output.contains("attempt_boundary_ready=true"), "{output}");
}
