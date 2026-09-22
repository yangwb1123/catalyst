use std::{io::Cursor, net::TcpListener, path::PathBuf, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

#[tokio::test]
async fn remote_tui_can_preview_run_attempt_lease_dispatch_preflight_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json",
    );
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "run-attempt-lease-dispatch-preflight-preview --input {}\nquit\n",
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
            "offline Run/Attempt/lease preflight [forge.run-attempt-lease-dispatch-preflight/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("declarative_preflight_ready=true selected_target_id=null"));
    assert!(output.contains("dispatch_performed=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_posts_authenticated_run_attempt_lease_dispatch_preflight_for_selected_session()
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let expected_request = request.clone();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-attempt-lease-dispatch-preflight-v1.json"
    ))
    .unwrap();
    let expected_response = response.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(
            &listener,
            &json!({
                "conversations": [{
                    "conversation": {
                        "id": "conversation-001",
                        "scope": {"kind": "global"},
                        "title": "Shared",
                        "created_at_ms": 1,
                        "updated_at_ms": 1
                    },
                    "aggregate_version": 1
                }],
                "next_after_id": null,
                "has_more": false
            }),
        );
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/attempt-lease-dispatch-preflight/preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &expected_response);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "run-attempt-lease-dispatch-preflight-remote-preview --input {}\nquit\n",
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
            "Run/Attempt/lease dispatch preflight [forge.run-attempt-lease-dispatch-preflight/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("conversation=conversation-001 run=run-001 status=nonterminal"));
    assert!(output.contains("declarative_preflight_ready=true selected_target_id=null"));
    assert!(output.contains("dispatch_performed=false"));
    assert!(!output.contains("fence-001"));
    assert!(!output.contains("/api/v1/devices"));
}
