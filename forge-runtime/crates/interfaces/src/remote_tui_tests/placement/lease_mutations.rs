use super::*;

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_posts_once_and_withholds_fencing_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_lease_claim(listener));
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

fn serve_lease_claim(listener: TcpListener) {
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
}

#[tokio::test]
async fn remote_tui_scheduler_selection_lease_renewal_posts_once_and_withholds_fencing_token() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || serve_lease_renewal(listener));
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

fn serve_lease_renewal(listener: TcpListener) {
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
