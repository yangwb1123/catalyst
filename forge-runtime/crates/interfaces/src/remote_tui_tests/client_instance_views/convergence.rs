use super::*;

#[tokio::test]
async fn remote_tui_show_converged_reads_and_commits_the_paired_client_instance_views() {
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
        thread::spawn(move || serve_converged_pair(listener, session_response, resource_response));

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
    assert_converged_pair_output(&output);
}

fn serve_converged_pair(listener: TcpListener, session_response: Value, resource_response: Value) {
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
}

fn assert_converged_pair_output(output: &str) {
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
        "../../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let mut resource_response: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .unwrap();
    resource_response["instances"][0]["observed_at_ms"] = json!(200501);
    let server =
        thread::spawn(move || serve_drifted_pair(listener, session_response, resource_response));

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

fn serve_drifted_pair(listener: TcpListener, session_response: Value, resource_response: Value) {
    serve_conversation_page(&listener, &conversation_page(1));

    let (mut session, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
    assert!(body.is_empty());
    respond(&mut session, "200 OK", &session_response);

    let (mut resource, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
    assert!(body.is_empty());
    respond(&mut resource, "200 OK", &resource_response);
}

#[tokio::test]
async fn remote_tui_show_converged_clears_the_pair_after_authorization_failure() {
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
    let server = thread::spawn(move || {
        serve_unauthorized_pair(listener, session_response, resource_response)
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

fn serve_unauthorized_pair(
    listener: TcpListener,
    session_response: Value,
    resource_response: Value,
) {
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
}
