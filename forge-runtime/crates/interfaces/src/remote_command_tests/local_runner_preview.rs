use super::*;

#[test]
fn local_runner_preview_consumes_the_canonical_observation_fixture() {
    let request = request();
    let response: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-local-execution-preview-v1.json"
    ))
    .expect("canonical local Runner preview fixture");
    super::super::local_runner_preview::validate_response(
        &response,
        &request,
        "conversation-001",
        "intent-001",
    )
    .expect("canonical local Runner preview response");
    assert_eq!(response["output_bytes"], 8);
    assert_eq!(response["exit_code"], 0);
    assert_eq!(response["disposition_kind"], "completed");
    assert!(response["selected_target_id"].is_null());
    assert!(response.get("argv").is_none());
    assert!(response.get("workspace_ref").is_none());
    assert!(response.get("fencing_token").is_none());
}

fn request() -> Value {
    let mut intent: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-execution-intent-v1.json"
    ))
    .expect("runner intent fixture");
    let intent_object = intent.as_object_mut().expect("intent object");
    for field in ["schema_version", "evaluation_mode", "authority", "expected"] {
        intent_object.remove(field);
    }
    json!({
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
    })
}

fn response() -> Value {
    let request = request();
    let intent = &request["intent"];
    let session: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json"
    ))
    .expect("session receipt fixture");
    json!({
        "schema_version": "forge.runner-local-execution-preview/v1",
        "evaluation_mode": "injected_local_runner_preview_only",
        "runner_execution_intent": {
            "schema_version": "forge.runner-execution-intent/v1",
            "evaluation_mode": "pure_runner_binding_only",
            "owner": intent["owner"].clone(),
            "conversation_id": intent["conversation_id"].clone(),
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
    })
}

#[tokio::test]
async fn local_runner_preview_posts_the_bound_request_once_without_retry() {
    let request = request();
    let response = response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/run-intents/intent-001/execution-readiness-preview ",
        required_headers: &[],
        body_fields: request.clone(),
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_local_runner_execution_readiness("conversation-001", "intent-001", &request)
        .await
        .unwrap();
    super::super::local_runner_preview::validate_response(
        &returned,
        &request,
        "conversation-001",
        "intent-001",
    )
    .unwrap();
    assert_eq!(returned["output_bytes"], 8);
    assert!(returned.get("argv").is_none());
    assert!(returned.get("workspace_ref").is_none());
    assert!(returned.get("fencing_token").is_none());
    server.join().unwrap();
}

#[test]
fn local_runner_preview_rejects_binding_and_authority_drift() {
    let request = request();
    let mut authority_response = response();
    authority_response["authority"]["execution_authorized"] = json!(true);
    assert!(
        super::super::local_runner_preview::validate_response(
            &authority_response,
            &request,
            "conversation-001",
            "intent-001",
        )
        .is_err()
    );

    let mut identity_response = response();
    identity_response["command_id"] = json!("command-foreign");
    assert!(
        super::super::local_runner_preview::validate_response(
            &identity_response,
            &request,
            "conversation-001",
            "intent-001",
        )
        .is_err()
    );

    let mut shape_response = response();
    shape_response["runner_execution_intent"]["command"] = json!("must-not-be-accepted");
    assert!(
        super::super::local_runner_preview::validate_response(
            &shape_response,
            &request,
            "conversation-001",
            "intent-001",
        )
        .is_err()
    );
}

#[tokio::test]
async fn local_runner_preview_does_not_retry_an_uncertain_write() {
    let request = request();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let captured = capture_request(&mut stream);
        assert!(captured
            .line
            .starts_with("POST /api/v1/conversations/conversation-001/run-intents/intent-001/execution-readiness-preview "));
        write_json_response(
            &mut stream,
            "500 Internal Server Error",
            &json!({"error":"private"}),
        );
    });
    let client = test_remote_client(address);
    let error = client
        .preview_local_runner_execution_readiness("conversation-001", "intent-001", &request)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("HTTP 500"));
    server.join().unwrap();
}
