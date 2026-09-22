use std::{io::Cursor, net::TcpListener, thread};

use serde_json::Value;

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn remote_tui_reads_lifecycle_registry_only_after_explicit_command() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let response: Value = serde_json::json!({
        "schema_version": "forge.device-enrollment-heartbeat-lifecycle-file-set/v1",
        "owner": {"issuer":"https://id.example","subject":"user-a","tenant_id":"tenant-a"},
        "states": []
    });
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("GET /api/v1/device-enrollment-heartbeat/lifecycle-registry "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(&mut stream, "200 OK", &response);
    });

    let client = test_client(address);
    let mut reader = Cursor::new("lifecycle-registry show\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "remote lifecycle registry [forge.device-enrollment-heartbeat-lifecycle-file-set/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("states=0"), "{output}");
    assert!(output.contains("observation_only:"), "{output}");
}
