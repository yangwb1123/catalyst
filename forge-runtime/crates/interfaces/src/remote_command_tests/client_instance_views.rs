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
