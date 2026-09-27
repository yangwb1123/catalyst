use serde_json::{Value, json};

use super::*;

fn history_fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
    ))
    .expect("session Runner receipt history fixture")
}

#[tokio::test]
async fn session_runner_receipt_history_preview_posts_the_bound_history_once() {
    let request = history_fixture();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-receipt-history/preview ",
        required_headers: &[],
        body_fields: json!({
            "owner": request["owner"].clone(),
            "conversation_id": "conversation-001",
            "prompt_id": "prompt-001",
            "run_id": "run-001",
            "attempt_count": 2,
            "latest_disposition_kind": "uncertain",
            "automatic_retry": false,
        }),
        response_status: "200 OK",
        response: request.clone(),
    }]);
    let returned = client
        .preview_session_runner_receipt_history("conversation-001", "run-001", &request)
        .await
        .unwrap();
    super::super::session_runner_receipt_history::validate_response(
        &returned,
        &request,
        "conversation-001",
        "run-001",
    )
    .unwrap();
    server.join().unwrap();
}

#[tokio::test]
async fn session_runner_receipt_history_preview_rejects_request_conversation_or_run_url_drift_before_post()
 {
    let request = history_fixture();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_receipt_history("conversation-001", "run-foreign", &request)
        .await
        .expect_err("a history bound to another Run must fail before transport");
    assert_eq!(
        error.to_string(),
        "Session Runner receipt history request does not match its URL"
    );
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_receipt_history("conversation-foreign", "run-001", &request)
        .await
        .expect_err("a history bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Session Runner receipt history request does not match its URL"
    );
}

#[tokio::test]
async fn session_runner_receipt_history_preview_rejects_malformed_request_before_post() {
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_receipt_history(
            "conversation-001",
            "run-001",
            &json!({"schema_version": "forge.session-runner-receipt-history/v1"}),
        )
        .await
        .expect_err("a malformed receipt history must fail before transport");
    assert_eq!(
        error.to_string(),
        "remote session Runner receipt history is invalid"
    );
}

#[tokio::test]
async fn session_runner_receipt_history_preview_rejects_a_foreign_response_at_the_client_boundary()
{
    let request = history_fixture();
    let mut response = request.clone();
    response["conversation_id"] = json!("conversation-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-receipt-history/preview ",
        required_headers: &[],
        body_fields: json!({
            "conversation_id": "conversation-001",
            "run_id": "run-001",
        }),
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .preview_session_runner_receipt_history("conversation-001", "run-001", &request)
        .await
        .expect_err("a response bound to another Conversation must fail closed");
    assert!(error.to_string().contains("invalid"), "{error}");
    server.join().unwrap();
}

#[test]
fn session_runner_receipt_history_response_rejects_binding_summary_and_authority_drift() {
    let request = history_fixture();
    let mut foreign_path = request.clone();
    foreign_path["run_id"] = json!("run-002");
    assert!(
        super::super::session_runner_receipt_history::validate_response(
            &foreign_path,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut summary = request.clone();
    summary["attempt_count"] = json!(3);
    assert!(
        super::super::session_runner_receipt_history::validate_response(
            &summary,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut selected = request.clone();
    selected["selected_target_id"] = json!("runner-2");
    assert!(
        super::super::session_runner_receipt_history::validate_response(
            &selected,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut authority = request;
    authority["authority"]["receipt_persisted"] = json!(true);
    assert!(
        super::super::session_runner_receipt_history::validate_response(
            &authority,
            &authority.clone(),
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
}

#[test]
fn session_runner_receipt_history_input_rejects_unknown_duplicate_and_trailing_fields() {
    let fixture = serde_json::to_string(&history_fixture()).unwrap();
    let unknown = format!("{},\"unexpected\":true}}", fixture.trim_end_matches('}'));
    let unknown_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(unknown_file.path(), unknown).unwrap();
    assert!(
        super::super::session_runner_receipt_history::read_request(
            unknown_file.path().to_str().unwrap()
        )
        .is_err()
    );

    let duplicate = format!(
        "{},\"schema_version\":\"forge.session-runner-receipt-history/v1\"}}",
        fixture.trim_end_matches('}')
    );
    let duplicate_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(duplicate_file.path(), duplicate).unwrap();
    assert!(
        super::super::session_runner_receipt_history::read_request(
            duplicate_file.path().to_str().unwrap()
        )
        .is_err()
    );

    let trailing_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(trailing_file.path(), format!("{fixture} {{}}")).unwrap();
    assert!(
        super::super::session_runner_receipt_history::read_request(
            trailing_file.path().to_str().unwrap()
        )
        .is_err()
    );
}
