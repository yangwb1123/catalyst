use serde_json::{Value, json};

use super::{conversation_and_intent, read_request, validate_response};

fn request() -> Value {
    let fixture = serde_json::from_str::<Value>(include_str!(
        "../../../../../docs/contracts/fixtures/forge-runner-execution-intent-v1.json"
    ))
    .expect("Runner intent fixture");
    let intent = json!({
        "owner": fixture["owner"],
        "conversation_id": fixture["conversation_id"],
        "prompt_receipt": fixture["prompt_receipt"],
        "run_reference": fixture["run_reference"],
        "execution_intent": fixture["execution_intent"],
        "command": fixture["command"]
    });
    json!({
        "intent": intent,
        "grant": {
            "v": 1,
            "attempt_id": "attempt-001",
            "target_id": "runner-1",
            "epoch": 1,
            "fencing_token": "fence-001",
            "issued_at_ms": 1000,
            "expires_at_ms": 11000
        },
        "observed_at_ms": 2000
    })
}

fn response(request: &Value) -> Value {
    let intent = &request["intent"];
    let intent_observation = intent_observation(intent);
    let receipt = receipt(intent);
    json!({
        "schema_version": "forge.runner-local-execution-preview/v1",
        "evaluation_mode": "injected_local_runner_preview_only",
        "runner_execution_intent": intent_observation,
        "session_runner_receipt": {
            "schema_version": "forge.session-runner-receipt-observation/v1",
            "evaluation_mode": "pure_session_runner_receipt_binding_only",
            "owner": intent["owner"],
            "conversation_id": intent["conversation_id"],
            "prompt_id": intent["prompt_receipt"]["prompt_id"],
            "run_id": intent["run_reference"]["run_id"],
            "receipt_observation": receipt,
            "prompt_run_binding_valid": true,
            "receipt_binding_valid": true,
            "preview_only": true,
            "selected_target_id": null,
            "authority": {
                "identity_verified": false,
                "receipt_persisted": false,
                "execution_authorized": false,
                "dispatch_performed": false,
                "audit_published": false
            }
        },
        "command_id": intent["execution_intent"]["command_id"],
        "attempt_id": intent["execution_intent"]["attempt_id"],
        "target_id": intent["execution_intent"]["target_id"],
        "command_sha256": intent["execution_intent"]["command_sha256"],
        "disposition_kind": "completed",
        "observed_at_ms": 2000,
        "output_bytes": 0,
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

#[test]
fn request_is_bounded_and_path_bound() {
    let request = request();
    assert_eq!(
        conversation_and_intent(&request).expect("path identities"),
        ("conversation-001", "intent-001")
    );

    let duplicate = serde_json::to_string(&request)
        .expect("request JSON")
        .replacen(
            "\"observed_at_ms\":2000",
            "\"observed_at_ms\":2000,\"observed_at_ms\":2000",
            1,
        );
    let path = tempfile::NamedTempFile::new().expect("request file");
    std::fs::write(path.path(), duplicate).expect("write request");
    assert!(read_request(path.path().to_str().expect("request path")).is_err());
}

#[test]
fn response_requires_identity_and_false_authority() {
    let request = request();
    let response = response(&request);
    validate_response(&response, &request, "conversation-001", "intent-001")
        .expect("valid local preview response");

    let mut foreign = response.clone();
    foreign["runner_execution_intent"]["owner"]["subject"] = json!("other-user");
    assert!(validate_response(&foreign, &request, "conversation-001", "intent-001").is_err());

    let mut authority = response.clone();
    authority["authority"]["execution_authorized"] = json!(true);
    assert!(validate_response(&authority, &request, "conversation-001", "intent-001").is_err());

    assert!(validate_response(&response, &request, "conversation-002", "intent-001").is_err());
}

#[test]
fn human_output_contains_metadata_only() {
    let request = request();
    let response = response(&request);
    let mut output = Vec::new();
    super::render_human(&response, &mut output).expect("human output");
    let output = String::from_utf8(output).expect("UTF-8 output");
    assert!(output.contains("authority: device_identity_verified=false"));
    assert!(!output.contains("forge-task"));
    assert!(!output.contains("fence-001"));
}

fn intent_observation(intent: &Value) -> Value {
    json!({
        "schema_version": "forge.runner-execution-intent/v1",
        "evaluation_mode": "pure_runner_binding_only",
        "owner": intent["owner"],
        "conversation_id": intent["conversation_id"],
        "prompt_id": intent["prompt_receipt"]["prompt_id"],
        "run_id": intent["run_reference"]["run_id"],
        "attempt_id": intent["execution_intent"]["attempt_id"],
        "command_id": intent["execution_intent"]["command_id"],
        "target_id": intent["execution_intent"]["target_id"],
        "command_sha256": intent["execution_intent"]["command_sha256"],
        "idempotency_key": intent["execution_intent"]["idempotency_key"],
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
    })
}

fn receipt(intent: &Value) -> Value {
    json!({
        "schema_version": "forge.runner-command-terminal-receipt/v1",
        "evaluation_mode": "pure_runner_command_receipt_only",
        "command_id": intent["execution_intent"]["command_id"],
        "command_sha256": intent["execution_intent"]["command_sha256"],
        "attempt_id": intent["execution_intent"]["attempt_id"],
        "target_id": intent["execution_intent"]["target_id"],
        "disposition_kind": "completed",
        "observed_at_ms": 2000,
        "receipt_valid": true,
        "preview_only": true,
        "uncertain": false,
        "reconciliation_required": false,
        "manual_review_required": false,
        "automatic_retry": false,
        "follow_up": "none",
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
