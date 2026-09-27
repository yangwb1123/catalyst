use super::*;

fn session_view() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .expect("client-instance session view fixture")
}

fn resource_view() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json"
    ))
    .expect("client-instance resource view fixture")
}

fn session_resource_convergence() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-resource-convergence-v1.json"
    ))
    .expect("client-instance session/resource convergence fixture")
}

fn inventory_resource_convergence() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("device inventory/resource convergence fixture")
}

#[tokio::test]
async fn client_instance_session_view_read_sends_one_authenticated_get_without_a_body() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/session-view ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: session_view(),
    }]);
    let returned = client.read_client_instance_session_view().await.unwrap();
    assert_eq!(
        returned["schema_version"],
        "forge.client-instance-session-view/v1"
    );
    assert_eq!(returned["instances"][0]["instance_id"], "client-app-001");
    assert_eq!(returned["instances"][0]["client_kind"], "app");
    assert_eq!(returned["instances"][2]["client_kind"], "mobile");
    assert_eq!(returned["instances"][3]["client_kind"], "tui");
    assert_eq!(returned["instances"][4]["client_kind"], "web");
    assert_eq!(returned["authority"]["execution_authorized"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn client_instance_session_view_read_rejects_authority_mutation() {
    let mut response = session_view();
    response["authority"]["prompt_write_authorized"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/session-view ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .read_client_instance_session_view()
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid client-instance session view"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn client_instance_resource_view_read_sends_one_authenticated_get_without_a_body() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/resource-view ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: resource_view(),
    }]);
    let returned = client.read_client_instance_resource_view().await.unwrap();
    assert_eq!(
        returned["schema_version"],
        "forge.client-instance-resource-view/v1"
    );
    assert_eq!(returned["devices"][0]["device_id"], "device-a");
    assert_eq!(returned["authority"]["dispatch_performed"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn client_instance_resource_view_read_rejects_authority_mutation() {
    let mut response = resource_view();
    response["authority"]["reservation_created"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/client-instances/resource-view ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .read_client_instance_resource_view()
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid client-instance resource view"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn converged_inventory_resource_read_binds_two_authenticated_gets() {
    let pair = inventory_resource_convergence();
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: pair["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: pair["resource_view"].clone(),
        },
    ]);
    let returned = client
        .read_converged_inventory_resource_view()
        .await
        .unwrap();
    assert_eq!(
        returned["schema_version"],
        "forge.device-inventory-resource-convergence/v1"
    );
    assert_eq!(returned["converged"], true);
    assert_eq!(
        returned["inventory"]["devices"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        returned["resource_view"]["devices"][0]["device_id"],
        "device-a"
    );
    assert_eq!(returned["authority"]["dispatch_performed"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn converged_inventory_resource_read_rejects_counter_drift() {
    let pair = inventory_resource_convergence();
    let mut resource = pair["resource_view"].clone();
    resource["devices"][0]["heartbeat_sequence"] = Value::from(5_u64);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: pair["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: resource,
        },
    ]);
    let error = client
        .read_converged_inventory_resource_view()
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API inventory/resource observations did not converge"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn converged_client_instance_read_binds_owner_and_instance_rows() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: resource_view(),
        },
    ]);
    let returned = client.read_converged_client_instance_views().await.unwrap();
    assert_eq!(returned, session_resource_convergence());
    assert_eq!(
        returned["schema_version"],
        "forge.client-instance-session-resource-convergence/v1"
    );
    assert_eq!(returned["converged"], true);
    assert_eq!(
        returned["session_view"]["instances"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        returned["resource_view"]["devices"][0]["device_id"],
        "device-a"
    );
    assert_eq!(returned["authority"]["prompt_write_authorized"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn converged_client_instance_read_rejects_instance_row_drift() {
    let mut resource = resource_view();
    resource["instances"][0]["status"] = Value::String("offline".into());
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: resource,
        },
    ]);
    let error = client
        .read_converged_client_instance_views()
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API client-instance session/resource observations did not converge"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn converged_client_instance_read_rejects_owner_drift() {
    let mut resource = resource_view();
    resource["owner_declaration"]["subject"] = Value::String("another-user".into());
    for device in resource["devices"].as_array_mut().unwrap() {
        device["owner"]["subject"] = Value::String("another-user".into());
    }
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/session-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: session_view(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: resource,
        },
    ]);
    let error = client
        .read_converged_client_instance_views()
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API client-instance session/resource observations did not converge"
    );
    server.join().unwrap();
}
