use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::TuiState;

fn fixture_text() -> String {
    include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-reconciliation-projection-v1.json"
    )
    .to_owned()
}

#[tokio::test]
async fn remote_session_runner_reconciliation_tui_consumes_projection_without_a_request() {
    let file = NamedTempFile::new().unwrap();
    std::fs::write(file.path(), fixture_text()).unwrap();
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let mut writer = Vec::new();

    let exited = super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "session-runner-reconciliation-preview --input {}",
            file.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();

    assert!(!exited);
    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains(
        "session Runner reconciliation projection [forge.session-runner-reconciliation-projection/v1]"
    ));
    assert!(output.contains("kind=manual"));
    assert!(output.contains("automatic_retry=false"));
    assert!(output.contains("selected_target=none"));
    assert!(output.contains("receipt_persisted=false"));
}

#[tokio::test]
async fn remote_session_runner_reconciliation_tui_rejects_projection_drift_locally() {
    let mut fixture: serde_json::Value = serde_json::from_str(&fixture_text()).unwrap();
    fixture["automatic_retry"] = serde_json::json!(true);
    let file = NamedTempFile::new().unwrap();
    std::fs::write(file.path(), serde_json::to_vec(&fixture).unwrap()).unwrap();
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    let mut writer = Vec::new();

    super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "session-runner-reconciliation-preview --input {}",
            file.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();

    assert!(
        String::from_utf8(writer)
            .unwrap()
            .contains("projection is invalid")
    );
}

#[tokio::test]
async fn remote_session_runner_reconciliation_tui_rejects_a_different_selected_run() {
    let input = NamedTempFile::new().unwrap();
    let request: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
    ))
    .unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = super::test_client("127.0.0.1:1".parse().unwrap());
    let mut state = TuiState::default();
    state.selected_id = Some("conversation-001".into());
    state.selected_run_id = Some("run-other".into());
    let mut writer = Vec::new();

    super::super::commands::dispatch_command(
        &client,
        &mut state,
        None,
        &format!(
            "session-runner-reconciliation-remote-preview --input {}",
            input.path().display()
        ),
        &mut writer,
    )
    .await
    .unwrap();

    assert!(
        String::from_utf8(writer)
            .unwrap()
            .contains("requires the selected Run to match Run run-001")
    );
}

#[tokio::test]
async fn remote_session_runner_reconciliation_tui_canonicalizes_history_before_projection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let request: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
    ))
    .unwrap();
    let expected_request = request.clone();
    let canonical_response = request.clone();
    let expected_response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-reconciliation-projection-v1.json"
    ))
    .unwrap();
    let response_for_server = expected_response.clone();
    let server = thread::spawn(move || {
        super::helpers::serve_conversation_page(
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
        let (mut history_stream, history_request_line, history_headers, history_body) =
            super::helpers::accept_request(&listener);
        assert!(history_request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/runner-receipt-history/preview "
        ));
        assert!(history_headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&history_body).unwrap(),
            expected_request
        );
        super::helpers::respond(&mut history_stream, "200 OK", &canonical_response);

        let (mut stream, request_line, headers, body) = super::helpers::accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/conversation-001/runs/run-001/runner-reconciliation/preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        super::helpers::respond(&mut stream, "200 OK", &response_for_server);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = super::test_client(address);
    let mut reader = Cursor::new(format!(
        "session-runner-reconciliation-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    super::run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(output.contains(
        "session Runner reconciliation projection [forge.session-runner-reconciliation-projection/v1]"
    ));
    assert!(output.contains("kind=manual"));
    assert!(output.contains("automatic_retry=false"));
    assert!(output.contains("selected_target=none"));
    assert!(!output.contains("/api/v1/devices"));
}
