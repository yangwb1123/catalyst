use super::*;

#[tokio::test]
async fn remote_tui_scheduler_selection_preview_accepts_a_declared_client_instance_conversation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut session_response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let web = session_response["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["instance_id"] == "client-web-001")
        .unwrap();
    web["session_ids"] = json!(["c-1", "conversation-1"]);
    let server = thread::spawn(move || serve_declared_conversation(listener, session_response));
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), SCHEDULER_SELECTION_INPUT).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances session-view\ninstance client-web-001\nscheduler-selection-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Client-instance filter set to \"client-web-001\""),
        "{output}"
    );
    assert!(output.contains("scheduler selection preview"), "{output}");
}

fn serve_declared_conversation(listener: TcpListener, session_response: Value) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut session, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut session, "200 OK", &session_response);

    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("POST /api/v1/device-placement/scheduler-preview "));
    let posted: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(posted["conversation_id"], "conversation-1");
    respond(
        &mut stream,
        "200 OK",
        &json!({
            "schema_version": "forge.scheduler-selection-preview/v1",
            "evaluation_mode": "pure_scheduler_selection_preview",
            "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "evaluated_at_ms": 1800000000000_i64,
            "candidate_count": 1, "eligible_candidate_count": 0,
            "selection_available": false, "selection_reason": "no_eligible_candidate",
            "selected_device_id": null, "selected_instance_id": null, "preview_only": true,
            "authority": {
                "placement_selected": false, "reservation_created": false, "lease_issued": false,
                "execution_authorized": false, "dispatch_performed": false, "audit_published": false
            }
        }),
    );
}

#[tokio::test]
async fn remote_tui_scheduler_selection_preview_rechecks_visibility_after_refresh() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut session: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    // The initial client-instance pair must be the same image; the final
    // refresh below is the only deliberate session/resource divergence.
    session["instances"] = resource["instances"].clone();
    let mut revoked_resource = resource.clone();
    revoked_resource["instances"][0]["session_ids"] = json!(["conversation-002"]);

    let server = thread::spawn(move || {
        serve_refreshed_visibility(listener, session, inventory, resource, revoked_resource)
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_INPUT.replace("conversation-1", "conversation-001"),
    )
    .unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances show-converged\ninstance client-web-001\ninventory show-converged\nscheduler-selection-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Conversation read blocked by client-instance display filter: no validated view is available"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
    assert!(
        !output.contains("scheduler selection preview ["),
        "{output}"
    );
}

fn serve_refreshed_visibility(
    listener: TcpListener,
    session: Value,
    inventory: Value,
    resource: Value,
    revoked_resource: Value,
) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", &session);

    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", &resource);

    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", &inventory);

    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", &resource);

    // The planning boundary refreshes the explicit pair. Its resource
    // image revokes this Conversation from the selected instance, so a
    // subsequent scheduler POST would be a fail-open.
    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", &inventory);

    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", &revoked_resource);
}

#[tokio::test]
async fn remote_tui_scheduler_selection_preview_blocks_a_foreign_client_instance_conversation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut session_response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let web = session_response["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["instance_id"] == "client-web-001")
        .unwrap();
    web["session_ids"] = json!(["c-1"]);
    let server = thread::spawn(move || serve_foreign_conversation(listener, session_response));
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_INPUT.replace("conversation-1", "conversation-2"),
    )
    .unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances session-view\ninstance client-web-001\nscheduler-selection-preview --input {}\nquit\n",
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
            "Conversation read blocked by client-instance display filter: conversation \"conversation-2\" is not declared"
        ),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
    assert!(
        !output.contains("scheduler selection preview ["),
        "{output}"
    );
}

fn serve_foreign_conversation(listener: TcpListener, session_response: Value) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut session, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut session, "200 OK", &session_response);
}
