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
async fn remote_tui_show_converged_reads_and_commits_the_paired_client_instance_views() {
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

        let (mut session, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_response);

        let (mut resource, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut resource, "200 OK", &resource_response);
    });

    let client = test_client(address);
    let mut reader = Cursor::new(
        "client-instances show-converged\ninstance client-web-001\ninstance list\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "remote client-instance/session-resource-convergence [forge.client-instance-session-resource-convergence/v1] converged=true read_only=true"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "remote client-instance/session-view [forge.client-instance-session-view/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "remote client-instance/resource-view [forge.client-instance-resource-view/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "Client-instance session/resource observations converged; both snapshots committed."
        ),
        "{output}"
    );
    assert!(output.contains("Client-instance filter set to \"client-web-001\""));
    assert!(output.contains("* client-web-001"), "{output}");
}

#[tokio::test]
async fn remote_tui_show_converged_rejects_drift_without_committing_a_partial_pair() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let session_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let mut resource_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    resource_response["instances"][0]["observed_at_ms"] = json!(200501);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_response);

        let (mut resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource, "200 OK", &resource_response);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("client-instances show-converged\ninstance list\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Remote client-instance session/resource convergence request failed: Forge API client-instance session/resource observations did not converge"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "Previous client-instance snapshots were retained; no mixed pair was committed."
        ),
        "{output}"
    );
    assert!(
        output.contains("No client-instance view is open. Use client-instances session-view or resource-view first."),
        "{output}"
    );
    assert!(
        !output.contains("remote client-instance/session-view ["),
        "{output}"
    );
    assert!(
        !output.contains("remote client-instance/resource-view ["),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_show_converged_clears_the_pair_after_authorization_failure() {
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

        let (mut session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_response);
        let (mut resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource, "200 OK", &resource_response);

        let (mut unauthorized, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(
            &mut unauthorized,
            "401 Unauthorized",
            &json!({"code": "unauthorized"}),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new(
        "client-instances show-converged\ninstance client-web-001\nclient-instances show-converged\ninstance list\nquit\n",
    );
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Local session view cleared after authorization failure."),
        "{output}"
    );
    assert!(
        output.contains(
            "No client-instance view is open. Use client-instances session-view or resource-view first."
        ),
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
async fn remote_tui_refreshes_selected_instance_before_private_session_reads() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let session_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
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

#[tokio::test]
async fn remote_tui_keeps_the_previous_client_instance_pair_when_the_second_refresh_fails() {
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
    let mut newer_session = session_response.clone();
    newer_session["instances"][0]["observed_at_ms"] = json!(200501);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));

        let (mut initial_session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut initial_session, "200 OK", &session_response);
        let (mut initial_resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut initial_resource, "200 OK", &resource_response);

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
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut history, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
        assert!(body.is_empty());
        respond(
            &mut history,
            "200 OK",
            &json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        );

        let (mut refreshed_session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut refreshed_session, "200 OK", &newer_session);
        let (mut failed_resource, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(
            &mut failed_resource,
            "503 Service Unavailable",
            &json!({"code": "temporarily_unavailable"}),
        );
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

#[tokio::test]
async fn remote_tui_retains_previous_client_instance_pair_until_refresh_converges() {
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
            assert!(
                request.starts_with("GET /api/v1/conversation-changes?after_cursor=0&limit=128 ")
            );
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
