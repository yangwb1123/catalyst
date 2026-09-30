use super::*;
use crate::remote_command::scheduler_selection as scheduler_selection_contract;

#[tokio::test]
async fn scheduler_selection_preview_posts_bound_request_and_keeps_authority_closed() {
    let request = json!({
        "conversation_id": "conversation-1",
        "run_id": "run-1",
        "attempt_id": "attempt-1",
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        }
    });
    let response = json!({
        "schema_version": "forge.scheduler-selection-preview/v1",
        "evaluation_mode": "pure_scheduler_selection_preview",
        "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
        "evaluated_at_ms": 1_800_000_000_000_i64,
        "candidate_count": 1, "eligible_candidate_count": 0,
        "selection_available": false, "selection_reason": "no_eligible_candidate",
        "selected_device_id": null, "selected_instance_id": null, "preview_only": true,
        "authority": {
            "placement_selected": false, "reservation_created": false, "lease_issued": false,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    });
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client.preview_scheduler_selection(&request).await.unwrap();
    scheduler_selection_contract::validate_response(&returned).unwrap();
    server.join().unwrap();

    let mut authority = response;
    authority["authority"]["dispatch_performed"] = json!(true);
    assert!(scheduler_selection_contract::validate_response(&authority).is_err());
}

#[tokio::test]
async fn scheduler_selection_client_rejects_a_response_for_another_run() {
    let request = json!({
        "conversation_id": "conversation-1",
        "run_id": "run-1",
        "attempt_id": "attempt-1",
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        }
    });
    let mut response = json!({
        "schema_version": "forge.scheduler-selection-preview/v1",
        "evaluation_mode": "pure_scheduler_selection_preview",
        "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "conversation_id": "conversation-1", "run_id": "run-foreign", "attempt_id": "attempt-1",
        "evaluated_at_ms": 1_800_000_000_000_i64,
        "candidate_count": 0, "eligible_candidate_count": 0,
        "selection_available": false, "selection_reason": "no_eligible_candidate",
        "selected_device_id": null, "selected_instance_id": null, "preview_only": true,
        "authority": {
            "placement_selected": false, "reservation_created": false, "lease_issued": false,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    });
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    assert!(client.preview_scheduler_selection(&request).await.is_err());
    server.join().unwrap();

    response["run_id"] = json!("run-1");
    scheduler_selection_contract::validate_response_for_request(&response, &request).unwrap();
}
