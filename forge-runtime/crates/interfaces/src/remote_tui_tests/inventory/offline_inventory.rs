use super::*;

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_inventory_status_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-status-contract-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory status --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline device inventory status [forge.device-inventory-status-contract/v1]"
        )
    );
    assert!(output.contains("approved_online_is_declared_eligible: status=online"));
    assert!(output.contains("future_snapshot_rejected: error=snapshot_from_future"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_inventory_persistence_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-persistence-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persistence-preview --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline device inventory persistence preview [forge.device-inventory-persistence/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("project_online: accepted=true revision=3"),
        "{output}"
    );
    assert!(
        output.contains("revision_conflict: accepted=false error=revision_conflict"),
        "{output}"
    );
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_inventory_snapshot_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-snapshot-canonical-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory snapshot-canonical --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline device inventory snapshot canonical"));
    assert!(output.contains("sorts_by_device_then_instance_without_mutating_input"));
    assert!(output.contains("owner_mismatch"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_resource_summary_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-resource-summary-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory resource-summary --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline device resource summary [forge.device-resource-summary/v1]"));
    assert!(output.contains(
        "resources: devices=9 runner_instances=9 cpu=66 memory=135168 storage=67584 gpus=1 gpu_memory=4096 eligible_devices=2 eligible_instances=2"
    ));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(!output.contains("/api/v1/devices"));
}
