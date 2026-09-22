use std::{io::Cursor, net::TcpListener, path::Path, thread};

use serde_json::Value;

use super::super::{commands, state::TuiState};
use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn remote_tui_can_preview_credential_candidate_without_a_device_request() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-device-credential-candidate-v1.json");
    let mut writer = Vec::new();

    let exited = commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!("credential-candidate-preview --input {}", fixture.display()),
        &mut writer,
    )
    .await
    .unwrap();

    assert!(!exited);
    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains(
            "offline device credential candidate [forge.device-credential-lifecycle/v1] action=Issue device=device-1 revision=7"
        ),
        "{output}"
    );
    assert!(output.contains("credential=credential-1 key=key-1 generation=1"));
    assert!(output.contains("authority: owner_binding_matched=false"));
}

#[tokio::test]
async fn remote_tui_posts_credential_candidate_only_after_explicit_command() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(
            request.starts_with("POST /api/v1/device-enrollment-heartbeat/credential-candidate "),
            "{request}"
        );
        assert!(headers.contains("authorization: bearer test-token"));
        let posted: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(posted["device_id"], "device-1");
        assert_eq!(posted["action"], "issue");
        assert!(posted.get("credential_material").is_none());
        let response: Value = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-device-credential-candidate-v1.json"
        ))
        .unwrap();
        respond(&mut stream, "200 OK", &response);
    });

    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        input.path(),
        br#"{"device_id":"device-1","action":"issue","approval_state":"approved","credential_id":"credential-1","key_id":"key-1","public_key_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","key_generation":1,"issued_at_ms":100,"expires_at_ms":1100,"next_credential_id":"","next_key_id":"","next_public_key_sha256":"","observed_at_ms":100,"expected_device_revision":7}"#,
    )
    .unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "credential-candidate --input {}\nquit\n",
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
            "remote device credential candidate [forge.device-credential-lifecycle/v1] action=Issue device=device-1 revision=7"
        ),
        "{output}"
    );
    assert!(output.contains("credential=credential-1 key=key-1 generation=1"));
    assert!(output.contains("credential_material_made=false"));
}
