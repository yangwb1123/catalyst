use serde_json::json;

use super::super::{
    OwnedConversationEntry,
    commands::dispatch_command,
    state::{PendingRunIntent, TuiState},
};
use super::helpers::{accept_request, conversation_projection, respond, test_client};
use std::time::{Duration, Instant};
use std::{net::TcpListener, thread};

#[tokio::test]
async fn tui_reads_pending_run_intent_metadata_and_payload_free_timeline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut list, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/run-intents?limit=25 "));
        respond(
            &mut list,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "intents": [{
                    "intent_id": "intent-1",
                    "conversation_id": "c-1",
                    "prompt_id": "prompt-1",
                    "project_id": "project-1",
                    "profile_id": "profile-1",
                    "submitted_at_ms": 20,
                    "aggregate_version": 2,
                    "latest_sequence": 1,
                    "status": "pending"
                }],
                "has_more": false
            }),
        );

        let (mut timeline, request, _, _) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversations/c-1/run-intents/intent-1/timeline?after_sequence=0&limit=25 "
        ));
        respond(
            &mut timeline,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
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
        );
    });

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(&client, &mut state, None, "run-intents", &mut writer)
        .await
        .unwrap();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents timeline intent-1",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Run-intent \"intent-1\" status=\"pending\" profile=\"profile-1\""));
    assert!(output.contains("Run-intent event seq=1 emitted_at_ms=20 type=\"submitted\""));
    assert!(!output.contains("prompt_content"));
    assert!(!output.contains("private"));
}

#[tokio::test]
async fn tui_sync_refreshes_an_explicitly_opened_pending_run_intent_page() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut initial, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/run-intents?limit=25 "));
        assert!(body.is_empty());
        respond(
            &mut initial,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "intents": [{
                    "intent_id": "intent-1",
                    "conversation_id": "c-1",
                    "prompt_id": "prompt-1",
                    "project_id": "project-1",
                    "profile_id": "profile-1",
                    "submitted_at_ms": 20,
                    "aggregate_version": 2,
                    "latest_sequence": 1,
                    "status": "pending"
                }],
                "has_more": false
            }),
        );

        let (mut changes, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
        assert!(body.is_empty());
        respond(
            &mut changes,
            "200 OK",
            &json!({
                "after_cursor": 0,
                "scanned_through_cursor": 0,
                "has_more": false,
                "changes": []
            }),
        );

        super::helpers::serve_conversation_page(&listener, &super::helpers::conversation_page(2));
        let (mut history, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        assert!(body.is_empty());
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [],
                "has_more": false
            }),
        );

        let (mut refreshed, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/run-intents?limit=25 "));
        assert!(body.is_empty());
        respond(
            &mut refreshed,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "intents": [{
                    "intent_id": "intent-1",
                    "conversation_id": "c-1",
                    "prompt_id": "prompt-1",
                    "project_id": "project-1",
                    "profile_id": "profile-2",
                    "submitted_at_ms": 20,
                    "aggregate_version": 2,
                    "latest_sequence": 1,
                    "status": "pending"
                }],
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 1,
    });
    let mut writer = Vec::new();
    dispatch_command(&client, &mut state, None, "run-intents", &mut writer)
        .await
        .unwrap();
    dispatch_command(&client, &mut state, None, "sync", &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_eq!(output.matches("Run-intent \"intent-1\"").count(), 2);
    assert!(output.contains("profile=\"profile-2\""), "{output}");
    assert!(output.contains("Selected pending Run-intent metadata refreshed."));
    assert!(output.contains("Synced 0 owner-visible changes through cursor 0"));
}

#[tokio::test]
async fn tui_resumes_pending_run_intent_timeline_from_the_bound_in_process_cursor() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut initial, request, _, body) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversations/c-1/run-intents/intent-1/timeline?after_sequence=0&limit=25 "
        ));
        assert!(body.is_empty());
        respond(
            &mut initial,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
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
        );

        let (mut resumed, request, _, body) = accept_request(&listener);
        assert!(request.starts_with(
            "GET /api/v1/conversations/c-1/run-intents/intent-1/timeline?after_sequence=1&limit=25 "
        ));
        assert!(body.is_empty());
        respond(
            &mut resumed,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "intent_id": "intent-1",
                "after_sequence": 1,
                "scanned_through_sequence": 1,
                "has_more": false,
                "events": []
            }),
        );
    });

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents timeline intent-1",
        &mut writer,
    )
    .await
    .unwrap();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents timeline intent-1 --resume",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert_eq!(
        state.selected_pending_run_intent_conversation_id.as_deref(),
        Some("c-1")
    );
    assert_eq!(
        state.selected_pending_run_intent_id.as_deref(),
        Some("intent-1")
    );
    assert_eq!(state.pending_run_intent_timeline_sequence, 1);
    let output = String::from_utf8(writer).unwrap();
    assert_eq!(output.matches("Run-intent event seq=1").count(), 1);
    assert!(output.contains("No pending Run-intent timeline events on this page."));
}

