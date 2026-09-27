use std::{
    io::Cursor,
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::super::{commands::dispatch_command, state::TuiState};
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

const SCHEDULER_SELECTION_INPUT: &str = r#"{
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
}"#;

const SCHEDULER_SELECTION_LEASE_INPUT: &str = r#"{
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
}"#;

const SCHEDULER_SELECTION_LEASE_RENEWAL_INPUT: &str = r#"{
  "conversation_id": "conversation-1",
  "run_id": "run-1",
  "attempt_id": "attempt-1",
  "target_id": "runner-a",
  "epoch": 1,
  "fencing_token": "fence-token-a",
  "ttl_ms": 30000
}"#;

const SCHEDULER_SELECTION_LEASE_RELEASE_INPUT: &str = r#"{
  "conversation_id": "conversation-1",
  "run_id": "run-1",
  "attempt_id": "attempt-1",
  "target_id": "runner-a",
  "epoch": 2,
  "fencing_token": "fence-token-b"
}"#;

fn canonical_inventory_resource_pair() -> (Value, Value) {
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .expect("inventory/resource convergence fixture");
    (pair["inventory"].clone(), pair["resource_view"].clone())
}

fn visible_scheduler_lease_state(inventory: Value, resource: Value) -> TuiState {
    let mut session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .expect("client-instance session-view fixture");
    // The session and resource readers must begin with one converged local
    // display image; the request refresh below is then the boundary under
    // test.
    session["instances"] = resource["instances"].clone();
    TuiState {
        client_instance_filter: Some("client-web-001".into()),
        client_instance_session_view_observed: Some(session),
        client_instance_resource_view_observed: Some(resource),
        device_inventory_v2_observed: Some(inventory),
        ..TuiState::default()
    }
}

fn scheduler_lease_response(conversation_id: &str, epoch: u64, fencing_token: &str) -> Value {
    json!({
        "schema_version": "forge.execution-lease-registry/v1",
        "evaluation_mode": "durable_scheduler_lease_claim",
        "owner": {"issuer":"https://id.example","subject":"user-1","tenant_id":"tenant-1"},
        "conversation_id": conversation_id, "run_id": "run-1", "attempt_id": "attempt-1",
        "device_id": "device-a", "instance_id": "runner-a",
        "inventory_revision": 3, "generation": 2, "heartbeat_sequence": 4,
        "grant": {
            "v": 1, "attempt_id": "attempt-1", "target_id": "runner-a", "epoch": epoch,
            "fencing_token": fencing_token, "issued_at_ms": 1800000000000_i64,
            "expires_at_ms": 1800000030000_i64
        },
        "replayed": false,
        "authority": {
            "placement_selected": true, "reservation_created": true, "lease_issued": true,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    })
}

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

#[tokio::test]
async fn remote_tui_scheduler_selection_preview_accepts_a_declared_client_instance_conversation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut session_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let web = session_response["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["instance_id"] == "client-web-001")
        .unwrap();
    web["session_ids"] = json!(["c-1", "conversation-1"]);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_response);

        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/scheduler-preview "));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posted["conversation_id"], "conversation-1");
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
        "client-instances session-view\ninstance client-web-001\nscheduler-selection-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Client-instance filter set to \"client-web-001\""),
        "{output}"
    );
    assert!(output.contains("scheduler selection preview"), "{output}");
}

