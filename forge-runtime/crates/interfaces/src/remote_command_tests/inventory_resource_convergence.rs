use super::*;

fn inventory_resource_convergence() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("device inventory/resource convergence fixture")
}

#[tokio::test]
async fn converged_inventory_resource_read_matches_the_canonical_fixture() {
    let fixture = inventory_resource_convergence();
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: fixture["inventory"].clone(),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/client-instances/resource-view ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: fixture["resource_view"].clone(),
        },
    ]);

    let returned = client
        .read_converged_inventory_resource_view()
        .await
        .expect("canonical inventory/resource convergence");
    assert_eq!(returned, fixture);
    assert_eq!(returned["converged"], true);
    assert_eq!(returned["read_only"], true);
    assert_eq!(returned["authority"]["lease_issued"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn converged_inventory_resource_read_rejects_canonical_counter_drift() {
    let fixture = inventory_resource_convergence();
    let mut resource = fixture["resource_view"].clone();
    resource["devices"][0]["heartbeat_sequence"] = Value::from(5_u64);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: fixture["inventory"].clone(),
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
        .expect_err("counter drift must fail closed");
    assert_eq!(
        error.to_string(),
        "Forge API inventory/resource observations did not converge"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn converged_inventory_resource_read_rejects_observation_time_drift() {
    let fixture = inventory_resource_convergence();
    let mut resource = fixture["resource_view"].clone();
    resource["devices"][0]["observed_at_ms"] = Value::from(150001_u64);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: fixture["inventory"].clone(),
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
        .expect_err("observation time drift must fail closed");
    assert_eq!(
        error.to_string(),
        "Forge API inventory/resource observations did not converge"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn converged_inventory_resource_read_rejects_owner_drift() {
    let fixture = inventory_resource_convergence();
    let mut resource = fixture["resource_view"].clone();
    resource["owner_declaration"]["subject"] = Value::from("foreign-user");
    resource["devices"][0]["owner"]["subject"] = Value::from("foreign-user");
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: fixture["inventory"].clone(),
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
        .expect_err("owner drift must fail closed");
    assert_eq!(
        error.to_string(),
        "Forge API inventory/resource owner or declaration drifted"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn converged_inventory_resource_read_rejects_shared_resource_drift() {
    let fixture = inventory_resource_convergence();
    let mut resource = fixture["resource_view"].clone();
    resource["devices"][0]["available_memory_bytes"] = Value::from(8192_u64);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/devices/observations/v2 ",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: fixture["inventory"].clone(),
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
        .expect_err("resource drift must fail closed");
    assert_eq!(
        error.to_string(),
        "Forge API inventory/resource observations did not converge"
    );
    server.join().unwrap();
}
