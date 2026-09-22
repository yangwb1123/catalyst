use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn remote_tui_can_read_authenticated_client_instance_session_view_candidate() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/client-instances/session-view "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &response);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("client-instances session-view\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "remote client-instance/session-view [forge.client-instance-session-view/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("instance client-cli-001:"), "{output}");
    assert!(output.contains("instance client-web-001:"), "{output}");
    assert!(
        output.contains("authority: owner_authenticated=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_can_read_authenticated_client_instance_resource_view_candidate() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/client-instances/resource-view "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &response);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("client-instances resource-view\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "remote client-instance/resource-view [forge.client-instance-resource-view/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("device device-a: runner=runner-a"),
        "{output}"
    );
    assert!(
        output.contains("device device-b: runner=runner-b"),
        "{output}"
    );
    assert!(
        output.contains("authority: owner_authenticated=false"),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_sync_refreshes_explicitly_opened_client_instance_views() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let session_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let resource_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut initial_session, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/client-instances/session-view "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut initial_session, "200 OK", &session_response);

        let (mut initial_resource, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/client-instances/resource-view "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut initial_resource, "200 OK", &resource_response);

        let (mut changes, request, _, _) = accept_request(&listener);
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

        serve_conversation_page(&listener, &conversation_page(1));

        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &serde_json::json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        );

        let (mut refreshed_session, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/client-instances/session-view "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut refreshed_session, "200 OK", &session_response);

        let (mut refreshed_resource, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/client-instances/resource-view "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut refreshed_resource, "200 OK", &resource_response);
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
async fn remote_tui_can_revoke_one_client_instance_reader_without_broadening_the_filter() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
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
    });

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

#[tokio::test]
async fn remote_tui_clears_a_stale_client_instance_view_after_refresh_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
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
    let server = thread::spawn(move || {
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
        serve_conversation_page(&listener, &page);
        let (mut history, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/conversation-001/prompts?"));
        respond(
            &mut history,
            "200 OK",
            &json!({"conversation_id": "conversation-001", "prompts": [], "has_more": false}),
        );
        let (mut failed_view, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        respond(
            &mut failed_view,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
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
    assert!(
        output.contains("Local client-instance/session-view view cleared after refresh failure."),
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