#[tokio::test]
async fn remote_tui_scheduler_selection_preview_rechecks_visibility_after_refresh() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let pair: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-device-inventory-resource-convergence-v1.json"
    ))
    .unwrap();
    let inventory = pair["inventory"].clone();
    let resource = pair["resource_view"].clone();
    // The initial client-instance pair must be the same image; the final
    // refresh below is the only deliberate session/resource divergence.
    session["instances"] = resource["instances"].clone();
    let mut revoked_resource = resource.clone();
    revoked_resource["instances"][0]["session_ids"] = json!(["conversation-002"]);

    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &session);

        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &resource);

        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &inventory);

        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &resource);

        // The planning boundary refreshes the explicit pair. Its resource
        // image revokes this Conversation from the selected instance, so a
        // subsequent scheduler POST would be a fail-open.
        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &inventory);

        let (mut stream, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &revoked_resource);
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_INPUT.replace("conversation-1", "conversation-001"),
    )
    .unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances show-converged\ninstance client-web-001\ninventory show-converged\nscheduler-selection-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Conversation read blocked by client-instance display filter: no validated view is available"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
    assert!(
        !output.contains("scheduler selection preview ["),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_scheduler_selection_preview_blocks_a_foreign_client_instance_conversation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let mut session_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json"
    ))
    .unwrap();
    let web = session_response["instances"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|instance| instance["instance_id"] == "client-web-001")
        .unwrap();
    web["session_ids"] = json!(["c-1"]);
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut session, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/session-view "));
        assert!(body.is_empty());
        respond(&mut session, "200 OK", &session_response);
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_INPUT.replace("conversation-1", "conversation-2"),
    )
    .unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instances session-view\ninstance client-web-001\nscheduler-selection-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "Conversation read blocked by client-instance display filter: conversation \"conversation-2\" is not declared"
        ),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
    assert!(
        !output.contains("scheduler selection preview ["),
        "{output}"
    );
}

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_refreshes_explicit_inventory_resource_before_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, resource) = canonical_inventory_resource_pair();
    let server_inventory = inventory.clone();
    let server_resource = resource.clone();
    let server = thread::spawn(move || {
        let (mut inventory_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut inventory_response, "200 OK", &server_inventory);

        let (mut resource_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource_response, "200 OK", &server_resource);

        let (mut lease, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/scheduler-lease "));
        assert!(headers.contains("idempotency-key: forge-tui-"));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posted["conversation_id"], "conversation-001");
        respond(
            &mut lease,
            "200 OK",
            &scheduler_lease_response("conversation-001", 1, "fence-token-a"),
        );
    });

    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_LEASE_INPUT.replace("conversation-1", "conversation-001"),
    )
    .unwrap();
    let mut state = visible_scheduler_lease_state(inventory, resource);
    let client = test_client(address);
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "scheduler-selection-lease --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("scheduler lease"), "{output}");
    assert!(output.contains("lease_issued=true"), "{output}");
    assert!(output.contains("fencing token withheld"), "{output}");
}

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_renewal_drift_blocks_post() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (inventory, resource) = canonical_inventory_resource_pair();
    let mut drifted_resource = resource.clone();
    drifted_resource["devices"][0]["heartbeat_sequence"] = json!(99);
    let server_inventory = inventory.clone();
    let server_drifted_resource = drifted_resource;
    let server = thread::spawn(move || {
        let (mut inventory_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices/observations/v2 "));
        assert!(body.is_empty());
        respond(&mut inventory_response, "200 OK", &server_inventory);

        let (mut resource_response, request, _, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/client-instances/resource-view "));
        assert!(body.is_empty());
        respond(&mut resource_response, "200 OK", &server_drifted_resource);

        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let (request, _, _) = super::helpers::read_request(&mut stream);
                    panic!(
                        "scheduler lease renewal POST escaped inventory/resource drift: {request}"
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("checking for unexpected scheduler lease request: {error}"),
            }
        }
    });

    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        SCHEDULER_SELECTION_LEASE_RENEWAL_INPUT.replace("conversation-1", "conversation-001"),
    )
    .unwrap();
    let mut state = visible_scheduler_lease_state(inventory, resource);
    let client = test_client(address);
    let mut writer = Vec::new();
    dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "scheduler-selection-lease-renew --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Scheduler lease renewal inventory/resource refresh failed"),
        "{output}"
    );
    assert!(output.contains("No request was sent."), "{output}");
}

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_posts_once_and_withholds_fencing_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/scheduler-lease "));
        assert!(headers.contains("idempotency-key: forge-tui-"));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posted["conversation_id"], "conversation-1");
        assert_eq!(posted["ttl_ms"], 30000);
        respond(
            &mut stream,
            "200 OK",
            &json!({
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
            }),
        );
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), SCHEDULER_SELECTION_LEASE_INPUT).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "scheduler-selection-lease --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("scheduler lease"), "{output}");
    assert!(output.contains("lease_issued=true"), "{output}");
    assert!(output.contains("fencing token withheld"), "{output}");
    assert!(!output.contains("fence-token-a"), "{output}");
}

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_renewal_posts_once_and_withholds_fencing_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/scheduler-lease/renew "));
        assert!(headers.contains("idempotency-key: forge-tui-"));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posted["target_id"], "runner-a");
        assert_eq!(posted["epoch"], 1);
        respond(
            &mut stream,
            "200 OK",
            &json!({
                "schema_version": "forge.execution-lease-registry/v1",
                "evaluation_mode": "durable_scheduler_lease_claim",
                "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
                "conversation_id": "conversation-1", "run_id": "run-1", "attempt_id": "attempt-1",
                "device_id": "device-a", "instance_id": "runner-a",
                "inventory_revision": 7, "generation": 3, "heartbeat_sequence": 12,
                "grant": {
                    "v": 1, "attempt_id": "attempt-1", "target_id": "runner-a", "epoch": 2,
                    "fencing_token": "fence-token-b", "issued_at_ms": 1800000000000_i64,
                    "expires_at_ms": 1800000030000_i64
                },
                "replayed": false,
                "authority": {
                    "placement_selected": true, "reservation_created": true, "lease_issued": true,
                    "execution_authorized": false, "dispatch_performed": false, "audit_published": false
                }
            }),
        );
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), SCHEDULER_SELECTION_LEASE_RENEWAL_INPUT).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "scheduler-selection-lease-renew --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("scheduler lease"), "{output}");
    assert!(output.contains("epoch=2"), "{output}");
    assert!(output.contains("fencing token withheld"), "{output}");
    assert!(!output.contains("fence-token-b"), "{output}");
}

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_release_posts_once_and_withholds_fencing_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("POST /api/v1/device-placement/scheduler-lease/release "));
        assert!(headers.contains("idempotency-key: forge-tui-"));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posted["target_id"], "runner-a");
        assert_eq!(posted["epoch"], 2);
        respond(
            &mut stream,
            "200 OK",
            &json!({
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
            }),
        );
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), SCHEDULER_SELECTION_LEASE_RELEASE_INPUT).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "scheduler-selection-lease-release --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("scheduler lease release"), "{output}");
    assert!(output.contains("epoch=2"), "{output}");
    assert!(output.contains("fencing token withheld"), "{output}");
    assert!(!output.contains("fence-token-b"), "{output}");
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
