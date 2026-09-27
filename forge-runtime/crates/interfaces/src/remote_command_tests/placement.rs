use super::*;

fn placement_request() -> Value {
    json!({
        "schema_version": "forge.device-placement-dry-run/v1",
        "evaluated_at_ms": 1_800_000_000_000_i64,
        "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "max_snapshot_age_ms": 60_000,
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        },
        "devices": [{
            "device_id": "device-1",
            "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
            "approval_state": "approved", "cordon_state": "clear", "liveness": "online",
            "snapshot_observed_at_ms": 1799999999000_i64, "lease_expires_at_ms": 1800000060000_i64,
            "os": "linux", "architecture": "amd64", "available_cpu_cores": 2,
            "available_memory_bytes": 4096, "available_storage_bytes": 4096,
            "runtimes": ["oci"], "gpu": {"present": false, "memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "trust_zone": "standard",
            "sandbox_levels": ["container"], "concurrency_limit": 2, "active_concurrency": 0
        }]
    })
}

fn placement_response(matches: bool, reasons: Value) -> Value {
    json!({
        "schema_version": "forge.device-placement-dry-run-result/v1",
        "evaluation_mode": "offline_static_only",
        "evaluated_at_ms": 1_800_000_000_000_i64,
        "owner_declaration": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "owner_declaration_unverified": true,
        "device_attributes_unverified": true,
        "notice": "All owner, approval, liveness, resource, residency, trust, and sandbox attributes are unverified caller declarations. This offline comparison selects no target and grants no execution authority.",
        "device_results": [{
            "device_id": "device-1",
            "attributes_unverified": true,
            "matches_requirements": matches,
            "exclusion_reasons": reasons
        }],
        "execution_authorized": false,
        "reservation_created": false,
        "dispatch_performed": false
    })
}

fn registry_placement_request() -> Value {
    json!({
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        }
    })
}

fn registry_placement_response() -> Value {
    json!({
        "schema_version": "forge.device-inventory-placement-evaluation/v2",
        "evaluation_mode": "offline_static_only",
        "source_schema_version": "forge.device-inventory-observation/v2",
        "evaluation_owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "evaluated_at_ms": 1800000000000_i64,
        "notice": "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority.",
        "decisions": [{
            "revision": 1, "generation": 1, "heartbeat_sequence": 1,
            "device_id": "device-a", "instance_id": "runner-a", "reservation_state": "none",
            "gpu_count": 0, "available_gpu_memory_bytes": 0,
            "matches_requirements": true, "exclusion_reasons": [],
            "owner_declaration_unverified": true, "device_attributes_unverified": true
        }],
        "eligible_candidate_count": 1,
        "selected_device_id": null,
        "selected_instance_id": null,
        "authority": {
            "identity_verified": false, "heartbeat_persisted": false,
            "inventory_authoritative": false, "placement_selected": false,
            "reservation_created": false, "execution_authorized": false,
            "dispatch_performed": false
        }
    })
}

fn scheduler_selection_lease_request() -> Value {
    json!({
        "conversation_id": "conversation-1",
        "run_id": "run-1",
        "attempt_id": "attempt-1",
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        },
        "ttl_ms": 30000
    })
}

