use super::*;
use crate::remote_command::placement_registry as placement_registry_contract;

#[tokio::test]
async fn registry_placement_preview_posts_requirements_once_and_validates_v2_response() {
    let request = registry_placement_request();
    let response = registry_placement_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/registry-preview ",
        required_headers: &[],
        body_fields: json!({"requirements": request["requirements"].clone()}),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_device_placement_registry(&request)
        .await
        .unwrap();
    placement_registry_contract::validate_response(&returned).unwrap();
    server.join().unwrap();

    let mut selected = response;
    selected["selected_device_id"] = json!("device-a");
    assert!(placement_registry_contract::validate_response(&selected).is_err());
}

#[tokio::test]
async fn registry_placement_preview_rejects_response_drift_at_client_boundary() {
    let request = registry_placement_request();
    let mut response = registry_placement_response();
    response["selected_device_id"] = json!("device-a");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/registry-preview ",
        required_headers: &[],
        body_fields: json!({"requirements": request["requirements"].clone()}),
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .preview_device_placement_registry(&request)
        .await
        .expect_err("a selected target must not escape the registry preview client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid registry placement preview"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn registry_placement_preview_rejects_authority_drift_at_client_boundary() {
    let request = registry_placement_request();
    let mut response = registry_placement_response();
    response["authority"]["placement_selected"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/registry-preview ",
        required_headers: &[],
        body_fields: json!({"requirements": request["requirements"].clone()}),
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .preview_device_placement_registry(&request)
        .await
        .expect_err("authority-bearing registry preview must not escape the client");
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid registry placement preview"
    );
    server.join().unwrap();
}
