use super::*;

use forge_runtime_domain::execution::reconciliation::{ReconciliationInput, observe};
use serde_json::{Value, json};

fn request() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-execution-reconciliation-observation-v1.json"
    ))
    .expect("execution reconciliation fixture");
    fixture["cases"][0]["input"].clone()
}

fn response(request: &Value) -> Value {
    let input: ReconciliationInput = serde_json::from_value(request.clone()).unwrap();
    serde_json::to_value(observe(input).unwrap()).unwrap()
}

#[tokio::test]
async fn execution_reconciliation_preview_posts_exact_restart_image_once() {
    let request = request();
    let expected_response = response(&request);
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/execution-reconciliation/preview ",
        required_headers: &[],
        body_fields: json!({
            "owner": request["owner"].clone(),
            "conversation_id": "conversation-1",
            "run_id": "run-1",
        }),
        response_status: "200 OK",
        response: expected_response.clone(),
    }]);
    let returned = client
        .preview_execution_reconciliation("conversation-1", "run-1", &request)
        .await
        .expect("execution reconciliation response");
    super::super::execution_reconciliation::validate_response(
        &returned,
        &request,
        "conversation-1",
        "run-1",
    )
    .expect("strict reconciliation response");
    assert_eq!(returned["next_observation"], "await_terminal");
    assert_eq!(returned["automatic_retry"], false);
    server.join().unwrap();
}

#[tokio::test]
async fn execution_reconciliation_client_rejects_a_response_for_another_run() {
    let request = request();
    let mut response = response(&request);
    response["run_id"] = json!("run-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-1/runs/run-1/execution-reconciliation/preview ",
        required_headers: &[],
        body_fields: json!({
            "owner": request["owner"].clone(),
            "conversation_id": "conversation-1",
            "run_id": "run-1",
        }),
        response_status: "200 OK",
        response,
    }]);
    assert!(
        client
            .preview_execution_reconciliation("conversation-1", "run-1", &request)
            .await
            .is_err()
    );
    server.join().unwrap();
}

#[test]
fn execution_reconciliation_preview_rejects_binding_and_authority_drift() {
    let request = request();
    let response = response(&request);

    let mut foreign = response.clone();
    foreign["run_id"] = json!("run-foreign");
    assert!(
        super::super::execution_reconciliation::validate_response(
            &foreign,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );

    let mut authority = response.clone();
    authority["authority"]["dispatch_performed"] = json!(true);
    assert!(
        super::super::execution_reconciliation::validate_response(
            &authority,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );

    let mut retry = response;
    retry["automatic_retry"] = json!(true);
    assert!(
        super::super::execution_reconciliation::validate_response(
            &retry,
            &request,
            "conversation-1",
            "run-1",
        )
        .is_err()
    );
}

#[test]
fn execution_reconciliation_preview_input_rejects_duplicate_and_missing_terminal() {
    let request = serde_json::to_string(&request()).unwrap();
    let duplicate = request.replacen(
        "\"run_id\":\"run-1\"",
        "\"run_id\":\"run-1\",\"run_id\":\"run-1\"",
        1,
    );
    let duplicate_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(duplicate_file.path(), duplicate).unwrap();
    assert!(
        super::super::execution_reconciliation::read_request(
            duplicate_file.path().to_str().unwrap()
        )
        .is_err()
    );

    let missing_terminal = request.replace(",\"terminal\":null", "");
    let missing_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(missing_file.path(), missing_terminal).unwrap();
    assert!(
        super::super::execution_reconciliation::read_request(missing_file.path().to_str().unwrap())
            .is_err()
    );
}
