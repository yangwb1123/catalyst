use super::*;

#[tokio::test]
async fn remote_tui_can_preview_placement_batch_from_a_file_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-batch-evaluation-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory placement-batch-evaluation --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline placement batch evaluation"),
        "{output}"
    );
    assert!(output.contains("decisions=6"), "{output}");
    assert!(output.contains("selected=none"), "{output}");
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_persisted_placement_from_a_file_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory placement-evaluation --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains("offline placement evaluation"), "{output}");
    assert!(output.contains("source=online"), "{output}");
    assert!(output.contains("device=device-a"), "{output}");
    assert!(output.contains("instance=runner-a"), "{output}");
    assert!(output.contains("matches=false"), "{output}");
    assert!(
        output.contains("authority: placement_evaluated=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_v2_placement_from_a_file_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-device-inventory-placement-evaluation-v2.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "inventory placement-evaluation-v2 --input {}\nquit\n",
        fixture.display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline placement evaluation [offline_static_only]"),
        "{output}"
    );
    assert!(output.contains("reservation=reserved"), "{output}");
    assert!(output.contains("gpus=2"), "{output}");
    assert!(output.contains("selected=none"), "{output}");
    assert!(
        output.contains("authority: identity_verified=false"),
        "{output}"
    );
    assert!(!output.contains("/api/v1/devices"), "{output}");
}
