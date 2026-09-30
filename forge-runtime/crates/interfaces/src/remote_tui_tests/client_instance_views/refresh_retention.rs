use super::*;

#[tokio::test]
async fn remote_tui_keeps_the_previous_client_instance_pair_when_the_second_refresh_fails() {
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
    let mut newer_session = session_response.clone();
    newer_session["instances"][0]["observed_at_ms"] = json!(200501);
    let server = thread::spawn(move || {
        serve_failed_pair_refresh(listener, session_response, resource_response, newer_session)
    });

    let client = test_client(address);
    let mut reader =
        Cursor::new("client-instances session-view\nclient-instances resource-view\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Previous client-instance observations were retained for display only"),
        "{output}"
    );
    assert!(!output.contains("observed_at_ms=200501"), "{output}");
    assert!(
        output.contains("Client-instance session/resource observations did not converge")
            || output.contains("Sync client-instance/resource-view refresh failed"),
        "{output}"
    );
}

fn serve_failed_pair_refresh(
    listener: TcpListener,
    session_response: Value,
    resource_response: Value,
    newer_session: Value,
) {
    serve_initial_pair_and_failed_refresh(
        &listener,
        &session_response,
        &resource_response,
        &newer_session,
    );
    serve_owner_after_failed_refresh(&listener);
}

fn serve_initial_pair_and_failed_refresh(
    listener: &TcpListener,
    session_response: &Value,
    resource_response: &Value,
    newer_session: &Value,
) {
    serve_conversation_page(listener, &conversation_page(1));

    let (mut initial_session, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut initial_session, "200 OK", session_response);

    let (mut initial_resource, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut initial_resource, "200 OK", resource_response);

    let (mut changes, request, _, body) = accept_request(listener);
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
    serve_conversation_page(listener, &conversation_page(1));

    let (mut history, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    assert!(body.is_empty());
    respond(
        &mut history,
        "200 OK",
        &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    );

    let (mut refreshed_session, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut refreshed_session, "200 OK", newer_session);
}

fn serve_owner_after_failed_refresh(listener: &TcpListener) {
    let (mut failed_resource, request, _, body) = accept_request(listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(
        &mut failed_resource,
        "503 Service Unavailable",
        &json!({"code": "temporarily_unavailable"}),
    );
}

#[tokio::test]
async fn remote_tui_retains_previous_client_instance_pair_until_refresh_converges() {
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
    let mut session_refresh = session_response.clone();
    for instance in session_refresh["instances"].as_array_mut().unwrap() {
        instance["observed_at_ms"] = json!(200501);
    }
    let mut resource_refresh = resource_response.clone();
    for instance in resource_refresh["instances"].as_array_mut().unwrap() {
        instance["observed_at_ms"] = json!(200502);
    }
    let converged_session = session_refresh.clone();
    let mut converged_resource = resource_refresh.clone();
    for instance in converged_resource["instances"].as_array_mut().unwrap() {
        instance["observed_at_ms"] = json!(200501);
    }

    let server = thread::spawn(move || {
        serve_converging_refresh(
            listener,
            session_response,
            resource_response,
            session_refresh,
            resource_refresh,
            converged_session,
            converged_resource,
        )
    });

    let client = test_client(address);
    let mut reader = Cursor::new(
        "client-instances session-view\nclient-instances resource-view\nsync\nsync\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_converging_refresh_output(&output);
}

fn serve_converging_refresh(
    listener: TcpListener,
    session_response: Value,
    resource_response: Value,
    session_refresh: Value,
    resource_refresh: Value,
    converged_session: Value,
    converged_resource: Value,
) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut initial_session, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    respond(&mut initial_session, "200 OK", &session_response);
    let (mut initial_resource, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    respond(&mut initial_resource, "200 OK", &resource_response);

    for (next_session, next_resource) in [
        (session_refresh.clone(), resource_refresh),
        (converged_session, converged_resource),
    ] {
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
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        );
        let (mut refreshed_session, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        respond(&mut refreshed_session, "200 OK", &next_session);
        let (mut refreshed_resource, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        respond(&mut refreshed_resource, "200 OK", &next_resource);
    }
}

fn assert_converging_refresh_output(output: &str) {
    assert!(
        output.contains(
            "Previous client-instance observations were retained for display only; no mixed pair was committed"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "Client-instance observations are not converged; instance filtering and private reads through this client-instance projection are blocked"
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
