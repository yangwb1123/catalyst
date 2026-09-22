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

#[tokio::test]
async fn create_conversation_sends_bearer_and_idempotency_contract() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        required_headers: &["idempotency-key: create-key"],
        body_fields: json!({"scope": {"kind": "global"}, "title": "Shared"}),
        response_status: "201 Created",
        response: json!({"id": "c-1"}),
    }]);
    assert_eq!(
        client
            .create_conversation("Shared", &RemoteConversationScope::Global, "create-key")
            .await
            .unwrap()["id"],
        "c-1"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn session_observation_preview_posts_the_bound_request_once_and_validates_the_envelope() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-device-observation-v1.json");
    let response: Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    let candidates = response["inventory"]["devices"].clone();
    let devices = candidates
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["device"].clone())
        .collect::<Vec<_>>();
    let request = json!({
        "owner": response["owner"].clone(),
        "conversation_id": response["conversation_id"].clone(),
        "run_id": response["run_id"].clone(),
        "placement": {
            "schema_version": "forge.device-placement-dry-run/v1",
            "evaluated_at_ms": response["evaluated_at_ms"].clone(),
            "owner": response["owner"].clone(),
            "max_snapshot_age_ms": 60000,
            "requirements": {
                "os": "linux",
                "architecture": "amd64",
                "min_cpu_cores": 4,
                "min_memory_bytes": 8192,
                "min_storage_bytes": 4096,
                "runtime": "oci",
                "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
                "data_residency_zones": ["us-west"],
                "minimum_trust_zone": "standard",
                "sandbox_floor": "container",
                "concurrency_slots": 1
            },
            "devices": devices,
        },
        "candidates": candidates,
    });
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/device-observation/preview ",
        required_headers: &[],
        body_fields: json!({
            "conversation_id": "conversation-001",
            "run_id": "run-001",
            "owner": response["owner"].clone(),
        }),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_session_device_observation("conversation-001", "run-001", &request)
        .await
        .unwrap();
    super::super::session_observation::validate_response(&returned, &request).unwrap();
    server.join().unwrap();

    let mut forged = returned;
    forged["authority"]["execution_authorized"] = Value::Bool(true);
    assert!(super::super::session_observation::validate_response(&forged, &request).is_err());
}

#[test]
fn remote_session_observation_requires_complete_nested_shape_before_request() {
    let request = json!({
        "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "conversation_id": "conversation-001",
        "run_id": "run-001",
        "placement": {
            "schema_version": "forge.device-placement-dry-run/v1",
            "evaluated_at_ms": 1800000000000_i64,
            "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
            "max_snapshot_age_ms": 60000,
            "requirements": {
                "os": "linux",
                "architecture": "amd64",
                "min_cpu_cores": 4,
                "min_memory_bytes": 8192,
                "min_storage_bytes": 4096,
                "runtime": "oci",
                "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
                "data_residency_zones": ["us-west"],
                "minimum_trust_zone": "standard",
                "sandbox_floor": "container",
                "concurrency_slots": 1
            },
            "devices": []
        },
        "candidates": []
    });
    super::super::session_observation::validate_request(&request).unwrap();

    let mut missing_gpu = request.clone();
    missing_gpu["placement"]["requirements"]
        .as_object_mut()
        .unwrap()
        .remove("gpu");
    assert!(super::super::session_observation::validate_request(&missing_gpu).is_err());

    let mut unknown_requirement = request;
    unknown_requirement["placement"]["requirements"]["unexpected"] = json!(true);
    assert!(super::super::session_observation::validate_request(&unknown_requirement).is_err());
}

#[tokio::test]
async fn get_conversation_detail_uses_exact_owner_scoped_read_path() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/c-1 ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: session_entry("c-1", &json!({"kind": "global"}), 10),
    }]);
    let detail = client.get_conversation("c-1").await.unwrap();
    assert_eq!(detail.conversation["id"], "c-1");
    assert_eq!(detail.aggregate_version, 1);
    server.join().unwrap();
}

