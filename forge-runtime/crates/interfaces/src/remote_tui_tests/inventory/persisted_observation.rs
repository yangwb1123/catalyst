use super::*;

#[tokio::test]
async fn remote_tui_can_preview_persisted_inventory_observation_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-persisted-observation-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persisted-observation --input {}\nquit\n",
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
            "offline persisted inventory observation [forge.device-inventory-observation/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("devices=2"), "{output}");
    assert!(
        output.contains("device-a / runner-a: approval=pending"),
        "{output}"
    );
    assert!(
        output.contains("cpu=7 memory=8192 storage=51200"),
        "{output}"
    );
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_can_preview_persisted_inventory_observation_v2_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persisted-observation-v2 --input {}\nquit\n",
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
            "offline persisted inventory observation v2 [forge.device-inventory-observation/v2]"
        ),
        "{output}"
    );
    assert!(output.contains("devices=2"), "{output}");
    assert!(output.contains("reservation=reserved"), "{output}");
    assert!(
        output.contains("gpus=[gpu-a:12884901888,gpu-b:4294967296]"),
        "{output}"
    );
    assert!(
        output.contains("authority: execution_authorized=false reservation_created=false dispatch_performed=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}

#[tokio::test]
async fn remote_tui_rejects_noncanonical_persisted_inventory_v2_tags() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let mut malformed: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
    ))
    .unwrap();
    malformed["devices"][0]["device"]["runtimes"] = json!(["oci/container"]);
    let mut file = NamedTempFile::new().unwrap();
    serde_json::to_writer(file.as_file_mut(), &malformed).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory persisted-observation-v2 --input {}\nquit\n",
        file.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Device inventory persisted-observation-v2 failed"),
        "{output}"
    );
    assert!(output.contains("canonical tag"), "{output}");
    assert!(!output.contains("/api/v1/devices"), "{output}");
}
