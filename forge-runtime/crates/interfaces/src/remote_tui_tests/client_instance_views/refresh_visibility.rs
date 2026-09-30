use super::*;

#[tokio::test]
async fn remote_tui_sync_refreshes_explicitly_opened_client_instance_views() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let session_response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let resource_response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    let server =
        thread::spawn(move || serve_sync_views(listener, session_response, resource_response));

    let client = test_client(address);
    let mut reader =
        Cursor::new("client-instances session-view\nclient-instances resource-view\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_sync_views_output(&output);
}

fn serve_sync_views(listener: TcpListener, session_response: Value, resource_response: Value) {
    serve_initial_views_and_owner_changes(&listener, &session_response, &resource_response);
    serve_history_and_refreshed_views(&listener, &session_response, &resource_response);
}

fn serve_initial_views_and_owner_changes(
    listener: &TcpListener,
    session_response: &Value,
    resource_response: &Value,
) {
    serve_conversation_page(listener, &conversation_page(1));

    let (mut initial_session, request, headers, body) = accept_request(listener);
    assert!(
        request.starts_with("GET /api/v1/client-instances/session-view "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    respond(&mut initial_session, "200 OK", session_response);

    let (mut initial_resource, request, headers, body) = accept_request(listener);
    assert!(
        request.starts_with("GET /api/v1/client-instances/resource-view "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    respond(&mut initial_resource, "200 OK", resource_response);

    let (mut changes, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
    respond(
        &mut changes,
        "200 OK",
        &serde_json::json!({
            "after_cursor": 0,
            "scanned_through_cursor": 0,
            "has_more": false,
            "changes": []
        }),
    );

    serve_conversation_page(listener, &conversation_page(1));

    let (mut history, request, _, _) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &serde_json::json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    );
}

fn serve_history_and_refreshed_views(
    listener: &TcpListener,
    session_response: &Value,
    resource_response: &Value,
) {
    let (mut refreshed_session, request, headers, body) = accept_request(listener);
    assert!(
        request.starts_with("GET /api/v1/client-instances/session-view "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    respond(&mut refreshed_session, "200 OK", session_response);

    let (mut refreshed_resource, request, headers, body) = accept_request(listener);
    assert!(
        request.starts_with("GET /api/v1/client-instances/resource-view "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    respond(&mut refreshed_resource, "200 OK", resource_response);
}

fn assert_sync_views_output(output: &str) {
    assert_eq!(
        output
            .matches("remote client-instance/session-view [forge.client-instance-session-view/v1]")
            .count(),
        2,
        "{output}"
    );
    assert_eq!(
        output
            .matches(
                "remote client-instance/resource-view [forge.client-instance-resource-view/v1]"
            )
            .count(),
        2,
        "{output}"
    );
    assert!(
        output.contains("Selected client-instance/session-view refreshed."),
        "{output}"
    );
    assert!(
        output.contains("Selected client-instance/resource-view refreshed."),
        "{output}"
    );
    assert!(
        output.contains("Synced 0 owner-visible changes through cursor 0"),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_refreshes_selected_instance_before_private_session_reads() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let session_response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let mut revoked_session_response = session_response.clone();
    let instances = revoked_session_response["instances"]
        .as_array_mut()
        .unwrap();
    for instance in instances {
        if instance["instance_id"] == json!("client-web-001") {
            instance["session_ids"] = json!([]);
        }
    }
    let server = thread::spawn(move || {
        serve_selected_visibility(listener, session_response, revoked_session_response)
    });

    let client = test_client(address);
    let mut reader =
        Cursor::new("client-instances session-view\ninstance client-web-001\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(!output.contains("Prompt history refreshed."), "{output}");
    assert!(!output.contains("Sync Run timeline refresh"), "{output}");
    assert!(
        output.contains("No sessions match this client-instance filter"),
        "{output}"
    );
}

fn serve_selected_visibility(
    listener: TcpListener,
    session_response: Value,
    revoked_session_response: Value,
) {
    // Initial owner snapshot used by the TUI before the explicit
    // client-instance projection is opened.
    serve_conversation_page(&listener, &conversation_page(1));

    let (mut initial_session, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut initial_session, "200 OK", &session_response);

    let (mut changes, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 "));
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

    // The selected instance is revoked before the owner Conversation
    // snapshot. A stale implementation would request the snapshot and
    // then read Prompt history under the old declaration.
    let (mut refreshed_session, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut refreshed_session, "200 OK", &revoked_session_response);

    serve_conversation_page(&listener, &conversation_page(1));
}
