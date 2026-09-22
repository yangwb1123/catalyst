use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

const PLACEMENT_INPUT: &str = r#"{
  "schema_version": "forge.device-placement-dry-run/v1",
  "evaluated_at_ms": 1800000000000,
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
    "snapshot_observed_at_ms": 1799999999000, "lease_expires_at_ms": 1800000060000,
    "os": "linux", "architecture": "amd64", "available_cpu_cores": 2,
    "available_memory_bytes": 4096, "available_storage_bytes": 4096,
    "runtimes": ["oci"], "gpu": {"present": false, "memory_bytes": 0, "runtime": ""},
    "data_residency_zones": ["us-west"], "trust_zone": "standard",
    "sandbox_levels": ["container"], "concurrency_limit": 2, "active_concurrency": 0
  }]
}"#;

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
    let server = thread::spawn(move || {
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
    });
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

#[tokio::test]
async fn remote_tui_clears_owner_view_after_placement_authorization_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "c-1",
                        "scope": {"kind": "global"},
                        "title": "Private session",
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut stream, request, _, _) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/preview "));
        respond(&mut stream, "403 Forbidden", &json!({"code": "forbidden"}));
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
    assert!(
        output.contains("Placement preview request failed: Forge API returned HTTP 403"),
        "{output}"
    );
    assert!(
        output.contains("Local session view cleared after authorization failure."),
        "{output}"
    );
    let last_render = output
        .rsplit("Forge shared sessions")
        .next()
        .unwrap_or_default();
    assert!(!last_render.contains("c-1"), "{last_render}");
    assert!(!last_render.contains("Private session"), "{last_render}");
}