#[tokio::test]
async fn tui_rejects_pending_run_intent_resume_without_an_exact_prior_binding() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents timeline intent-1 --resume",
        &mut writer,
    )
    .await
    .unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains(
        "Pending Run-intent timeline resume requires a prior read for this session and intent."
    ));
    assert_eq!(state.pending_run_intent_timeline_sequence, 0);
}

#[tokio::test]
async fn tui_submits_pending_run_intent_with_selected_version_and_retries_history_read() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut submit, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/run-intents "));
        assert!(headers.contains("idempotency-key: forge-tui-"));
        let request_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(request_json["content"], "compute this");
        assert_eq!(request_json["expected_version"], 2);
        respond(
            &mut submit,
            "201 Created",
            &json!({
                "prompt": {
                    "id": "prompt-2",
                    "conversation_id": "c-1",
                    "role": "user",
                    "content": "compute this",
                    "created_at_ms": 30
                },
                "intent": {
                    "intent_id": "intent-2",
                    "conversation_id": "c-1",
                    "prompt_id": "prompt-2",
                    "project_id": "project-1",
                    "profile_id": "profile-1",
                    "submitted_at_ms": 30,
                    "aggregate_version": 3,
                    "latest_sequence": 1,
                    "status": "pending"
                },
                "initial_event": {
                    "event_id": "event-2",
                    "seq": 1,
                    "emitted_at_ms": 30,
                    "type": "submitted"
                },
                "replayed": false
            }),
        );

        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?limit=128 "));
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "prompts": [{
                    "id": "prompt-2",
                    "conversation_id": "c-1",
                    "role": "user",
                    "content": "compute this",
                    "created_at_ms": 30
                }],
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents submit compute this",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert!(state.pending_run_intent.is_none());
    assert_eq!(state.selected_entry.unwrap().aggregate_version, 3);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Pending Run-intent \"intent-2\" stored. No Run was started."));
    assert!(output.contains("Prompt history refreshed."));
}

