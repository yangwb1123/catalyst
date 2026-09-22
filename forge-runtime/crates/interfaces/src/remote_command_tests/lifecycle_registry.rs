use super::*;
use serde_json::json;

fn empty_registry() -> Value {
    json!({
        "schema_version": "forge.device-enrollment-heartbeat-lifecycle-file-set/v1",
        "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "states": []
    })
}

#[tokio::test]
async fn lifecycle_registry_show_sends_one_authenticated_get_without_body() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/device-enrollment-heartbeat/lifecycle-registry ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: empty_registry(),
    }]);
    let response = client.read_lifecycle_registry().await.unwrap();
    assert_eq!(
        response["schema_version"],
        "forge.device-enrollment-heartbeat-lifecycle-file-set/v1"
    );
    assert!(response["states"].as_array().unwrap().is_empty());
    server.join().unwrap();
}

#[tokio::test]
async fn lifecycle_registry_show_rejects_schema_owner_and_unknown_fields() {
    for name in ["schema", "unknown", "owner"] {
        let mut response = empty_registry();
        match name {
            "schema" => response["schema_version"] = json!("forge.other/v1"),
            "unknown" => response["extra"] = json!(true),
            "owner" => response["owner"]["subject"] = json!(" "),
            _ => unreachable!(),
        }
        let (client, server) = spawn_mock_server(vec![ExpectedRequest {
            request_prefix: "GET /api/v1/device-enrollment-heartbeat/lifecycle-registry ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response,
        }]);
        let error = client.read_lifecycle_registry().await.unwrap_err();
        assert_eq!(
            error.to_string(),
            "Forge API returned an invalid lifecycle registry candidate",
            "case {name}"
        );
        server.join().unwrap();
    }
}
