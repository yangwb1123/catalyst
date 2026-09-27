use serde_json::{Value, json};

use crate::args::RemoteCommand;

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-session-runner-reconciliation-projection-v1.json"
);

fn history_fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
    ))
    .expect("session Runner receipt history fixture")
}

fn projection_fixture() -> Value {
    serde_json::from_str(FIXTURE).expect("session Runner reconciliation projection fixture")
}

#[tokio::test]
async fn session_runner_reconciliation_remote_preview_canonicalizes_history_before_projection() {
    let request = history_fixture();
    let response = projection_fixture();
    let (client, server) = super::spawn_mock_server(vec![
        super::ExpectedRequest {
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
        },
        super::ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-reconciliation/preview ",
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
            response: response.clone(),
        },
    ]);
    let (canonical, returned) = client
        .preview_session_runner_reconciliation_from_history("conversation-001", "run-001", &request)
        .await
        .unwrap();
    super::super::session_runner_receipt_history::validate_response(
        &canonical,
        &request,
        "conversation-001",
        "run-001",
    )
    .unwrap();
    super::super::session_runner_reconciliation::validate_remote_response(
        &returned,
        &request,
        "conversation-001",
        "run-001",
    )
    .unwrap();
    assert_eq!(returned, response);
    server.join().unwrap();
}

#[tokio::test]
async fn session_runner_reconciliation_preview_rejects_request_conversation_or_run_url_drift_before_post()
 {
    let request = history_fixture();
    let error = super::test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_reconciliation("conversation-001", "run-foreign", &request)
        .await
        .expect_err("a history bound to another Run must fail before transport");
    assert_eq!(
        error.to_string(),
        "Session Runner reconciliation request does not match its URL"
    );
    let error = super::test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_reconciliation("conversation-foreign", "run-001", &request)
        .await
        .expect_err("a history bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Session Runner reconciliation request does not match its URL"
    );
}

#[tokio::test]
async fn session_runner_reconciliation_preview_rejects_malformed_request_before_post() {
    let error = super::test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_reconciliation(
            "conversation-001",
            "run-001",
            &json!({"schema_version": "forge.session-runner-receipt-history/v1"}),
        )
        .await
        .expect_err("a malformed history must fail before transport");
    assert_eq!(
        error.to_string(),
        "remote session Runner receipt history is invalid"
    );
}

#[tokio::test]
async fn session_runner_reconciliation_preview_rejects_a_foreign_response_at_the_client_boundary() {
    let request = history_fixture();
    let mut response = projection_fixture();
    response["run_id"] = json!("run-foreign");
    let (client, server) = super::spawn_mock_server(vec![super::ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-reconciliation/preview ",
        required_headers: &[],
        body_fields: json!({
            "conversation_id": "conversation-001",
            "run_id": "run-001",
        }),
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .preview_session_runner_reconciliation("conversation-001", "run-001", &request)
        .await
        .expect_err("a response bound to another Run must fail closed");
    assert!(error.to_string().contains("invalid"), "{error}");
    server.join().unwrap();
}

#[test]
fn session_runner_reconciliation_response_rejects_history_and_authority_drift() {
    let request = history_fixture();
    let response = projection_fixture();
    let mut foreign = response.clone();
    foreign["conversation_id"] = json!("conversation-other");
    assert!(
        super::super::session_runner_reconciliation::validate_remote_response(
            &foreign,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut selected = response.clone();
    selected["selected_target_id"] = json!("runner-001");
    assert!(
        super::super::session_runner_reconciliation::validate_remote_response(
            &selected,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut authority = response;
    authority["authority"]["execution_authorized"] = json!(true);
    assert!(
        super::super::session_runner_reconciliation::validate_remote_response(
            &authority,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
}

#[tokio::test]
async fn remote_session_runner_reconciliation_preview_is_local_and_canonical() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), FIXTURE).unwrap();
    let result = super::super::dispatch::execute(
        &RemoteCommand::SessionRunnerReconciliationPreview {
            input: file.path().display().to_string(),
        },
        None,
    )
    .await
    .unwrap();
    let expected: Value = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(result, expected);
}

#[test]
fn remote_session_runner_reconciliation_rejects_wire_and_projection_drift() {
    let duplicate = FIXTURE.replacen(
        "  \"schema_version\": \"forge.session-runner-reconciliation-projection/v1\",\n",
        "  \"schema_version\": \"forge.session-runner-reconciliation-projection/v1\",\n  \"schema_version\": \"forge.session-runner-reconciliation-projection/v1\",\n",
        1,
    );
    assert!(read(duplicate).is_err());

    let unknown = FIXTURE.replacen(
        "  \"evaluation_mode\": \"pure_session_runner_reconciliation_projection_only\",\n",
        "  \"evaluation_mode\": \"pure_session_runner_reconciliation_projection_only\",\n  \"unknown\": true,\n",
        1,
    );
    assert!(read(unknown).is_err());
    assert!(read(format!("{FIXTURE} {{}}")).is_err());

    for (path, value) in [
        (("source", "latest_attempt_id"), json!("attempt-other")),
        (("source", "latest_disposition_kind"), json!("failed")),
        (("authority", "audit_published"), json!(true)),
    ] {
        let mut fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        fixture[path.0][path.1] = value;
        assert!(read(serde_json::to_vec(&fixture).unwrap()).is_err());
    }

    let mut latest: Value = serde_json::from_str(FIXTURE).unwrap();
    latest["latest_observed_at_ms"] = json!(201);
    assert!(read(serde_json::to_vec(&latest).unwrap()).is_err());
}

fn read(bytes: impl AsRef<[u8]>) -> Result<(), super::super::RemoteError> {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), bytes).unwrap();
    super::super::session_runner_reconciliation::read_projection(file.path().to_str().unwrap())
        .map(|_| ())
}
