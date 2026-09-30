use super::*;

#[tokio::test]
async fn unfiltered_execution_consent_preview_keeps_the_exact_candidate_read() {
    let response = execution_consent_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/execution-consents ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_execution_consent_preview(&client, "conversation-001", None, None)
        .await
        .expect("unfiltered execution-consent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_execution_consent_preview_reads_converged_pair_before_candidate_get() {
    let response = execution_consent_response();
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: resource_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/conversation-001/execution-consents ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_execution_consent_preview(
        &client,
        "conversation-001",
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance execution-consent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_execution_consent_preview_is_rejected_before_candidate_get() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: resource_view(),
        },
    ]);
    let error = execute_execution_consent_preview(
        &client,
        "conversation-002",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no execution-consent request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn drifted_instance_execution_consent_preview_is_rejected_before_candidate_get() {
    let mut drifted_resource = resource_view();
    drifted_resource["instances"][4]["observed_at_ms"] = json!(200501);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let error = execute_execution_consent_preview(
        &client,
        "conversation-001",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("resource drift must block the candidate read");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_execution_consent_preview_uses_view_without_candidate_observations() {
    let response = execution_consent_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-001/execution-consents ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_execution_consent_preview(
        &client,
        "conversation-001",
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local view should permit visible execution-consent preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}
