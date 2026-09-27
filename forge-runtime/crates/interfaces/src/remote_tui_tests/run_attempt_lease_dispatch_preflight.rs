use std::{io::Cursor, net::TcpListener, path::PathBuf, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::super::state::TuiState;
use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
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
async fn remote_tui_can_preview_run_attempt_lease_dispatch_preflight_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "run-attempt-lease-dispatch-preflight-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline Run/Attempt/lease preflight [forge.run-attempt-lease-dispatch-preflight/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("declarative_preflight_ready=true selected_target_id=null"));
    assert!(output.contains("dispatch_performed=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_posts_authenticated_run_attempt_lease_dispatch_preflight_for_selected_session()
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let expected_request = request.clone();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
    ))
    .unwrap();
    let expected_response = response.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "conversation-001",
                        "scope": {"kind": "global"},
                        "title": "Shared",
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview "
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
        "run-attempt-lease-dispatch-preflight-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Run/Attempt/lease dispatch preflight [forge.run-attempt-lease-dispatch-preflight/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("conversation=conversation-001 run=run-001 status=nonterminal"));
    assert!(output.contains("declarative_preflight_ready=true selected_target_id=null"));
    assert!(output.contains("dispatch_performed=false"));
    assert!(!output.contains("fence-001"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_blocks_run_attempt_lease_preflight_after_inventory_resource_drift() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let state = &mut TuiState {
        selected_id: Some("conversation-001".to_owned()),
        device_inventory_v2_observed: Some(json!({
            "owner_declaration": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
            "owner_declaration_unverified": true,
            "inventory_declarations_unverified": true,
            "devices": []
        })),
        client_instance_resource_view_observed: Some(json!({
            "owner_declaration": {"issuer":"https://id.example","subject":"foreign-user","tenant_id":"tenant-1"},
            "owner_declaration_unverified": true,
            "device_attributes_unverified": true,
            "devices": []
        })),
        ..TuiState::default()
    };
    let mut writer = Vec::new();
    super::super::commands::dispatch_command(
        &client,
        state,
        None,
        &format!(
            "run-attempt-lease-dispatch-preflight-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Run/Attempt/lease preflight blocked by inventory/resource observation drift. No request was sent."
        ),
        "{output}"
    );
    assert!(!output.contains("request failed"), "{output}");
}

#[tokio::test]
async fn remote_tui_run_attempt_lease_preflight_refreshes_pair_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
    ))
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
            "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview "
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
            "run-attempt-lease-dispatch-preflight-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Run/Attempt/lease dispatch preflight [forge.run-attempt-lease-dispatch-preflight/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("dispatch_performed=false"), "{output}");
}

#[tokio::test]
async fn remote_tui_run_attempt_lease_preflight_blocks_refresh_drift_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
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
            "run-attempt-lease-dispatch-preflight-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Run/Attempt/lease preflight inventory/resource refresh failed"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
}