#[tokio::test]
async fn explicit_instance_pending_run_intent_retry_refreshes_inventory_resource_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, resource) = inventory_resource_pair();
    let inventory_observation = inventory.clone();
    let resource_observation = resource.clone();
    let session_view = session_view_for_resource(&resource_observation);
    let server_inventory = inventory_observation.clone();
    let server_resource = resource_observation.clone();
    let server = thread::spawn(move || {
        let (mut inventory_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut inventory_response, "200 OK", &server_inventory);

        let (mut resource_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource_response, "200 OK", &server_resource);

        let (mut submit, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/conversations/conversation-001/run-intents "));
        let request_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(request_json["content"], "compute this");
        assert_eq!(request_json["expected_version"], 2);
        respond(
            &mut submit,
            "201 Created",
            &json!({
                "prompt": {
                    "id": "prompt-2",
                    "conversation_id": "conversation-001",
                    "role": "user",
                    "content": "compute this",
                    "created_at_ms": 30
                },
                "intent": {
                    "intent_id": "intent-2",
                    "conversation_id": "conversation-001",
                    "prompt_id": "prompt-2",
                    "project_id": "project-1",
                    "profile_id": "profile-1",
                    "submitted_at_ms": 30,
                    "aggregate_version": 3,
                    "latest_sequence": 1,
                    "status": "pending"
                },
                "initial_event": {
                    "event_id": "event-2",
                    "seq": 1,
                    "emitted_at_ms": 30,
                    "type": "submitted"
                },
                "replayed": false
            }),
        );

        let (mut history, request, _, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/conversations/conversation-001/prompts?limit=128 ")
        );
        assert!(body.is_empty());
        respond(
            &mut history,
            "200 OK",
            &json!({
                "conversation_id": "conversation-001",
                "prompts": [{
                    "id": "prompt-2",
                    "conversation_id": "conversation-001",
                    "role": "user",
                    "content": "compute this",
                    "created_at_ms": 30
                }],
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut state = pending_run_intent_state(
        inventory_observation.clone(),
        resource_observation.clone(),
        session_view,
    );
    let mut writer = Vec::new();
    dispatch_command(&client, &mut state, None, "retry", &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    assert!(state.pending_run_intent.is_none());
    assert_eq!(state.selected_conversation().unwrap().aggregate_version, 3);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Pending Run-intent \"intent-2\" stored. No Run was started."));
    assert!(output.contains("Prompt history refreshed."));
}

#[tokio::test]
async fn explicit_instance_pending_run_intent_retry_keeps_pending_on_inventory_resource_drift() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, resource) = inventory_resource_pair();
    let inventory_observation = inventory.clone();
    let resource_observation = resource.clone();
    let session_view = session_view_for_resource(&resource_observation);
    let mut drifted_resource = resource_observation.clone();
    drifted_resource["devices"][0]["heartbeat_sequence"] = json!(99);
    let server_inventory = inventory_observation.clone();
    let server_drifted_resource = drifted_resource.clone();
    let server = thread::spawn(move || {
        let (mut inventory_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut inventory_response, "200 OK", &server_inventory);

        let (mut resource_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource_response, "200 OK", &server_drifted_resource);

        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let (request, _, _) = super::helpers::read_request(&mut stream);
                    panic!("pending Run-intent POST escaped inventory/resource drift: {request}");
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("checking for unexpected pending Run-intent request: {error}"),
            }
        }
    });

    let client = test_client(address);
    let mut state =
        pending_run_intent_state(inventory_observation, resource_observation, session_view);
    let mut writer = Vec::new();
    dispatch_command(&client, &mut state, None, "retry", &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    assert!(state.pending_run_intent.is_some());
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Pending Run-intent inventory/resource refresh failed"));
    assert!(output.contains("No request was sent."));
}

#[tokio::test]
async fn tui_clears_pending_run_intent_and_refreshes_on_cas_conflict() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut submit, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/conversations/c-1/run-intents "));
        assert!(headers.contains("idempotency-key: forge-tui-"));
        let request_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(request_json["content"], "compute this");
        assert_eq!(request_json["expected_version"], 2);
        respond(
            &mut submit,
            "409 Conflict",
            &json!({"code": "aggregate_version_conflict"}),
        );

        let (mut list, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations?limit=128"));
        respond(
            &mut list,
            "200 OK",
            &json!({
                "conversations": [{
                    "conversation": conversation_projection("c-1", "Shared"),
                    "aggregate_version": 3
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
    });

    let client = test_client(address);
    let mut state = TuiState::default();
    state.selected_id = Some("c-1".into());
    state.selected_entry = Some(OwnedConversationEntry {
        conversation: conversation_projection("c-1", "Shared"),
        aggregate_version: 2,
    });
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        "run-intents submit compute this",
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();

    assert!(state.pending_run_intent.is_none());
    assert_eq!(state.selected_conversation().unwrap().aggregate_version, 3);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("Pending Run-intent was rejected: Forge API returned HTTP 409"));
    assert!(output.contains("The session was refreshed; submit the pending Run-intent again."));
    assert!(!output.contains("Enter retry to reuse the same version"));
}

fn inventory_resource_pair() -> (serde_json::Value, serde_json::Value) {
    let pair: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    (pair["inventory"].clone(), pair["resource_view"].clone())
}

fn session_view_for_resource(resource: &serde_json::Value) -> serde_json::Value {
    let mut session_view: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    session_view["instances"] = resource["instances"].clone();
    session_view
}

fn pending_run_intent_state(
    inventory: serde_json::Value,
    resource: serde_json::Value,
    session_view: serde_json::Value,
) -> TuiState {
    let entry = OwnedConversationEntry {
        conversation: conversation_projection("conversation-001", "Shared"),
        aggregate_version: 2,
    };
    TuiState {
        conversations: vec![entry.clone()],
        selected_id: Some("conversation-001".into()),
        selected_entry: Some(entry),
        client_instance_filter: Some("client-web-001".into()),
        device_inventory_v2_observed: Some(inventory),
        client_instance_session_view_observed: Some(session_view),
        client_instance_resource_view_observed: Some(resource),
        pending_run_intent: Some(PendingRunIntent {
            conversation_id: "conversation-001".into(),
            expected_version: 2,
            content: "compute this".into(),
            idempotency_key: "pending-run-intent-key".into(),
        }),
        ..TuiState::default()
    }
}
