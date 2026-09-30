use super::*;

#[tokio::test]
async fn remote_tui_run_execution_evidence_preview_uses_the_authenticated_session_route() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request, response) = execution_evidence_request_and_response();
    let expected_request = request.clone();
    let server =
        thread::spawn(move || serve_execution_evidence(listener, response, expected_request));
    let input = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(input.path(), serde_json::to_vec(&request).unwrap()).unwrap();
    let client = test_client(address);
    let mut reader = Cursor::new(format!(
        "open c-1\nrun-execution-evidence-remote-preview --input {}\nquit\n",
        input.path().display()
    ));
    let mut writer = Vec::new();
    run_with_io(&client, &mut reader, &mut writer)
        .await
        .unwrap();
    server.join().unwrap();

    let output = String::from_utf8(writer).unwrap();
    assert_execution_evidence_output(&output);
}

fn serve_execution_evidence(listener: TcpListener, response: Value, expected_request: Value) {
    serve_conversation_page(&listener, &conversation_page(1));
    let (mut history, request, _, _) = accept_request(&listener);
    assert!(request.starts_with("GET /api/v1/conversations/c-1/prompts?"));
    respond(
        &mut history,
        "200 OK",
        &serde_json::json!({"conversation_id":"c-1","prompts":[],"has_more":false}),
    );
    let (mut stream, request_line, headers, body) = accept_request(&listener);
    assert!(
        request_line
            .starts_with("POST /api/v1/conversations/c-1/runs/run-1/execution-evidence/preview "),
        "{request_line}"
    );
    assert!(headers.contains("authorization: bearer test-token"));
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        expected_request
    );
    respond(&mut stream, "200 OK", &response);
}

fn assert_execution_evidence_output(output: &str) {
    assert!(
        output.contains("authenticated Run execution evidence [forge.run.execution-evidence.v1]"),
        "{output}"
    );
    assert!(output.contains("conversation=c-1 prompt=prompt-001 run=run-1 status=nonterminal"));
    assert!(output.contains("disposition=completed observed_at_ms=300"));
    assert!(
        output.contains("content_included=false uncertain=false reconciliation_required=false")
    );
    assert!(output.contains("execution_authorized=false dispatch_performed=false"));
    assert!(!output.contains("/api/v1/devices"));
}

fn execution_evidence_request_and_response() -> (Value, Value) {
    let receipt_fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let receipt: Value = serde_json::from_slice(&std::fs::read(&receipt_fixture).unwrap()).unwrap();
    let evidence_fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-run-execution-evidence-v1.json");
    let evidence: Value =
        serde_json::from_slice(&std::fs::read(&evidence_fixture).unwrap()).unwrap();
    let request = execution_evidence_request(&receipt, &evidence);
    let response = execution_evidence_response(&evidence);
    (request, response)
}

fn execution_evidence_request(receipt: &Value, evidence: &Value) -> Value {
    serde_json::json!({
        "run_observed": {
            "api_version": "forge.run.observed.v1",
            "owner_ref": evidence["owner_ref"].clone(),
            "conversation_id": "c-1",
            "run_id": "run-1",
            "prompt_id": "prompt-001",
            "created_at_ms": 200,
            "latest_sequence": 5,
            "status": "nonterminal",
            "metadata_observed": true,
            "content_included": false,
            "authority": {
                "identity_verified": false, "owner_authorized": false,
                "run_authoritative": false, "persistence_attested": false,
                "content_provenance_verified": false, "reservation_created": false,
                "execution_authorized": false, "dispatch_performed": false
            }
        },
        "session_receipt_observed": {
            "schema_version": receipt["schema_version"].clone(),
            "evaluation_mode": receipt["evaluation_mode"].clone(),
            "owner": receipt["owner"].clone(),
            "conversation_id": "c-1",
            "prompt_id": receipt["prompt_id"].clone(),
            "run_id": "run-1",
            "receipt_observation": receipt["receipt_observation"].clone(),
            "prompt_run_binding_valid": true,
            "receipt_binding_valid": true,
            "preview_only": true,
            "selected_target_id": Value::Null,
            "authority": receipt["authority"].clone()
        }
    })
}

fn execution_evidence_response(evidence: &Value) -> Value {
    serde_json::json!({
        "api_version": "forge.run.execution-evidence.v1",
        "evaluation_mode": "pure_run_execution_evidence_binding",
        "owner_ref": evidence["owner_ref"].clone(),
        "conversation_id": "c-1", "run_id": "run-1", "prompt_id": "prompt-001",
        "run_status": "nonterminal", "attempt_id": "attempt-001", "target_id": "runner-1",
        "command_id": "command-001", "command_sha256": evidence["command_sha256"].clone(),
        "disposition_kind": "completed", "receipt_observed_at_ms": 300,
        "uncertain": false, "reconciliation_required": false,
        "metadata_observed": true, "content_included": false,
        "authority": {
            "identity_verified": false, "owner_authorized": false, "run_authoritative": false,
            "receipt_persisted": false, "reservation_created": false,
            "execution_authorized": false, "dispatch_performed": false, "audit_published": false
        }
    })
}
