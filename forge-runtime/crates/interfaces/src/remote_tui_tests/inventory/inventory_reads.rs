use super::*;

#[tokio::test]
async fn remote_tui_can_show_bounded_offline_inventory_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory show --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline device inventory [forge.device-inventory-observation/v1]"));
    assert!(output.contains("authority: identity_verified=false"));
    assert!(output.contains("device-a / runner-a"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_read_owner_bound_inventory_candidate() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v1.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = super::super::helpers::accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/devices "), "{request}");
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::super::helpers::respond(&mut stream, "200 OK", &fixture);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("remote device inventory [forge.device-inventory-observation/v1]"));
    assert!(output.contains("device-a / runner-a"));
    assert!(output.contains("authority: identity_verified=false"));
}

#[tokio::test]
async fn remote_tui_can_read_owner_bound_lossless_v2_inventory_candidate() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../../docs/contracts/fixtures/forge-device-inventory-observation-v2.json"
    ))
    .unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = super::super::helpers::accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/devices/observations/v2 "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        super::super::helpers::respond(&mut stream, "200 OK", &fixture);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("inventory read-v2\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("remote device inventory [forge.device-inventory-observation/v2]"),
        "{output}"
    );
    assert!(output.contains("device-a / runner-a"), "{output}");
    assert!(
        output.contains("revision=1 generation=1 heartbeat=1"),
        "{output}"
    );
    assert!(output.contains("reservation=reserved"), "{output}");
    assert!(output.contains("gpus=2"), "{output}");
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
}
