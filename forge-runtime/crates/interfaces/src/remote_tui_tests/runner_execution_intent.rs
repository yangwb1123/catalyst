use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::super::state::TuiState;
use super::{
    accept_request,
    helpers::{serve_conversation_page, test_client},
    respond, run_with_io,
};

fn canonical_inventory_resource_pair() -> (Value, Value) {
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("inventory/resource convergence fixture");
    (pair["inventory"].clone(), pair["resource_view"].clone())
}

fn selected_instance_state(inventory: Value, resource: Value) -> TuiState {
    let mut session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .expect("client-instance session-view fixture");
    session["instances"] = resource["instances"].clone();
    TuiState {
        selected_id: Some("conversation-001".into()),
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(session),
        client_instance_resource_view_observed: Some(resource),
        device_inventory_v2_observed: Some(inventory),
        ..TuiState::default()
    }
}

#[tokio::test]
async fn remote_tui_posts_runner_execution_intent_for_selected_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json"
    ))
    .unwrap();
    let typed_request: forge_runtime_domain::execution::runner_execution_intent::RunnerExecutionIntentRequest =
        serde_json::from_value(request.clone()).unwrap();
    let expected_response = serde_json::to_value(
        forge_runtime_domain::execution::runner_execution_intent::observe_runner_execution_intent(
            typed_request,
        )
        .unwrap(),
    )
    .unwrap();
    let expected_request = request.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {"id":"conversation-001","scope":{"kind":"global"},"title":"Shared","created_at_ms":1,"updated_at_ms":1},
                    "aggregate_version":1
                }],
                "next_after_id": null, "has_more": false
            }),
        );
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
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
        "runner-execution-intent-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("remote Runner execution intent preview"),
        "{output}"
    );
    assert!(output.contains("selected_target=none"), "{output}");
    assert!(output.contains("execution_authorized=false"), "{output}");
    assert!(!output.contains("fence-001"), "{output}");
    assert!(!output.contains("forge-task"), "{output}");
}

#[tokio::test]
async fn remote_tui_runner_execution_intent_refreshes_explicit_pair_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json"
    ))
    .unwrap();
    let typed_request: forge_runtime_domain::execution::runner_execution_intent::RunnerExecutionIntentRequest =
        serde_json::from_value(request.clone()).unwrap();
    let response = serde_json::to_value(
        forge_runtime_domain::execution::runner_execution_intent::observe_runner_execution_intent(
            typed_request,
        )
        .unwrap(),
    )
    .unwrap();
    let expected_request = request.clone();
    let (inventory, resource) = canonical_inventory_resource_pair();
    let server = thread::spawn(move || {
        let (mut stream, request_line, _, body) = accept_request(&listener);
        assert!(request_line.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &inventory);

        let (mut stream, request_line, _, body) = accept_request(&listener);
        assert!(request_line.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &resource);

        let (mut stream, request_line, _, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/runner-execution-intent/preview "
        ));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &response);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let (inventory, resource) = canonical_inventory_resource_pair();
    let mut state = selected_instance_state(inventory, resource);
    let mut writer = Vec::new();
    super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "runner-execution-intent-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("remote Runner execution intent preview"),
        "{output}"
    );
    assert!(output.contains("execution_authorized=false"), "{output}");
}

#[tokio::test]
async fn remote_tui_runner_execution_intent_blocks_pair_drift_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-execution-intent-request-v1.json"
    ))
    .unwrap();
    let (inventory, resource) = canonical_inventory_resource_pair();
    let mut drifted_resource = resource.clone();
    drifted_resource["devices"][0]["heartbeat_sequence"] = json!(99);
    let server = thread::spawn(move || {
        let (mut stream, request_line, _, _) = accept_request(&listener);
        assert!(request_line.starts_with("GET /api/v1/devices/observations/v2 "));
        respond(&mut stream, "200 OK", &inventory);
        let (mut stream, request_line, _, _) = accept_request(&listener);
        assert!(request_line.starts_with("GET /api/v1/client-instances/resource-view "));
        respond(&mut stream, "200 OK", &drifted_resource);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let (inventory, resource) = canonical_inventory_resource_pair();
    let mut state = selected_instance_state(inventory, resource);
    let mut writer = Vec::new();
    super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "runner-execution-intent-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Runner execution-intent inventory/resource refresh failed"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
}
