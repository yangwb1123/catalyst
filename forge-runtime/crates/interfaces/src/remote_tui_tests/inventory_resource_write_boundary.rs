use std::{net::TcpListener, thread};

use serde_json::{Value, json};

use super::super::{
    OwnedConversationEntry,
    state::{PendingPrompt, TuiState},
    writes::ensure_inventory_resource_converged,
};
use super::helpers::{accept_request, prompt_append_response, respond};

fn owner() -> Value {
    json!({
        "issuer": "https://id.example",
        "subject": "user-1",
        "tenant_id": "tenant-1"
    })
}

fn observation_pair() -> (Value, Value) {
    let owner = owner();
    (
        json!({
            "owner_declaration": owner.clone(),
            "owner_declaration_unverified": true,
            "inventory_declarations_unverified": true,
            "devices": [{
                "instance_id": "runner-a",
                "revision": 7,
                "generation": 3,
                "heartbeat_sequence": 12,
                "device": {
                    "device_id": "device-a",
                    "owner": owner.clone(),
                    "approval_state": "approved",
                    "cordon_state": "clear",
                    "reservation_state": "none",
                    "liveness": "online",
                    "snapshot_observed_at_ms": 150000,
                    "os": "linux",
                    "architecture": "amd64",
                    "available_cpu_cores": 8,
                    "available_memory_bytes": 16384,
                    "available_storage_bytes": 8192,
                    "gpus": []
                }
            }]
        }),
        json!({
            "owner_declaration": owner.clone(),
            "owner_declaration_unverified": true,
            "device_attributes_unverified": true,
            "devices": [{
                "device_id": "device-a",
                "runner_instance_id": "runner-a",
                "revision": 7,
                "generation": 3,
                "heartbeat_sequence": 12,
                "observed_at_ms": 150000,
                "owner": owner,
                "approval_state": "approved",
                "cordon_state": "clear",
                "reservation_state": "none",
                "liveness": "online",
                "os": "linux",
                "architecture": "amd64",
                "available_cpu_cores": 8,
                "available_memory_bytes": 16384,
                "available_storage_bytes": 8192,
                "gpu_count": 0,
                "available_gpu_memory_bytes": 0
            }]
        }),
    )
}

#[test]
fn converged_pair_allows_the_display_only_write_boundary() {
    let (inventory, resource) = observation_pair();
    let state = TuiState {
        device_inventory_v2_observed: Some(inventory),
        client_instance_resource_view_observed: Some(resource),
        ..TuiState::default()
    };
    let mut output = Vec::new();

    assert!(ensure_inventory_resource_converged(&state, "Prompt", &mut output).unwrap());
    assert!(output.is_empty());
}

#[test]
fn drifted_pair_blocks_without_creating_write_state() {
    let (inventory, mut resource) = observation_pair();
    resource["devices"][0]["observed_at_ms"] = json!(150001);
    let state = TuiState {
        device_inventory_v2_observed: Some(inventory),
        client_instance_resource_view_observed: Some(resource),
        ..TuiState::default()
    };
    let mut output = Vec::new();

    assert!(
        !ensure_inventory_resource_converged(&state, "Pending Run-intent", &mut output).unwrap()
    );
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Pending Run-intent blocked by inventory/resource observation drift. No request was sent.\n"
    );
}

#[test]
fn one_sided_observation_keeps_existing_compatibility() {
    let (inventory, _) = observation_pair();
    let state = TuiState {
        device_inventory_v2_observed: Some(inventory),
        ..TuiState::default()
    };
    let mut output = Vec::new();

    assert!(
        ensure_inventory_resource_converged(&state, "Scheduler selection preview", &mut output)
            .unwrap()
    );
    assert!(output.is_empty());
}

fn canonical_pair() -> (Value, Value) {
    let value: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("inventory/resource convergence fixture");
    (value["inventory"].clone(), value["resource_view"].clone())
}

fn visible_prompt_state(inventory: Value, resource: Value) -> TuiState {
    TuiState {
        conversations: vec![OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-001",
                "scope": {"kind": "global"},
                "title": "Shared"
            }),
            aggregate_version: 1,
        }],
        selected_id: Some("conversation-001".into()),
        selected_entry: Some(OwnedConversationEntry {
            conversation: json!({
                "id": "conversation-001",
                "scope": {"kind": "global"},
                "title": "Shared"
            }),
            aggregate_version: 1,
        }),
        client_instance_filter: Some("client-web-001".into()),
        device_inventory_v2_observed: Some(inventory),
        client_instance_resource_view_observed: Some(resource),
        pending_prompt: Some(PendingPrompt {
            conversation_id: "conversation-001".into(),
            expected_version: 1,
            content: "send this".into(),
            idempotency_key: "prompt-key".into(),
        }),
        ..TuiState::default()
    }
}

#[tokio::test]
async fn explicit_instance_prompt_retry_refreshes_inventory_resource_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, resource) = canonical_pair();
    let server = thread::spawn(move || {
        let (mut inventory_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut inventory_response, "200 OK", &inventory);

        let (mut resource_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource_response, "200 OK", &resource);

        let (mut prompt_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/conversations/conversation-001/prompts "));
        let prompt: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(prompt["content"], "send this");
        assert_eq!(prompt["expected_version"], 1);
        respond(
            &mut prompt_response,
            "201 Created",
            &prompt_append_response("conversation-001", "prompt-1", "send this", 2, false),
        );

        let (mut history_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/conversation-001/prompts?"));
        assert!(body.is_empty());
        respond(
            &mut history_response,
            "200 OK",
            &json!({
                "conversation_id": "conversation-001",
                "prompts": [],
                "has_more": false
            }),
        );
    });

    let client = super::helpers::receipt_test_client(address);
    let (inventory, resource) = canonical_pair();
    let mut state = visible_prompt_state(inventory, resource);
    let mut writer = Vec::new();
    let exited =
        super::super::commands::dispatch_command(&client, &mut state, None, "retry", &mut writer)
            .await
            .unwrap();
    assert!(!exited);
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Prompt stored. No Run was started."),
        "{output}"
    );
    assert!(output.contains("Prompt history refreshed."), "{output}");
    assert!(state.pending_prompt.is_none());
}

#[tokio::test]
async fn explicit_instance_prompt_retry_blocks_on_inventory_resource_drift() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, mut resource) = canonical_pair();
    resource["devices"][0]["heartbeat_sequence"] = json!(99);
    let server = thread::spawn(move || {
        let (mut inventory_response, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        respond(&mut inventory_response, "200 OK", &inventory);
        let (mut resource_response, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        respond(&mut resource_response, "200 OK", &resource);
    });

    let client = super::helpers::receipt_test_client(address);
    let (inventory, resource) = canonical_pair();
    let mut state = visible_prompt_state(inventory, resource);
    let mut output = Vec::new();
    let exited =
        super::super::commands::dispatch_command(&client, &mut state, None, "retry", &mut output)
            .await
            .unwrap();
    assert!(!exited);
    server.join().unwrap();

    assert!(state.pending_prompt.is_some());
    let output = String::from_utf8(output).unwrap();
    assert!(
        output.contains("Prompt inventory/resource refresh failed"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
    assert!(!output.contains("Prompt stored."), "{output}");
}
