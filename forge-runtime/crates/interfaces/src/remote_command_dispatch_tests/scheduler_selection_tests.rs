use super::*;

#[tokio::test]
async fn unfiltered_scheduler_selection_preview_keeps_the_exact_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let response = scheduler_selection_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        None,
        None,
    )
    .await
    .expect("unfiltered scheduler selection preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_scheduler_selection_preview_reads_inventory_after_pair_before_candidate_post()
 {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let response = scheduler_selection_response();
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
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "200 OK",
            response: inventory_resource_convergence()["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: inventory_resource_convergence()["resource_view"].clone(),
        },
        ExpectedRequest {
            request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance scheduler selection preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_scheduler_selection_preview_is_rejected_before_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    let mut request = scheduler_selection_request();
    request["conversation_id"] = json!("conversation-002");
    serde_json::to_writer(input.as_file(), &request).expect("write scheduler input");
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
    let error = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("hidden Conversation must be rejected locally");
    assert!(
        error
            .to_string()
            .contains("no scheduler-selection request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn drifted_instance_scheduler_selection_preview_is_rejected_before_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
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
    let error = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("resource drift must block the candidate post");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_scheduler_selection_preview_is_rejected_before_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let pair = inventory_resource_convergence();
    let mut drifted_resource = pair["resource_view"].clone();
    drifted_resource["devices"][0]["heartbeat_sequence"] = json!(99);
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
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "200 OK",
            response: pair["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            response_status: "200 OK",
            response: drifted_resource,
        },
    ]);
    let error = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory/resource drift must block the candidate post");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_scheduler_selection_preview_uses_view_without_candidate_observations() {
    let input = tempfile::NamedTempFile::new().expect("scheduler input");
    serde_json::to_writer(input.as_file(), &scheduler_selection_request())
        .expect("write scheduler input");
    let response = scheduler_selection_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_preview(
        &client,
        &input.path().display().to_string(),
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local view should permit visible scheduler selection preview");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}
