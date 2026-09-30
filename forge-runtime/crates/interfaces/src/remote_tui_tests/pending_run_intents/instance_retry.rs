use super::*;

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
    let server =
        thread::spawn(move || serve_instance_retry(listener, server_inventory, server_resource));

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

fn serve_instance_retry(listener: TcpListener, server_inventory: Value, server_resource: Value) {
    serve_retry_inventory_resource(&listener, &server_inventory, &server_resource);
    serve_retried_intent(&listener);
    serve_retried_prompt_history(&listener);
}

fn serve_retry_inventory_resource(
    listener: &TcpListener,
    server_inventory: &Value,
    server_resource: &Value,
) {
    let (mut inventory_response, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    assert!(body.is_empty());
    respond(&mut inventory_response, "200 OK", server_inventory);

    let (mut resource_response, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut resource_response, "200 OK", server_resource);
}

fn serve_retried_intent(listener: &TcpListener) {
    let (mut submit, request, _, body) = accept_request(listener);
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
}

fn serve_retried_prompt_history(listener: &TcpListener) {
    let (mut history, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/conversation-001/prompts?limit=128 "));
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
        serve_retry_drift(listener, server_inventory, server_drifted_resource)
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

fn serve_retry_drift(
    listener: TcpListener,
    server_inventory: Value,
    server_drifted_resource: Value,
) {
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
                let (request, _, _) = super::super::helpers::read_request(&mut stream);
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
}
