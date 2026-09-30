use super::*;

#[tokio::test]
async fn remote_tui_show_converged_commits_inventory_and_resource_pair() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    let server = thread::spawn(move || serve_converged_inventory(listener, inventory, resource));

    let client = test_client(address);
    let mut reader = Cursor::new("inventory show-converged\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "remote inventory/resource-convergence [forge.device-inventory-resource-convergence/v1] converged=true read_only=true"
        ),
        "{output}"
    );
    assert!(
        output.contains("remote device inventory [forge.device-inventory-observation/v2]"),
        "{output}"
    );
    assert!(
        output.contains(
            "remote client-instance/resource-view [forge.client-instance-resource-view/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("Inventory/resource observations converged; both snapshots committed."),
        "{output}"
    );
}

fn serve_converged_inventory(listener: TcpListener, inventory: Value, resource: Value) {
    serve_conversation_page(&listener, &conversation_page(1));

    let (mut inventory_response, request, headers, body) = accept_request(&listener);
    assert!(
        request.starts_with("GET /api/v1/devices/observations/v2 "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    respond(&mut inventory_response, "200 OK", &inventory);

    let (mut resource_response, request, headers, body) = accept_request(&listener);
    assert!(
        request.starts_with("GET /api/v1/client-instances/resource-view "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    respond(&mut resource_response, "200 OK", &resource);
}

#[tokio::test]
async fn remote_tui_inventory_convergence_reconciles_revoked_instance_selection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    let mut revoked_resource = resource.clone();
    for instance in revoked_resource["instances"].as_array_mut().unwrap() {
        if instance["instance_id"] == "client-web-001" {
            instance["session_ids"] = json!([]);
        }
    }
    let page = json!({
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
    });
    let server = thread::spawn(move || {
        serve_revoked_selection(listener, inventory, resource, revoked_resource, page)
    });

    let client = test_client(address);
    let mut reader = Cursor::new(
        "inventory show-converged\ninstance client-web-001\nopen conversation-001\ninventory show-converged\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_revoked_selection_output(&output);
}

fn serve_revoked_selection(
    listener: TcpListener,
    inventory: Value,
    resource: Value,
    revoked_resource: Value,
    page: Value,
) {
    serve_conversation_page(&listener, &page);

    let (mut initial_inventory, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    assert!(body.is_empty());
    respond(&mut initial_inventory, "200 OK", &inventory);

    let (mut initial_resource, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut initial_resource, "200 OK", &resource);

    let (mut history, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversations/conversation-001/prompts?limit=128 "));
    assert!(body.is_empty());
    respond(
        &mut history,
        "200 OK",
        &json!({
            "conversation_id": "conversation-001",
            "prompts": [{
                "id": "prompt-before-revocation",
                "conversation_id": "conversation-001",
                "role": "user",
                "content": "private prompt before refresh",
                "created_at_ms": 10
            }],
            "has_more": false
        }),
    );

    let (mut refreshed_inventory, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    assert!(body.is_empty());
    respond(&mut refreshed_inventory, "200 OK", &inventory);

    let (mut refreshed_resource, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut refreshed_resource, "200 OK", &revoked_resource);
}

fn assert_revoked_selection_output(output: &str) {
    assert!(
        output.contains("Opened session \"conversation-001\""),
        "{output}"
    );
    assert!(output.contains("private prompt before refresh"), "{output}");
    let last_render = output
        .rsplit("Forge shared sessions")
        .next()
        .unwrap_or_default();
    assert!(
        last_render.contains("Client-instance filter: \"client-web-001\""),
        "{last_render}"
    );
    assert!(
        last_render.contains("No sessions match this client-instance filter"),
        "{last_render}"
    );
    assert!(!last_render.contains("Prompt history for"), "{last_render}");
    assert!(
        !last_render.contains("private prompt before refresh"),
        "{last_render}"
    );
}

#[tokio::test]
async fn remote_tui_show_converged_rejects_inventory_resource_drift_without_partial_commit() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let mut resource = pair["resource_view"].clone();
    resource["devices"][0]["heartbeat_sequence"] = json!(99);
    let server = thread::spawn(move || serve_inventory_drift(listener, inventory, resource));

    let client = test_client(address);
    let mut reader = Cursor::new("inventory show-converged\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Remote inventory/resource convergence request failed: Forge API inventory/resource observations did not converge"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "Previous inventory/resource snapshots were retained; no mixed pair was committed."
        ),
        "{output}"
    );
    assert!(!output.contains("remote device inventory ["), "{output}");
    assert!(
        !output.contains("remote client-instance/resource-view ["),
        "{output}"
    );
}

fn serve_inventory_drift(listener: TcpListener, inventory: Value, resource: Value) {
    serve_conversation_page(&listener, &conversation_page(1));

    let (mut inventory_response, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    assert!(body.is_empty());
    respond(&mut inventory_response, "200 OK", &inventory);

    let (mut resource_response, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut resource_response, "200 OK", &resource);
}