fn scheduler_selection_lease_response() -> Value {
    json!({
        "schema_version": "forge.execution-lease-registry/v1",
        "evaluation_mode": "durable_scheduler_lease_claim",
        "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
        "device_id": "device-a", "instance_id": "runner-a",
        "inventory_revision": 7, "generation": 3, "heartbeat_sequence": 12,
        "grant": {
            "v": 1, "attempt_id": "attempt-1", "target_id": "runner-a", "epoch": 1,
            "fencing_token": "fence-token-a", "issued_at_ms": 1800000000000_i64,
            "expires_at_ms": 1800000030000_i64
        },
        "replayed": false,
        "authority": {
            "placement_selected": true, "reservation_created": true, "lease_issued": true,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    })
}

fn scheduler_selection_lease_renewal_request() -> Value {
    json!({
        "conversation_id": "conversation-1",
        "run_id": "run-1",
        "attempt_id": "attempt-1",
        "target_id": "runner-a",
        "epoch": 1,
        "fencing_token": "fence-token-a",
        "ttl_ms": 30000
    })
}

fn scheduler_selection_lease_release_request() -> Value {
    json!({
        "conversation_id": "conversation-1",
        "run_id": "run-1",
        "attempt_id": "attempt-1",
        "target_id": "runner-a",
        "epoch": 2,
        "fencing_token": "fence-token-b",
    })
}

fn scheduler_selection_lease_release_response() -> Value {
    json!({
        "schema_version": "forge.execution-lease-registry/v1",
        "evaluation_mode": "durable_scheduler_lease_release",
        "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
        "device_id": "device-a", "instance_id": "runner-a", "epoch": 2,
        "released_at_ms": 1800000000100_i64, "replayed": false,
        "authority": {
            "placement_selected": false, "reservation_created": false, "lease_issued": false,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    })
}

#[tokio::test]
async fn scheduler_selection_lease_posts_once_with_explicit_idempotency() {
    let request = scheduler_selection_lease_request();
    let response = scheduler_selection_lease_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
        required_headers: &["idempotency-key: lease-key-00000001"],
        body_fields: json!({
            "conversation_id": "conversation-1",
            "run_id": "run-1",
            "attempt_id": "attempt-1",
            "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .claim_scheduler_selection_lease(&request, "lease-key-00000001")
        .await
        .unwrap();
    super::super::scheduler_lease::validate_response(&returned).unwrap();
    assert_eq!(returned["grant"]["epoch"], 1);
    server.join().unwrap();

    let mut enabled = response;
    enabled["authority"]["dispatch_performed"] = Value::Bool(true);
    assert!(super::super::scheduler_lease::validate_response(&enabled).is_err());
}

#[tokio::test]
async fn scheduler_selection_lease_renewal_posts_once_with_explicit_idempotency() {
    let request = scheduler_selection_lease_renewal_request();
    let mut response = scheduler_selection_lease_response();
    response["grant"]["epoch"] = json!(2);
    response["grant"]["fencing_token"] = json!("fence-token-b");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
        required_headers: &["idempotency-key: renew-key-00000001"],
        body_fields: json!({
            "conversation_id": "conversation-1",
            "target_id": "runner-a",
            "epoch": 1,
            "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .renew_scheduler_selection_lease(&request, "renew-key-00000001")
        .await
        .unwrap();
    super::super::scheduler_lease_renew::validate_response(&returned).unwrap();
    assert_eq!(returned["grant"]["epoch"], 2);
    server.join().unwrap();
}

#[tokio::test]
async fn scheduler_selection_lease_release_posts_once_with_explicit_idempotency() {
    let request = scheduler_selection_lease_release_request();
    let response = scheduler_selection_lease_release_response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/release ",
        required_headers: &["idempotency-key: release-key-00000001"],
        body_fields: json!({
            "conversation_id": "conversation-1",
            "target_id": "runner-a",
            "epoch": 2
        }),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .release_scheduler_selection_lease(&request, "release-key-00000001")
        .await
        .unwrap();
    super::super::scheduler_lease_release::validate_response(&returned).unwrap();
    assert_eq!(returned["released_at_ms"], 1800000000100_i64);
    assert!(!returned.to_string().contains("fence-token-b"));
    server.join().unwrap();

    let mut authority = response;
    authority["authority"]["lease_issued"] = Value::Bool(true);
    assert!(super::super::scheduler_lease_release::validate_response(&authority).is_err());
}

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
    super::super::placement_registry::validate_response(&returned).unwrap();
    server.join().unwrap();

    let mut selected = response;
    selected["selected_device_id"] = json!("device-a");
    assert!(super::super::placement_registry::validate_response(&selected).is_err());
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

#[tokio::test]
async fn placement_preview_posts_the_caller_declaration_once_and_rejects_authority_claims() {
    let request = json!({
        "schema_version": "forge.device-placement-dry-run/v1",
        "evaluated_at_ms": 1800000000000_i64,
        "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "max_snapshot_age_ms": 60000,
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        },
        "devices": []
    });
    let response = json!({
        "schema_version": "forge.device-placement-dry-run-result/v1",
        "evaluation_mode": "offline_static_only",
        "evaluated_at_ms": 1800000000000_i64,
        "owner_declaration": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "owner_declaration_unverified": true,
        "device_attributes_unverified": true,
        "notice": "All owner, approval, liveness, resource, residency, trust, and sandbox attributes are unverified caller declarations. This offline comparison selects no target and grants no execution authority.",
        "device_results": [],
        "execution_authorized": false,
        "reservation_created": false,
        "dispatch_performed": false
    });
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/preview ",
        required_headers: &[],
        body_fields: json!({"schema_version": "forge.device-placement-dry-run/v1"}),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client.preview_device_placement(&request).await.unwrap();
    super::super::placement::validate_response(&returned, &request).unwrap();
    server.join().unwrap();

    let request_with_device = json!({
        "schema_version": "forge.device-placement-dry-run/v1",
        "evaluated_at_ms": 1800000000000_i64,
        "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
        "max_snapshot_age_ms": 60000,
        "requirements": {
            "os": "linux", "architecture": "amd64", "min_cpu_cores": 1,
            "min_memory_bytes": 1, "min_storage_bytes": 1, "runtime": "oci",
            "gpu": {"required": false, "min_memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "minimum_trust_zone": "standard",
            "sandbox_floor": "container", "concurrency_slots": 1
        },
        "devices": [{
            "device_id": "device-1",
            "owner": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
            "approval_state": "approved", "cordon_state": "clear", "liveness": "online",
            "snapshot_observed_at_ms": 1799999999000_i64, "lease_expires_at_ms": 1800000060000_i64,
            "os": "linux", "architecture": "amd64", "available_cpu_cores": 2,
            "available_memory_bytes": 4096, "available_storage_bytes": 4096,
            "runtimes": ["oci"], "gpu": {"present": false, "memory_bytes": 0, "runtime": ""},
            "data_residency_zones": ["us-west"], "trust_zone": "standard",
            "sandbox_levels": ["container"], "concurrency_limit": 2, "active_concurrency": 0
        }]
    });
    assert!(super::super::placement::validate_response(&returned, &request_with_device).is_err());

    let mut forged = response;
    forged["execution_authorized"] = Value::Bool(true);
    assert!(super::super::placement::validate_response(&forged, &request).is_err());
}

#[tokio::test]
async fn placement_preview_rejects_response_binding_drift_at_client_boundary() {
    let request = placement_request();
    let mut response = placement_response(true, json!([]));
    response["owner_declaration"]["subject"] = json!("user-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/preview ",
        required_headers: &[],
        body_fields: json!({
            "schema_version": "forge.device-placement-dry-run/v1",
            "evaluated_at_ms": 1800000000000_i64,
        }),
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .preview_device_placement(&request)
        .await
        .expect_err("a foreign placement response must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned a placement preview with invalid authority or binding"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn placement_preview_rejects_authority_drift_at_client_boundary() {
    let request = placement_request();
    let mut response = placement_response(true, json!([]));
    response["execution_authorized"] = Value::Bool(true);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/preview ",
        required_headers: &[],
        body_fields: json!({
            "schema_version": "forge.device-placement-dry-run/v1",
            "evaluated_at_ms": 1800000000000_i64,
        }),
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .preview_device_placement(&request)
        .await
        .expect_err("an authority-bearing placement response must not escape the HTTP client");
    assert_eq!(
        error.to_string(),
        "Forge API returned a placement preview with invalid authority or binding"
    );
    server.join().unwrap();
}

#[test]
fn remote_placement_request_rejects_unsafe_numbers_and_owner_controls() {
    let mut unsafe_timestamp = placement_request();
    unsafe_timestamp["evaluated_at_ms"] = json!(9_007_199_254_740_992_i64);
    assert!(super::super::placement::validate_request(&unsafe_timestamp).is_err());

    let mut unsafe_age = placement_request();
    unsafe_age["max_snapshot_age_ms"] = json!(86_400_000_i64 + 1);
    assert!(super::super::placement::validate_request(&unsafe_age).is_err());

    let mut control_owner = placement_request();
    control_owner["owner"]["subject"] = json!("user-\n-a");
    assert!(super::super::placement::validate_request(&control_owner).is_err());

    let mut requirement_memory = placement_request();
    requirement_memory["requirements"]["min_memory_bytes"] = json!(9_007_199_254_740_992_u64);
    assert!(super::super::placement::validate_request(&requirement_memory).is_err());

    let mut requirement_storage = placement_request();
    requirement_storage["requirements"]["min_storage_bytes"] = json!(9_007_199_254_740_992_u64);
    assert!(super::super::placement::validate_request(&requirement_storage).is_err());

    let mut requirement_gpu = placement_request();
    requirement_gpu["requirements"]["gpu"] = json!({
        "required": true,
        "min_memory_bytes": 9_007_199_254_740_992_u64,
        "runtime": "cuda"
    });
    assert!(super::super::placement::validate_request(&requirement_gpu).is_err());

    let mut device_snapshot = placement_request();
    device_snapshot["devices"][0]["snapshot_observed_at_ms"] = json!(9_007_199_254_740_992_u64);
    assert!(super::super::placement::validate_request(&device_snapshot).is_err());

    let mut device_lease = placement_request();
    device_lease["devices"][0]["lease_expires_at_ms"] = json!(9_007_199_254_740_992_u64);
    assert!(super::super::placement::validate_request(&device_lease).is_err());

    let mut device_memory = placement_request();
    device_memory["devices"][0]["available_memory_bytes"] = json!(9_007_199_254_740_992_u64);
    assert!(super::super::placement::validate_request(&device_memory).is_err());

    let mut device_storage = placement_request();
    device_storage["devices"][0]["available_storage_bytes"] = json!(9_007_199_254_740_992_u64);
    assert!(super::super::placement::validate_request(&device_storage).is_err());

    let mut device_gpu = placement_request();
    device_gpu["devices"][0]["gpu"] = json!({
        "present": true,
        "memory_bytes": 9_007_199_254_740_992_u64,
        "runtime": "cuda"
    });
    assert!(super::super::placement::validate_request(&device_gpu).is_err());
}

#[test]
fn remote_placement_request_requires_complete_nested_shape_and_unique_keys() {
    let mut missing_requirement = placement_request();
    missing_requirement["requirements"]
        .as_object_mut()
        .unwrap()
        .remove("runtime");
    assert!(super::super::placement::validate_request(&missing_requirement).is_err());

    let mut unknown_field = placement_request();
    unknown_field["devices"][0]["unexpected"] = json!(true);
    assert!(super::super::placement::validate_request(&unknown_field).is_err());

    let duplicate = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        duplicate.path(),
        br#"{"schema_version":"forge.device-placement-dry-run/v1","schema_version":"forge.device-placement-dry-run/v1"}"#,
    )
    .unwrap();
    assert!(super::super::placement::read_request(duplicate.path().to_str().unwrap()).is_err());
}

#[test]
fn remote_placement_response_rejects_inconsistent_exclusion_declarations() {
    let request = placement_request();
    let response = placement_response(true, json!(["approval_pending"]));
    assert!(super::super::placement::validate_response(&response, &request).is_err());

    let response = placement_response(false, json!(["approval_pending", "approval_pending"]));
    assert!(super::super::placement::validate_response(&response, &request).is_err());

    let response = placement_response(false, json!(["approval_pending", "bad\nreason"]));
    assert!(super::super::placement::validate_response(&response, &request).is_err());

    let response = placement_response(false, json!(["approval_pending"]));
    assert!(super::super::placement::validate_response(&response, &request).is_err());
}

#[tokio::test]
async fn remote_placement_response_rejects_duplicate_json_keys_before_decode() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let _ = capture_request(&mut stream);
        let body = br#"{"schema_version":"forge.device-placement-dry-run-result/v1","schema_version":"forge.device-placement-dry-run-result/v1"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    });
    let error = test_remote_client(address)
        .preview_device_placement(&placement_request())
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), "Forge API returned duplicate JSON keys");
    server.join().unwrap();
}

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
    super::super::scheduler_selection::validate_response(&returned).unwrap();
    server.join().unwrap();

    let mut authority = response;
    authority["authority"]["dispatch_performed"] = json!(true);
    assert!(super::super::scheduler_selection::validate_response(&authority).is_err());
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
    super::super::scheduler_selection::validate_response_for_request(&response, &request).unwrap();
}

#[tokio::test]
async fn scheduler_lease_claim_client_rejects_a_response_for_another_attempt() {
    let request = scheduler_selection_lease_request();
    let mut response = scheduler_selection_lease_response();
    response["attempt_id"] = json!("attempt-foreign");
    response["grant"]["attempt_id"] = json!("attempt-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease ",
        required_headers: &["idempotency-key: lease-binding-key-0001"],
        body_fields: json!({
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "requirements": request["requirements"].clone(), "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response,
    }]);
    assert!(
        client
            .claim_scheduler_selection_lease(&request, "lease-binding-key-0001")
            .await
            .is_err()
    );
    server.join().unwrap();
}

#[tokio::test]
async fn scheduler_lease_renewal_client_rejects_a_response_for_another_epoch() {
    let request = scheduler_selection_lease_renewal_request();
    let mut response = scheduler_selection_lease_response();
    response["grant"]["epoch"] = json!(3);
    response["grant"]["fencing_token"] = json!("fence-token-c");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/renew ",
        required_headers: &["idempotency-key: renewal-binding-key-0001"],
        body_fields: json!({
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "target_id": "runner-a", "epoch": 1, "fencing_token": "fence-token-a", "ttl_ms": 30000
        }),
        response_status: "200 OK",
        response,
    }]);
    assert!(
        client
            .renew_scheduler_selection_lease(&request, "renewal-binding-key-0001")
            .await
            .is_err()
    );
    server.join().unwrap();
}

#[tokio::test]
async fn scheduler_lease_release_client_rejects_a_response_for_another_target() {
    let request = scheduler_selection_lease_release_request();
    let mut response = scheduler_selection_lease_release_response();
    response["instance_id"] = json!("runner-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/device-placement/scheduler-lease/release ",
        required_headers: &["idempotency-key: release-binding-key-0001"],
        body_fields: json!({
            "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
            "target_id": "runner-a", "epoch": 2, "fencing_token": "fence-token-b"
        }),
        response_status: "200 OK",
        response,
    }]);
    assert!(
        client
            .release_scheduler_selection_lease(&request, "release-binding-key-0001")
            .await
            .is_err()
    );
    server.join().unwrap();
}
