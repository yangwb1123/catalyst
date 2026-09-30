use super::*;
use crate::remote_command::session_observation as session_observation_contract;

#[tokio::test]
async fn session_observation_preview_posts_the_bound_request_once_and_validates_the_envelope() {
    let (response, request) = session_observation_fixture();
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
    session_observation_contract::validate_response(&returned, &request).unwrap();
    server.join().unwrap();

    let mut forged = returned;
    forged["authority"]["execution_authorized"] = Value::Bool(true);
    assert!(session_observation_contract::validate_response(&forged, &request).is_err());
}

#[tokio::test]
async fn session_observation_preview_rejects_response_drift_at_client_boundary() {
    let (response, request) = session_observation_fixture();
    let mut forged = response;
    forged["conversation_id"] = json!("conversation-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/device-observation/preview ",
        required_headers: &[],
        body_fields: json!({
            "conversation_id": "conversation-001",
            "run_id": "run-001",
            "owner": request["owner"].clone(),
        }),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_session_device_observation("conversation-001", "run-001", &request)
        .await
        .expect_err("foreign observation must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "session device observation binding mismatch"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn session_observation_preview_rejects_request_url_drift_before_post() {
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
        "conversation_id": "conversation-001",
        "run_id": "run-001",
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
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_device_observation("conversation-foreign", "run-001", &request)
        .await
        .expect_err("a request bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Session device observation request does not match its URL"
    );
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
    session_observation_contract::validate_request(&request).unwrap();

    let mut missing_gpu = request.clone();
    missing_gpu["placement"]["requirements"]
        .as_object_mut()
        .unwrap()
        .remove("gpu");
    assert!(session_observation_contract::validate_request(&missing_gpu).is_err());

    let mut unknown_requirement = request;
    unknown_requirement["placement"]["requirements"]["unexpected"] = json!(true);
    assert!(session_observation_contract::validate_request(&unknown_requirement).is_err());
}

fn session_observation_fixture() -> (Value, Value) {
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
    (response, request)
}