#[tokio::test]
async fn create_project_conversation_sends_only_scope_metadata() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations ",
        required_headers: &["idempotency-key: project-key"],
        body_fields: json!({
            "scope": {"kind": "project", "id": "prj_1"},
            "title": "Project work"
        }),
        response_status: "201 Created",
        response: json!({"id": "c-1", "scope": {"kind": "project", "id": "prj_1"}}),
    }]);
    assert_eq!(
        client
            .create_conversation(
                "Project work",
                &RemoteConversationScope::Project("prj_1".into()),
                "project-key",
            )
            .await
            .unwrap()["scope"]["id"],
        "prj_1"
    );
    server.join().unwrap();
}
#[tokio::test]
async fn session_list_scope_filter_keeps_server_cursor_and_filters_exact_scope() {
    let conversations = (0..128)
        .map(|index| {
            let id = format!("c-{index:03}");
            let scope = if index == 0 {
                json!({"kind": "project", "id": "prj_1"})
            } else {
                json!({"kind": "global"})
            };
            session_entry(&id, &scope, index)
        })
        .collect::<Vec<_>>();
    let (client, server) = spawn_mock_server(vec![session_page_request(
        "GET /api/v1/conversations?limit=128 ",
        &json!(conversations),
        Some("c-127"),
        true,
    )]);
    let page = client
        .list_conversations_json(
            None,
            Some(&RemoteConversationScope::Project("prj_1".into())),
            false,
        )
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(page["conversations"][0]["conversation"]["id"], "c-000");
    assert_eq!(page["next_after_id"], "c-127");
    assert_eq!(page["has_more"], true);
    server.join().unwrap();
}

#[tokio::test]
async fn session_list_all_scans_later_pages_before_applying_scope_filter() {
    let first_page = (0..128)
        .map(|index| session_entry(&format!("c-{index:03}"), &json!({"kind": "global"}), index))
        .collect::<Vec<_>>();
    let second_page = vec![session_entry(
        "c-128",
        &json!({"kind": "project", "id": "prj_1"}),
        128,
    )];
    let (client, server) = spawn_mock_server(vec![
        session_page_request(
            "GET /api/v1/conversations?limit=128 ",
            &json!(first_page),
            Some("c-127"),
            true,
        ),
        session_page_request(
            "GET /api/v1/conversations?limit=128&after_id=c-127 ",
            &json!(second_page),
            None,
            false,
        ),
    ]);

    let page = client
        .list_conversations_json(
            None,
            Some(&RemoteConversationScope::Project("prj_1".into())),
            true,
        )
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(page["conversations"][0]["conversation"]["id"], "c-128");
    assert_eq!(page["next_after_id"], Value::Null);
    assert_eq!(page["has_more"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn session_list_instance_projection_filters_locally_without_query_authority() {
    let conversations = vec![
        session_entry("c-001", &json!({"kind": "global"}), 1),
        session_entry("c-002", &json!({"kind": "global"}), 2),
    ];
    let (client, server) = spawn_mock_server(vec![session_page_request(
        "GET /api/v1/conversations?limit=128 ",
        &json!(conversations),
        None,
        false,
    )]);
    let instance_scope = crate::client_instance_session_scope::ClientInstanceSessionScope {
        instance_id: "client-web-001".into(),
        client_kind: "web".into(),
        session_ids: ["c-002".to_owned()].into_iter().collect(),
    };
    let page = client
        .list_conversations_json_with_instance(None, None, Some(&instance_scope), false)
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 1);
    assert_eq!(page["conversations"][0]["conversation"]["id"], "c-002");
    server.join().unwrap();
}

#[tokio::test]
async fn session_list_all_stops_at_the_page_bound_and_returns_a_continuation() {
    let (client, server) = spawn_mock_server(full_conversation_pages(64));
    let page = client
        .list_conversations_json(None, None, true)
        .await
        .unwrap();
    assert_eq!(page["conversations"].as_array().unwrap().len(), 64 * 128);
    assert_eq!(page["next_after_id"], "c-08191");
    assert_eq!(page["has_more"], true);
    server.join().unwrap();
}
#[tokio::test]
async fn list_prompts_sends_bearer_and_reads_conversation_page() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/c-1/prompts?",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
    }]);
    assert_eq!(
        client.list_prompts("c-1", None).await.unwrap()["conversation_id"],
        "c-1"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn transient_read_response_is_retried_with_the_same_authenticated_request() {
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/c-1/prompts?",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "429 Too Many Requests",
            response: json!({"code": "temporarily_unavailable"}),
        },
        ExpectedRequest {
            request_prefix: "GET /api/v1/conversations/c-1/prompts?",
            required_headers: &[],
            body_fields: Value::Null,
            response_status: "200 OK",
            response: json!({"conversation_id": "c-1", "prompts": [], "has_more": false}),
        },
    ]);
    assert_eq!(
        client.list_prompts("c-1", None).await.unwrap()["conversation_id"],
        "c-1"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn older_prompt_page_uses_both_cursor_fields_and_stays_before_cursor() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/c-1/prompts?limit=128&before_created_at_ms=200&before_prompt_id=p-new ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: json!({
            "conversation_id": "c-1",
            "prompts": [{"id": "p-old", "conversation_id": "c-1", "role": "user",
                "content": "older prompt", "created_at_ms": 100}],
            "has_more": false
        }),
    }]);
    let page = client
        .list_prompts(
            "c-1",
            Some(&crate::args::PromptPageCursor {
                created_at_ms: 200,
                prompt_id: "p-new".into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(page["prompts"][0]["id"], "p-old");
    server.join().unwrap();
}
#[tokio::test]
async fn append_prompt_sends_bearer_cas_and_idempotency_contract() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "run this prompt", "expected_version": 7}),
        response_status: "200 OK",
        response: json!({"aggregate_version": 8, "replayed": false}),
    }]);
    assert_eq!(
        client
            .append_prompt("c-1", 7, "run this prompt", "prompt-key")
            .await
            .unwrap()["aggregate_version"],
        8
    );
    server.join().unwrap();
}

