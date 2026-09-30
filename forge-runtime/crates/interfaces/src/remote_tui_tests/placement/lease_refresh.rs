use super::*;

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_refreshes_explicit_inventory_resource_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, resource) = canonical_inventory_resource_pair();
    let server_inventory = inventory.clone();
    let server_resource = resource.clone();
    let server =
        thread::spawn(move || serve_lease_refresh(listener, server_inventory, server_resource));

    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_LEASE_INPUT.replace("conversation-1", "conversation-001"),
    )
    .unwrap();
    let mut state = visible_scheduler_lease_state(inventory, resource);
    let client = test_client(address);
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "scheduler-selection-lease --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("scheduler lease"), "{output}");
    assert!(output.contains("lease_issued=true"), "{output}");
    assert!(output.contains("fencing token withheld"), "{output}");
}

fn serve_lease_refresh(listener: TcpListener, server_inventory: Value, server_resource: Value) {
    let (mut inventory_response, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    assert!(body.is_empty());
    respond(&mut inventory_response, "200 OK", &server_inventory);

    let (mut resource_response, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut resource_response, "200 OK", &server_resource);

    let (mut lease, request, headers, body) = accept_request(&listener);
    assert!(request.starts_with("POST /api/v1/device-placement/scheduler-lease "));
    assert!(headers.contains("idempotency-key: forge-tui-"));
    let posted: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(posted["conversation_id"], "conversation-001");
    respond(
        &mut lease,
        "200 OK",
        &scheduler_lease_response("conversation-001", 1, "fence-token-a"),
    );
}

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_renewal_drift_blocks_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, resource) = canonical_inventory_resource_pair();
    let mut drifted_resource = resource.clone();
    drifted_resource["devices"][0]["heartbeat_sequence"] = json!(99);
    let server_inventory = inventory.clone();
    let server_drifted_resource = drifted_resource;
    let server = thread::spawn(move || {
        serve_renewal_drift(listener, server_inventory, server_drifted_resource)
    });

    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_LEASE_RENEWAL_INPUT.replace("conversation-1", "conversation-001"),
    )
    .unwrap();
    let mut state = visible_scheduler_lease_state(inventory, resource);
    let client = test_client(address);
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "scheduler-selection-lease-renew --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Scheduler lease renewal inventory/resource refresh failed"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
}

fn serve_renewal_drift(
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
                panic!("scheduler lease renewal POST escaped inventory/resource drift: {request}");
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("checking for unexpected scheduler lease request: {error}"),
        }
    }
}
