use std::{io::Cursor, net::TcpListener, thread};

use serde_json::{Value, json};
use tempfile::NamedTempFile;

use super::{
    helpers::{accept_request, conversation_page, respond, serve_conversation_page, test_client},
    run_with_io,
};

fn local_preview_fixture() -> (Value, Value) {
    let mut intent: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-execution-intent-v1.json"
    ))
    .expect("runner intent fixture");
    let intent_object = intent.as_object_mut().expect("intent object");
    for field in ["schema_version", "evaluation_mode", "authority", "expected"] {
        intent_object.remove(field);
    }
    intent["conversation_id"] = json!("c-1");
    intent["prompt_receipt"]["conversation_id"] = json!("c-1");
    intent["run_reference"]["conversation_id"] = json!("c-1");
    intent["execution_intent"]["conversation_id"] = json!("c-1");

    let request = json!({
        "intent": intent,
        "grant": {
            "v": 1,
            "attempt_id": "attempt-001",
            "target_id": "runner-1",
            "epoch": 1,
            "fencing_token": "fence-001",
            "issued_at_ms": 100,
            "expires_at_ms": 10100
        },
        "observed_at_ms": 300
    });
    let intent = &request["intent"];
    let mut session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json"
    ))
    .expect("session receipt fixture");
    session["conversation_id"] = json!("c-1");
    let response = json!({
        "schema_version": "forge.runner-local-execution-preview/v1",
        "evaluation_mode": "injected_local_runner_preview_only",
        "runner_execution_intent": {
            "schema_version": "forge.runner-execution-intent/v1",
            "evaluation_mode": "pure_runner_binding_only",
            "owner": intent["owner"].clone(),
            "conversation_id": "c-1",
            "prompt_id": intent["prompt_receipt"]["prompt_id"].clone(),
            "run_id": intent["run_reference"]["run_id"].clone(),
            "attempt_id": intent["execution_intent"]["attempt_id"].clone(),
            "command_id": intent["execution_intent"]["command_id"].clone(),
            "target_id": intent["execution_intent"]["target_id"].clone(),
            "command_sha256": intent["execution_intent"]["command_sha256"].clone(),
            "idempotency_key": intent["execution_intent"]["idempotency_key"].clone(),
            "prompt_run_binding_valid": true,
            "runner_command_binding_valid": true,
            "preview_only": true,
            "selected_target_id": null,
            "authority": {
                "device_identity_verified": false,
                "command_persisted": false,
                "reservation_created": false,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }
        },
        "session_runner_receipt": session,
        "command_id": "command-001",
        "attempt_id": "attempt-001",
        "target_id": "runner-1",
        "command_sha256": intent["execution_intent"]["command_sha256"].clone(),
        "disposition_kind": "completed",
        "observed_at_ms": 300,
        "output_bytes": 8,
        "exit_code": 0,
        "executor_invoked": true,
        "preview_only": true,
        "authority": {
            "device_identity_verified": false,
            "command_persisted": false,
            "reservation_created": false,
            "execution_authorized": false,
            "dispatch_performed": false,
            "audit_published": false
        }
    });
    (request, response)
}

#[tokio::test]
async fn remote_tui_local_runner_preview_uses_selected_session_and_metadata_only_output() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request, response) = local_preview_fixture();
    let expected_request = request.clone();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request_line, headers, body) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/c-1/run-intents/intent-001/execution-readiness-preview "
        ));
        assert!(headers.contains("authorization: bearer test-token"));
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            expected_request
        );
        respond(&mut stream, "200 OK", &response);
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-execution-readiness-preview --input {}\nquit\n",
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
            "local Runner execution-readiness preview [forge.runner-local-execution-preview/v1]"
        ),
        "{output}"
    );
    assert!(output.contains("owner=user-1 conversation=c-1 prompt=prompt-001 run=run-001"));
    assert!(
        output.contains(
            "command=command-001 attempt=attempt-001 target=runner-1 disposition=completed"
        )
    );
    assert!(output.contains("preview_only=true executor_invoked=true"));
    assert!(output.contains("execution_authorized=false dispatch_performed=false"));
    assert!(!output.contains("forge-task"));
    assert!(!output.contains("fence-001"));
}

#[tokio::test]
async fn remote_tui_local_runner_preview_clears_session_after_authorization_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request, _) = local_preview_fixture();
    let server = thread::spawn(move || {
        serve_conversation_page(&listener, &conversation_page(1));
        let (mut stream, request_line, _, _) = accept_request(&listener);
        assert!(request_line.starts_with(
            "POST /api/v1/conversations/c-1/run-intents/intent-001/execution-readiness-preview "
        ));
        respond(
            &mut stream,
            "403 Forbidden",
            &json!({"error":"owner mismatch"}),
        );
    });
    let input = NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "runner-execution-readiness-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert!(
        output.contains("Local Runner preview request failed"),
        "{output}"
    );
    assert!(output.contains("Local session view cleared after authorization failure."));
    assert!(!output.contains("owner mismatch"));
}