#[tokio::test]
async fn append_prompt_rejects_aggregate_version_above_json_safe_integer() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "run this prompt", "expected_version": 7}),
        response_status: "201 Created",
        response: json!({"aggregate_version": 9_007_199_254_740_992_u64, "replayed": false}),
    }]);
    let error = client
        .append_prompt("c-1", 7, "run this prompt", "prompt-key")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Prompt receipt"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn append_prompt_rejects_a_non_sequential_aggregate_version() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-key"],
        body_fields: json!({"content": "run this prompt", "expected_version": 7}),
        response_status: "200 OK",
        response: json!({"aggregate_version": 9, "replayed": false}),
    }]);
    let error = client
        .append_prompt("c-1", 7, "run this prompt", "prompt-key")
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Forge API returned an invalid Prompt receipt"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn append_prompt_rejects_a_missing_or_non_boolean_replay_marker() {
    for response in [
        json!({"aggregate_version": 8}),
        json!({"aggregate_version": 8, "replayed": "false"}),
    ] {
        let (client, server) = spawn_mock_server(vec![ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/c-1/prompts ",
            required_headers: &["idempotency-key: prompt-key"],
            body_fields: json!({"content": "run this prompt", "expected_version": 7}),
            response_status: "200 OK",
            response,
        }]);
        let error = client
            .append_prompt("c-1", 7, "run this prompt", "prompt-key")
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "Forge API returned an invalid Prompt receipt"
        );
        server.join().unwrap();
    }
}

#[tokio::test]
async fn append_prompt_preserves_multiline_content() {
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/c-1/prompts ",
        required_headers: &["idempotency-key: prompt-multiline"],
        body_fields: json!({
            "content": "first line\nsecond line\n",
            "expected_version": 7
        }),
        response_status: "201 Created",
        response: json!({"aggregate_version": 8, "replayed": false}),
    }]);
    client
        .append_prompt("c-1", 7, "first line\nsecond line\n", "prompt-multiline")
        .await
        .unwrap();
    server.join().unwrap();
}
