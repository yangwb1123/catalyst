use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{
    accept_request,
    helpers::{serve_conversation_page, test_client},
    respond, run_with_io,
};

#[tokio::test]
async fn remote_tui_posts_runner_dispatch_plan_for_the_selected_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request = super::super::super::run_attempt_lease_dispatch_preflight::test_request();
    let expected_request = request["dispatch_plan"].clone();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-dispatch-plan-preview-v1.json"
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
            "POST /api/v1/conversations/conversation-001/runs/run-001/runner-dispatch-plan-preview "
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
        "runner-dispatch-plan-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("offline Runner dispatch-plan preview"),
        "{output}"
    );
    assert!(output.contains("selected_target=none"), "{output}");
    assert!(output.contains("dispatch_performed=false"), "{output}");
    assert!(!output.contains("fence-001"), "{output}");
    assert!(!output.contains("/api/v1/devices"), "{output}");
}
