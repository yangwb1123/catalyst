use super::*;

#[tokio::test]
async fn remote_tui_sync_refreshes_an_explicitly_opened_inventory_view() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
    ))
    .unwrap();
    let mut refreshed_fixture = fixture.clone();
    refreshed_fixture["evaluated_at_ms"] = json!(300_000);
    refreshed_fixture["devices"][0]["revision"] = json!(2);
    refreshed_fixture["devices"][0]["heartbeat_sequence"] = json!(2);
    refreshed_fixture["devices"][0]["device"]["available_cpu_cores"] = json!(3);
    let server =
        thread::spawn(move || serve_inventory_v2_refresh(listener, fixture, refreshed_fixture));

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read-v2\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_eq!(
        output
            .matches("remote device inventory [forge.device-inventory-observation/v2]")
            .count(),
        2,
        "{output}"
    );
    assert!(output.contains("at 200000 devices=2"), "{output}");
    assert!(output.contains("at 300000 devices=2"), "{output}");
    assert!(
        output.contains(
            "device-a / runner-a: revision=2 generation=1 heartbeat=2 reservation=reserved cpu=3"
        ),
        "{output}"
    );
    assert!(output.contains("Selected inventory observation refreshed."));
    assert!(output.contains("Synced 0 owner-visible changes through cursor 0"));
}

fn serve_inventory_v2_refresh(listener: TcpListener, fixture: Value, refreshed_fixture: Value) {
    serve_conversation_page(&listener, &conversation_page(1));

    let (mut initial_inventory, request, headers, body) =
        super::super::helpers::accept_request(&listener);
    assert!(
        request.starts_with("GET /api/v1/devices/observations/v2 "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    super::super::helpers::respond(&mut initial_inventory, "200 OK", &fixture);

    let (mut changes, request, _, _) = super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
    super::super::helpers::respond(
        &mut changes,
        "200 OK",
        &json!({
            "after_cursor": 0,
            "scanned_through_cursor": 0,
            "has_more": false,
            "changes": []
        }),
    );

    serve_conversation_page(&listener, &conversation_page(1));

    let (mut history, request, _, _) = super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    super::super::helpers::respond(
        &mut history,
        "200 OK",
        &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    );

    let (mut refreshed_inventory, request, headers, body) =
        super::super::helpers::accept_request(&listener);
    assert!(
        request.starts_with("GET /api/v1/devices/observations/v2 "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    super::super::helpers::respond(&mut refreshed_inventory, "200 OK", &refreshed_fixture);
}

#[tokio::test]
async fn remote_tui_retains_previous_pair_until_inventory_resource_refresh_converges() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    let mut inventory_revision_two = inventory.clone();
    inventory_revision_two["devices"][0]["revision"] = json!(2);
    inventory_revision_two["devices"][0]["heartbeat_sequence"] = json!(2);
    let mut resource_revision_three = resource.clone();
    resource_revision_three["devices"][0]["revision"] = json!(3);
    resource_revision_three["devices"][0]["heartbeat_sequence"] = json!(3);
    let mut inventory_revision_three = inventory_revision_two.clone();
    inventory_revision_three["devices"][0]["revision"] = json!(3);
    inventory_revision_three["devices"][0]["heartbeat_sequence"] = json!(3);
    let server = thread::spawn(move || {
        serve_converging_inventory(
            listener,
            inventory,
            resource,
            inventory_revision_two,
            resource_revision_three,
            inventory_revision_three,
        )
    });

    let client = test_client(address);
    let mut reader =
        Cursor::new("inventory read-v2\nclient-instances resource-view\nsync\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_converging_inventory_output(&output);
}

fn serve_converging_inventory(
    listener: TcpListener,
    inventory: Value,
    resource: Value,
    inventory_revision_two: Value,
    resource_revision_three: Value,
    inventory_revision_three: Value,
) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut initial_inventory, request, _, _) = super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    super::super::helpers::respond(&mut initial_inventory, "200 OK", &inventory);
    let (mut initial_resource, request, _, _) = super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    super::super::helpers::respond(&mut initial_resource, "200 OK", &resource);

    for (next_inventory, next_resource) in [
        (
            inventory_revision_two.clone(),
            resource_revision_three.clone(),
        ),
        (inventory_revision_three, resource_revision_three),
    ] {
        serve_inventory_refresh_pair(&listener, &next_inventory, &next_resource);
    }
}

#[tokio::test]
async fn remote_tui_sync_refreshes_an_explicitly_opened_v1_inventory_view() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json"
    ))
    .unwrap();
    let mut refreshed_fixture = fixture.clone();
    refreshed_fixture["evaluated_at_ms"] = json!(300_000);
    refreshed_fixture["devices"][0]["device"]["liveness"] = json!("offline");
    refreshed_fixture["devices"][0]["device"]["available_cpu_cores"] = json!(2);
    let server =
        thread::spawn(move || serve_inventory_v1_refresh(listener, fixture, refreshed_fixture));

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_eq!(
        output
            .matches("remote device inventory [forge.device-inventory-observation/v1]")
            .count(),
        2,
        "{output}"
    );
    assert!(
        output
            .contains("remote device inventory [forge.device-inventory-observation/v1] at 200000"),
        "{output}"
    );
    assert!(
        output
            .contains("remote device inventory [forge.device-inventory-observation/v1] at 300000"),
        "{output}"
    );
    assert!(
        output
            .contains("device-a / runner-a: approval=approved cordon=clear liveness=offline cpu=2"),
        "{output}"
    );
    assert!(output.contains("Selected inventory observation refreshed."));
    assert!(output.contains("Synced 0 owner-visible changes through cursor 0"));
}

