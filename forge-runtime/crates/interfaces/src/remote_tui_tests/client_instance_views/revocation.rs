use super::*;

#[tokio::test]
async fn remote_tui_can_revoke_one_client_instance_reader_without_broadening_the_filter() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let server = thread::spawn(move || serve_reader_revocation(listener, response));

    let client = test_client(address);
    let mut reader = Cursor::new(
        "client-instances session-view\ninstance client-web-001\nclient-instances clear session-view\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Client-instance/session-view cleared; any active instance filter remains fail-closed"
        ),
        "{output}"
    );
    let last_render = output
        .rsplit("Forge shared sessions")
        .next()
        .unwrap_or_default();
    assert!(last_render.contains("Client-instance filter: \"client-web-001\""));
    assert!(last_render.contains("No sessions match this client-instance filter"));
    assert!(!last_render.contains("conversation-001"));
}

fn serve_reader_revocation(listener: TcpListener, response: Value) {
    serve_conversation_page(
        &listener,
        &json!({
            "conversations": [{
                "conversation": {
                    "id": "conversation-001",
                    "scope": {"kind": "global"},
                    "title": "Web session",
                    "created_at_ms": 1,
                    "updated_at_ms": 1
                },
                "aggregate_version": 1
            }],
            "next_after_id": null,
            "has_more": false
        }),
    );
    let (mut stream, request, headers, body) = accept_request(&listener);
    assert!(
        request.starts_with("GET /api/v1/client-instances/session-view "),
        "{request}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert!(body.is_empty());
    respond(&mut stream, "200 OK", &response);
}

#[tokio::test]
async fn remote_tui_clears_a_stale_client_instance_view_after_refresh_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let page = json!({
        "conversations": [{
            "conversation": {
                "id": "conversation-001",
                "scope": {"kind": "global"},
                "title": "Web session",
                "created_at_ms": 1,
                "updated_at_ms": 1
            },
            "aggregate_version": 1
        }],
        "next_after_id": null,
        "has_more": false
    });
    let server = thread::spawn(move || serve_stale_view(listener, response, page));

    let client = test_client(address);
    let mut reader =
        Cursor::new("client-instances session-view\ninstance client-web-001\nsync\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_stale_view_output(&output);
}

fn serve_stale_view(listener: TcpListener, response: Value, page: Value) {
    serve_conversation_page(&listener, &page);
    let (mut initial_view, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    respond(&mut initial_view, "200 OK", &response);

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
    let (mut failed_view, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    respond(
        &mut failed_view,
        "503 Service Unavailable",
        &json!({"code": "temporarily_unavailable"}),
    );
}

fn assert_stale_view_output(output: &str) {
    assert!(
        output.contains("Previous client-instance observations were retained for display only"),
        "{output}"
    );
    assert!(
        output.contains(
            "Client-instance observations are not converged; instance filtering and private reads through this client-instance projection are blocked"
        ),
        "{output}"
    );
    let last_render = output
        .rsplit("Forge shared sessions")
        .next()
        .unwrap_or_default();
    assert!(last_render.contains("Client-instance filter: \"client-web-001\""));
    assert!(last_render.contains("No sessions match this client-instance filter"));
    assert!(!last_render.contains("conversation-001"));
}
