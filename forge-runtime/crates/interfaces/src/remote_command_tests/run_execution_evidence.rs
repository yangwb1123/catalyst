use forge_runtime_domain::run_execution_evidence::{
    RunExecutionEvidenceInput, observe_run_execution_evidence,
};
use serde_json::{Value, json};

use super::*;

fn request() -> Value {
    let evidence: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-run-execution-evidence-v1.json"
    ))
    .expect("Run execution evidence fixture");
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json"
    ))
    .expect("session receipt fixture");
    json!({
        "run_observed": {
            "api_version": "forge.run.observed.v1",
            "owner_ref": evidence["owner_ref"].clone(),
            "conversation_id": evidence["conversation_id"].clone(),
            "run_id": evidence["run_id"].clone(),
            "prompt_id": evidence["prompt_id"].clone(),
            "created_at_ms": 200,
            "latest_sequence": 5,
            "status": evidence["run_status"].clone(),
            "metadata_observed": true,
            "content_included": false,
            "authority": {
                "identity_verified": false,
                "owner_authorized": false,
                "run_authoritative": false,
                "persistence_attested": false,
                "content_provenance_verified": false,
                "reservation_created": false,
                "execution_authorized": false,
                "dispatch_performed": false
            }
        },
        "session_receipt_observed": receipt
    })
}

fn response(request: &Value) -> Value {
    let run = serde_json::from_value(request["run_observed"].clone()).unwrap();
    let receipt = serde_json::from_value(request["session_receipt_observed"].clone()).unwrap();
    serde_json::to_value(
        observe_run_execution_evidence(RunExecutionEvidenceInput { run, receipt }).unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn run_execution_evidence_preview_posts_the_bound_pair_once() {
    let request = request();
    let expected_response = response(&request);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/execution-evidence/preview ",
        required_headers: &[],
        body_fields: json!({
            "run_observed": request["run_observed"].clone(),
            "session_receipt_observed": request["session_receipt_observed"].clone(),
        }),
        response_status: "200 OK",
        response: expected_response.clone(),
    }]);
    let returned = client
        .preview_run_execution_evidence("conversation-001", "run-001", &request)
        .await
        .expect("Run execution evidence response");
    super::super::run_execution_evidence::validate_response(
        &returned,
        &request,
        "conversation-001",
        "run-001",
    )
    .expect("strict Run execution evidence response");
    assert_eq!(returned["disposition_kind"], "completed");
    assert_eq!(returned["content_included"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn run_execution_evidence_preview_rejects_response_drift_at_client_boundary() {
    let request = request();
    let mut forged = response(&request);
    forged["run_id"] = json!("run-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/execution-evidence/preview ",
        required_headers: &[],
        body_fields: json!({
            "run_observed": request["run_observed"].clone(),
            "session_receipt_observed": request["session_receipt_observed"].clone(),
        }),
        response_status: "200 OK",
        response: forged,
    }]);
    let error = client
        .preview_run_execution_evidence("conversation-001", "run-001", &request)
        .await
        .expect_err("foreign evidence must not escape the HTTP client");
    assert!(error.to_string().contains("binding or value drift"));
    server.join().unwrap();
}

#[tokio::test]
async fn run_execution_evidence_preview_rejects_request_url_drift_before_post() {
    let request = request();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_run_execution_evidence("conversation-001", "run-foreign", &request)
        .await
        .expect_err("a request bound to another Run must fail before transport");
    assert_eq!(
        error.to_string(),
        "Run execution evidence request does not match its URL"
    );
}

#[test]
fn run_execution_evidence_response_rejects_binding_authority_and_content_drift() {
    let request = request();
    let response = response(&request);
    let mut foreign = response.clone();
    foreign["run_id"] = json!("run-foreign");
    assert!(
        super::super::run_execution_evidence::validate_response(
            &foreign,
            &request,
            "conversation-001",
            "run-001"
        )
        .is_err()
    );
    let mut authority = response.clone();
    authority["authority"]["dispatch_performed"] = json!(true);
    assert!(
        super::super::run_execution_evidence::validate_response(
            &authority,
            &request,
            "conversation-001",
            "run-001"
        )
        .is_err()
    );
    let mut content = response;
    content["content_included"] = json!(true);
    assert!(
        super::super::run_execution_evidence::validate_response(
            &content,
            &request,
            "conversation-001",
            "run-001"
        )
        .is_err()
    );
}

#[test]
fn run_execution_evidence_input_rejects_unknown_and_duplicate_fields() {
    let request = serde_json::to_string(&request()).unwrap();
    let unknown = request.replacen(
        "\"session_receipt_observed\"",
        "\"unexpected\":{},\"session_receipt_observed\"",
        1,
    );
    let unknown_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(unknown_file.path(), unknown).unwrap();
    assert!(
        super::super::run_execution_evidence::read_request(unknown_file.path().to_str().unwrap())
            .is_err()
    );
    let duplicate = request.replacen(
        "\"run_id\":\"run-001\"",
        "\"run_id\":\"run-001\",\"run_id\":\"run-001\"",
        1,
    );
    let duplicate_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(duplicate_file.path(), duplicate).unwrap();
    assert!(
        super::super::run_execution_evidence::read_request(duplicate_file.path().to_str().unwrap())
            .is_err()
    );
}