fn serve_inventory_v1_refresh(listener: TcpListener, fixture: Value, refreshed_fixture: Value) {
    serve_conversation_page(&listener, &conversation_page(1));

    let (mut initial_inventory, request, headers, body) =
        super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices "), "{request}");
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    super::super::helpers::respond(&mut initial_inventory, "200 OK", &fixture);

    let (mut changes, request, _, _) = super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
    super::super::helpers::respond(
        &mut changes,
        "200 OK",
        &json!({
            "after_cursor": 0,
            "scanned_through_cursor": 0,
            "has_more": false,
            "changes": []
        }),
    );

    serve_conversation_page(&listener, &conversation_page(1));

    let (mut history, request, _, _) = super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    super::super::helpers::respond(
        &mut history,
        "200 OK",
        &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    );

    let (mut refreshed_inventory, request, headers, body) =
        super::super::helpers::accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices "), "{request}");
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    super::super::helpers::respond(&mut refreshed_inventory, "200 OK", &refreshed_fixture);
}

fn assert_converging_inventory_output(output: &str) {
    assert!(
        output.contains(
            "Inventory/resource observations did not converge; previous snapshots were retained"
        ),
        "{output}"
    );
    assert_eq!(
        output
            .matches("Synced 0 owner-visible changes through cursor 0")
            .count(),
        1,
        "{output}"
    );
}

fn serve_inventory_refresh_pair(
    listener: &TcpListener,
    next_inventory: &Value,
    next_resource: &Value,
) {
    let (mut changes, request, _, _) = super::super::helpers::accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
    super::super::helpers::respond(
        &mut changes,
        "200 OK",
        &json!({
            "after_cursor": 0,
            "scanned_through_cursor": 0,
            "has_more": false,
            "changes": []
        }),
    );
    serve_conversation_page(listener, &conversation_page(1));
    let (mut history, request, _, _) = super::super::helpers::accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    super::super::helpers::respond(
        &mut history,
        "200 OK",
        &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    );
    let (mut refreshed_inventory, request, _, _) = super::super::helpers::accept_request(listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    super::super::helpers::respond(&mut refreshed_inventory, "200 OK", next_inventory);
    let (mut refreshed_resource, request, _, _) = super::super::helpers::accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    super::super::helpers::respond(&mut refreshed_resource, "200 OK", next_resource);
}
