use std::{io::Cursor, net::TcpListener, thread};

use serde_json::json;

use super::{
    TuiState,
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn remote_tui_execution_consent_preview_uses_selected_session_and_gets_metadata_only() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request, headers, body) = accept_request(&listener);
        assert!(request.starts_with("GET /api/v1/conversations/c-1/execution-consents "));
        assert!(headers.contains("authorization: bearer test-token"));
        assert!(body.is_empty());
        respond(
            &mut stream,
            "200 OK",
            &json!({
                "conversation_id": "c-1",
                "project_id": "project-1",
                "profile_id": "profile-reviewed-v1",
                "profile_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "maximum_ttl_ms": 2592000000_u64
            }),
        );
    });

    let client = test_client(address);
    let mut reader = Cursor::new("execution-consent-preview\nquit\n");
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("execution-consent preview [read-only candidate]"),
        "{output}"
    );
    assert!(output.contains(
        "conversation=c-1 project=project-1 profile=profile-reviewed-v1 profile_sha256=0123456789abcdef"
    ));
    assert!(output.contains("No consent was granted"));
    assert!(!output.contains("grant_id"));
    assert!(!output.contains("fencing_token"));
}

#[tokio::test]
async fn remote_tui_execution_consent_preview_requires_selected_session() {
    let client = test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let mut writer = Vec::new();
    let exited = super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        "execution-consent-preview",
        &mut writer,
    )
    .await
    .unwrap();
    assert!(!exited);
    assert!(
        String::from_utf8(writer)
            .unwrap()
            .contains("Open a session before viewing its execution-consent preview.")
    );
}
