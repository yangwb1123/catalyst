use super::*;

#[tokio::test]
async fn remote_tui_placement_preview_posts_file_and_renders_unverified_authority() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/preview "));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            posted["schema_version"],
            "forge.device-placement-dry-run/v1"
        );
        respond(
            &mut stream,
            "200 OK",
            &json!({
                "schema_version": "forge.device-placement-dry-run-result/v1",
                "evaluation_mode": "offline_static_only",
                "evaluated_at_ms": 1800000000000_i64,
                "owner_declaration": {"issuer": "https://id.example", "subject": "user-a", "tenant_id": "tenant-a"},
                "owner_declaration_unverified": true,
                "device_attributes_unverified": true,
                "notice": "All owner, approval, liveness, resource, residency, trust, and sandbox attributes are unverified caller declarations. This offline comparison selects no target and grants no execution authority.",
                "device_results": [{"device_id": "device-1", "attributes_unverified": true, "matches_requirements": true, "exclusion_reasons": []}],
                "execution_authorized": false,
                "reservation_created": false,
                "dispatch_performed": false
            }),
        );
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), PLACEMENT_INPUT).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "placement-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline placement preview"), "{output}");
    assert!(output.contains("device-1: matches"), "{output}");
    assert!(output.contains("execution_authorized=false"), "{output}");
}

#[tokio::test]
async fn remote_tui_registry_placement_preview_posts_requirements_and_renders_no_selection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_registry_preview(listener));
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        br#"{"requirements":{"os":"linux","architecture":"amd64","min_cpu_cores":1,"min_memory_bytes":1,"min_storage_bytes":1,"runtime":"oci","gpu":{"required":false,"min_memory_bytes":0,"runtime":""},"data_residency_zones":["us-west"],"minimum_trust_zone":"standard","sandbox_floor":"container","concurrency_slots":1}}"#,
    )
    .unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "placement-registry-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("registry placement preview"), "{output}");
    assert!(output.contains("selected=none"), "{output}");
    assert!(output.contains("placement_selected=false"), "{output}");
}

fn serve_registry_preview(listener: TcpListener) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut stream, request, _, body) = accept_request(&listener);
    assert!(request.starts_with("POST /api/v1/device-placement/registry-preview "));
    let posted: Value = serde_json::from_slice(&body).unwrap();
    assert!(posted.get("requirements").is_some());
    assert_eq!(posted.as_object().unwrap().len(), 1);
    respond(
        &mut stream,
        "200 OK",
        &json!({
            "schema_version": "forge.device-inventory-placement-evaluation/v2",
            "evaluation_mode": "offline_static_only",
            "source_schema_version": "forge.device-inventory-observation/v2",
            "evaluation_owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
            "evaluated_at_ms": 1800000000000_i64,
            "notice": "Every owner, state, timestamp, resource, GPU, reservation, residency, trust, sandbox, and concurrency value is an unverified caller declaration. This read-only comparison selects no target and grants no execution authority.",
            "decisions": [], "eligible_candidate_count": 0,
            "selected_device_id": null, "selected_instance_id": null,
            "authority": {
                "identity_verified": false, "heartbeat_persisted": false,
                "inventory_authoritative": false, "placement_selected": false,
                "reservation_created": false, "execution_authorized": false,
                "dispatch_performed": false
            }
        }),
    );
}

#[tokio::test]
async fn remote_tui_scheduler_selection_preview_posts_bound_request_and_renders_closed_authority() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/scheduler-preview "));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posted["conversation_id"], "conversation-1");
        assert_eq!(posted["run_id"], "run-1");
        assert_eq!(posted["attempt_id"], "attempt-1");
        respond(
            &mut stream,
            "200 OK",
            &json!({
                "schema_version": "forge.scheduler-selection-preview/v1",
                "evaluation_mode": "pure_scheduler_selection_preview",
                "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
                "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
                "evaluated_at_ms": 1800000000000_i64,
                "candidate_count": 1, "eligible_candidate_count": 0,
                "selection_available": false, "selection_reason": "no_eligible_candidate",
                "selected_device_id": null, "selected_instance_id": null, "preview_only": true,
                "authority": {
                    "placement_selected": false, "reservation_created": false, "lease_issued": false,
                    "execution_authorized": false, "dispatch_performed": false, "audit_published": false
                }
            }),
        );
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), SCHEDULER_SELECTION_INPUT).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "scheduler-selection-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("scheduler selection preview"), "{output}");
    assert!(output.contains("selected=none"), "{output}");
    assert!(output.contains("lease_issued=false"), "{output}");
}
