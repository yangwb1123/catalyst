use super::*;

#[tokio::test]
async fn remote_tui_can_preview_client_instance_session_view_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-session-view-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-session-view-preview --input {}\nquit\n",
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
            "offline client-instance/session-view [forge.client-instance-session-view/v1]"
        ),
        "{output}"
    );
    assert!(output.contains(
        "instance client-cli-001: client_kind=cli status=active observed_at_ms=200500 sessions=conversation-001,conversation-002"
    ));
    assert!(output.contains(
        "instance client-web-001: client_kind=web status=idle observed_at_ms=200500 sessions=conversation-001"
    ));
    assert!(output.contains("read_only=true"));
    assert!(output.contains("prompt_write_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_preview_client_instance_resource_view_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-client-instance-resource-view-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "client-instance-resource-view-preview --input {}\nquit\n",
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
            "offline client-instance/resource-view [forge.client-instance-resource-view/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("instance client-cli-001: client_kind=cli"));
    assert!(
        output.contains("device device-a: runner=runner-a revision=1 generation=1 heartbeat=1")
    );
    assert!(output.contains("read_only=true device_attributes_unverified=true"));
    assert!(output.contains("prompt_write_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}
