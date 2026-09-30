use super::*;

#[tokio::test]
async fn remote_tui_can_preview_a_session_runner_receipt_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-offline-preview --input {}\nquit\n",
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
            "offline session Runner terminal receipt observation [forge.session-runner-receipt-observation/v1]"
        ),
        "{output}"
    );
    assert!(
        output.contains("owner=user-1 conversation=conversation-001 prompt=prompt-001 run=run-001")
    );
    assert!(output.contains("receipt_command=command-001 attempt=attempt-001 target=runner-1"));
    assert!(output.contains("selected_target=none"));
    assert!(output.contains("receipt_persisted=false execution_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_can_reduce_session_runner_receipt_history_without_a_device_request() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json");
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-history-offline-preview --input {}\nquit\n",
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
            "offline session Runner receipt history [forge.session-runner-receipt-history/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("attempt_count=2"));
    assert!(output.contains(
        "attempt[1]: command=command-001 attempt=attempt-001 target=runner-1 disposition=failed"
    ));
    assert!(output.contains(
        "attempt[2]: command=command-002 attempt=attempt-002 target=runner-2 disposition=uncertain"
    ));
    assert!(output.contains("follow_up=reconciliation_manual"));
    assert!(output.contains("automatic_retry=false"));
    assert!(output.contains("selected_target=none"));
    assert!(output.contains("receipt_persisted=false execution_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_rejects_session_runner_receipt_history_summary_drift_locally() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
    });

    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json");
    let mut malformed: Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    malformed["attempt_count"] = json!(3);
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&malformed).unwrap()).unwrap();

    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-history-offline-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Session Runner receipt history preview failed:"),
        "{output}"
    );
    assert!(!output.contains("attempt_count=3"));
    assert!(!output.contains("/api/v1/devices"));
}

#[tokio::test]
async fn remote_tui_session_runner_receipt_preview_uses_the_authenticated_session_route() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let mut request: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    request["conversation_id"] = Value::String("c-1".into());
    request["run_id"] = Value::String("run-1".into());
    let expected_request = request.clone();
    let response = request.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/c-1/runs/run-1/runner-receipt-observation/preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &response);
    });
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-receipt-preview --input {}\nquit\n",
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
            "authenticated session Runner terminal receipt observation [forge.session-runner-receipt-observation/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("owner=user-1 conversation=c-1 prompt=prompt-001 run=run-1"));
    assert!(output.contains("receipt_command=command-001 attempt=attempt-001 target=runner-1"));
    assert!(output.contains("selected_target=none"));
    assert!(output.contains("receipt_persisted=false execution_authorized=false"));
    assert!(!output.contains("/api/v1/devices"));
}
