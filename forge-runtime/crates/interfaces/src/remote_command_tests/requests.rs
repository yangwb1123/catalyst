use super::*;

fn session_entry(id: &str, scope: &Value, created_at_ms: u64) -> Value {
    json!({
        "conversation": {
            "id": id,
            "scope": scope,
            "title": "Shared",
            "created_at_ms": created_at_ms,
            "updated_at_ms": created_at_ms,
        },
        "aggregate_version": 1
    })
}

fn inventory_observation() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json"
    ))
    .expect("device inventory observation fixture")
}

fn inventory_observation_v2() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
    ))
    .expect("device inventory observation v2 fixture")
}

fn prompt_append_response(
    conversation_id: &str,
    content: &str,
    aggregate_version: u64,
    replayed: bool,
) -> Value {
    json!({
        "prompt": {
            "id": "prompt-1",
            "conversation_id": conversation_id,
            "role": "user",
            "content": content,
            "created_at_ms": 300,
        },
        "aggregate_version": aggregate_version,
        "replayed": replayed,
    })
}

#[tokio::test]
async fn device_inventory_read_sends_one_authenticated_get_without_a_body() {
    let response = inventory_observation();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/devices ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client.read_device_inventory().await.unwrap();
    assert_eq!(
        returned["schema_version"],
        "forge.device-inventory-observation/v1"
    );
    assert_eq!(returned["owner_declaration"]["subject"], "user-1");
    assert_eq!(returned["devices"][0]["device"]["device_id"], "device-a");
    assert_eq!(returned["execution_authorized"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn device_inventory_read_rejects_authority_mutation_after_authenticated_get() {
    let mut response = inventory_observation();
    response["execution_authorized"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/devices ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response,
    }]);
    let error = client.read_device_inventory().await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid device inventory"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn device_inventory_v2_read_sends_one_authenticated_get_without_a_body() {
    let response = inventory_observation_v2();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/devices/observations/v2 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client.read_device_inventory_v2().await.unwrap();
    assert_eq!(
        returned["schema_version"],
        "forge.device-inventory-observation/v2"
    );
    assert_eq!(returned["devices"][0]["revision"], 1);
    assert_eq!(
        returned["devices"][0]["device"]["reservation_state"],
        "reserved"
    );
    assert_eq!(
        returned["devices"][0]["device"]["gpus"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(returned["execution_authorized"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn device_inventory_v2_read_rejects_authority_mutation_after_authenticated_get() {
    let mut response = inventory_observation_v2();
    response["dispatch_performed"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/devices/observations/v2 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response,
    }]);
    let error = client.read_device_inventory_v2().await.unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid v2 device inventory"
    );
    server.join().unwrap();
}

fn session_page_request(
    request_prefix: &'static str,
    conversations: &Value,
    next_after_id: Option<&str>,
    has_more: bool,
) -> ExpectedRequest {
    ExpectedRequest {
        request_prefix,
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversations": conversations,
            "next_after_id": next_after_id,
            "has_more": has_more
        }),
    }
}

fn full_conversation_pages(page_count: usize) -> Vec<ExpectedRequest> {
    (0..page_count)
        .map(|page_index| {
            let first_row = page_index * 128;
            let conversations = (0..128)
                .map(|offset| {
                    let row = first_row + offset;
                    session_entry(
                        &format!("c-{row:05}"),
                        &json!({"kind": "global"}),
                        u64::try_from(row).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            let cursor = format!("c-{:05}", first_row + 127);
            session_page_request(
                "GET /api/v1/conversations",
                &json!(conversations),
                Some(&cursor),
                true,
            )
        })
        .collect()
}

#[path = "requests/conversations_tests.rs"]
mod conversations_tests;
#[path = "requests/prompts_tests.rs"]
mod prompts_tests;
#[path = "requests/session_observation_tests.rs"]
mod session_observation_tests;
