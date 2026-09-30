use super::*;

#[tokio::test]
async fn visible_instance_scheduler_lease_claim_reads_converged_pair_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
    let response = scheduler_lease_response();
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
            request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-instance-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance scheduler lease claim");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn visible_instance_scheduler_lease_renewal_reads_inventory_after_pair_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
    let mut response = scheduler_lease_response();
    response["grant"]["epoch"] = json!(2);
    response["grant"]["fencing_token"] = json!("fence-token-b");
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
            request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
            response_status: "200 OK",
            response: response.clone(),
        },
    ]);
    let returned = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-instance-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect("visible instance scheduler lease renewal");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_visible_instance_scheduler_lease_claim_is_rejected_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
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
    let error = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-drift-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory/resource drift must block the lease claim POST");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_drifted_visible_instance_scheduler_lease_renewal_is_rejected_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
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
    let error = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-drift-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory/resource drift must block the lease renewal POST");
    assert!(error.to_string().contains("did not converge"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_read_failure_blocks_visible_instance_scheduler_lease_claim_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
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
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
    ]);
    let error = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-read-failure-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory read failure must block the lease claim POST");
    assert!(error.to_string().contains("Forge API returned HTTP 503"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn inventory_read_failure_blocks_visible_instance_scheduler_lease_renewal_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
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
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            response_status: "503 Service Unavailable",
            response: json!({"code": "temporarily_unavailable"}),
        },
    ]);
    let error = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-read-failure-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("inventory read failure must block the lease renewal POST");
    assert!(error.to_string().contains("Forge API returned HTTP 503"));
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_view_scheduler_lease_claim_keeps_the_single_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-001"),
    )
    .expect("write scheduler lease input");
    let response = scheduler_lease_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-local-view-1",
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local instance view scheduler lease claim");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn local_instance_view_scheduler_lease_renewal_keeps_the_single_candidate_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease renewal input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_renewal_request("conversation-001"),
    )
    .expect("write scheduler lease renewal input");
    let mut response = scheduler_lease_response();
    response["grant"]["epoch"] = json!(2);
    response["grant"]["fencing_token"] = json!("fence-token-b");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_lease_renewal(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-renew-local-view-1",
        Some("client-web-001"),
        Some(&session_view_path()),
    )
    .await
    .expect("local instance view scheduler lease renewal");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}

#[tokio::test]
async fn hidden_instance_scheduler_lease_claim_is_rejected_before_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease input");
    serde_json::to_writer(
        input.as_file(),
        &scheduler_lease_request("conversation-002"),
    )
    .expect("write scheduler lease input");
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
    let error = execute_scheduler_selection_lease(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-hidden-1",
        Some("client-web-001"),
        None,
    )
    .await
    .expect_err("hidden Conversation must be rejected before the lease POST");
    assert!(
        error
            .to_string()
            .contains("no scheduler lease request was sent")
    );
    server.join().expect("mock server");
}

#[tokio::test]
async fn unfiltered_scheduler_lease_release_preserves_legacy_post() {
    let input = tempfile::NamedTempFile::new().expect("scheduler lease release input");
    let request = scheduler_lease_release_request();
    serde_json::to_writer(input.as_file(), &request).expect("write scheduler lease release input");
    let response = scheduler_lease_release_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/release ",
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = execute_scheduler_selection_lease_release(
        &client,
        &input.path().display().to_string(),
        "scheduler-lease-release-legacy-1",
        None,
        None,
    )
    .await
    .expect("unfiltered scheduler lease release");
    assert_eq!(returned, response);
    server.join().expect("mock server");
}
