use super::*;

#[tokio::test]
async fn session_runner_receipt_preview_posts_the_bound_request_once() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let request: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-receipt-observation/preview ",
        required_headers: &[],
        body_fields: json!({
            "owner": request["owner"].clone(),
            "conversation_id": "conversation-001",
            "prompt_id": "prompt-001",
            "run_id": "run-001",
        }),
        response_status: "200 OK",
        response: request.clone(),
    }]);
    let returned = client
        .preview_session_runner_receipt_observation("conversation-001", "run-001", &request)
        .await
        .unwrap();
    super::super::session_runner_receipt::validate_response(
        &returned,
        &request,
        "conversation-001",
        "run-001",
    )
    .unwrap();
    server.join().unwrap();
}

#[tokio::test]
async fn session_runner_receipt_preview_rejects_request_conversation_or_run_url_drift_before_post()
{
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let request: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_receipt_observation("conversation-001", "run-foreign", &request)
        .await
        .expect_err("a request bound to another Run must fail before transport");
    assert_eq!(
        error.to_string(),
        "Session Runner receipt observation request does not match its URL"
    );
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_receipt_observation("conversation-foreign", "run-001", &request)
        .await
        .expect_err("a request bound to another Conversation must fail before transport");
    assert_eq!(
        error.to_string(),
        "Session Runner receipt observation request does not match its URL"
    );
}

#[tokio::test]
async fn session_runner_receipt_preview_rejects_malformed_request_before_post() {
    let error = test_remote_client("127.0.0.1:1".parse().unwrap())
        .preview_session_runner_receipt_observation(
            "conversation-001",
            "run-001",
            &json!({"schema_version": "forge.session-runner-receipt-observation/v1"}),
        )
        .await
        .expect_err("a malformed receipt observation must fail before transport");
    assert_eq!(
        error.to_string(),
        "remote session Runner receipt observation has an invalid shape"
    );
}

#[tokio::test]
async fn session_runner_receipt_preview_rejects_a_foreign_response_at_the_client_boundary() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let request: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    let mut response = request.clone();
    response["run_id"] = json!("run-foreign");
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-receipt-observation/preview ",
        required_headers: &[],
        body_fields: json!({
            "conversation_id": "conversation-001",
            "run_id": "run-001",
        }),
        response_status: "200 OK",
        response,
    }]);
    let error = client
        .preview_session_runner_receipt_observation("conversation-001", "run-001", &request)
        .await
        .expect_err("a response bound to another Run must fail closed");
    assert!(
        error.to_string().contains("different path binding"),
        "{error}"
    );
    server.join().unwrap();
}

#[tokio::test]
async fn session_runner_receipt_preview_rejects_response_binding_and_authority_drift_at_client_boundary()
 {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let request: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    let mut foreign_owner = request.clone();
    foreign_owner["owner"]["subject"] = json!("other-user");
    let mut authority = request.clone();
    authority["authority"]["execution_authorized"] = json!(true);
    let (client, server) = spawn_mock_server(vec![
        ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-receipt-observation/preview ",
            required_headers: &[],
            body_fields: json!({
                "conversation_id": "conversation-001",
                "run_id": "run-001",
            }),
            response_status: "200 OK",
            response: foreign_owner,
        },
        ExpectedRequest {
            request_prefix: "POST /api/v1/conversations/conversation-001/runs/run-001/runner-receipt-observation/preview ",
            required_headers: &[],
            body_fields: json!({
                "conversation_id": "conversation-001",
                "run_id": "run-001",
            }),
            response_status: "200 OK",
            response: authority,
        },
    ]);
    let binding_error = client
        .preview_session_runner_receipt_observation("conversation-001", "run-001", &request)
        .await
        .expect_err("a foreign response binding must fail closed");
    assert!(
        binding_error.to_string().contains("different owner"),
        "{binding_error}"
    );
    let authority_error = client
        .preview_session_runner_receipt_observation("conversation-001", "run-001", &request)
        .await
        .expect_err("an authority-bearing response must fail closed");
    assert!(
        authority_error.to_string().contains("invalid"),
        "{authority_error}"
    );
    server.join().unwrap();
}

#[test]
fn session_runner_receipt_preview_rejects_response_binding_and_authority_mutations() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let request: Value = serde_json::from_slice(&std::fs::read(&fixture).unwrap()).unwrap();
    let mut foreign_owner = request.clone();
    foreign_owner["owner"]["subject"] = json!("other-user");
    assert!(
        super::super::session_runner_receipt::validate_response(
            &foreign_owner,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut foreign_path = request.clone();
    foreign_path["run_id"] = json!("run-002");
    assert!(
        super::super::session_runner_receipt::validate_response(
            &foreign_path,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut selected = request.clone();
    selected["selected_target_id"] = json!("runner-1");
    assert!(
        super::super::session_runner_receipt::validate_response(
            &selected,
            &request,
            "conversation-001",
            "run-001",
        )
        .is_err()
    );

    let mut authority = request;
    authority["authority"]["execution_authorized"] = json!(true);
    assert!(
        super::super::session_runner_receipt::validate_response(
            &authority,
            &authority.clone(),
            "conversation-001",
            "run-001",
        )
        .is_err()
    );
}

#[test]
fn session_runner_receipt_input_rejects_root_and_nested_duplicate_keys() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json");
    let source = std::fs::read_to_string(&fixture).unwrap();
    let root_duplicate = source.replacen(
        "  \"schema_version\": \"forge.session-runner-receipt-observation/v1\",\n",
        "  \"schema_version\": \"forge.session-runner-receipt-observation/v1\",\n  \"schema_version\": \"forge.session-runner-receipt-observation/v1\",\n",
        1,
    );
    let root_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(root_path.path(), root_duplicate).unwrap();
    assert!(
        super::super::session_runner_receipt::read_request(root_path.path().to_str().unwrap())
            .is_err()
    );

    let nested_duplicate = source.replacen(
        "      \"device_identity_verified\": false,\n",
        "      \"device_identity_verified\": false,\n      \"device_identity_verified\": false,\n",
        1,
    );
    let nested_path = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(nested_path.path(), nested_duplicate).unwrap();
    assert!(
        super::super::session_runner_receipt::read_request(nested_path.path().to_str().unwrap())
            .is_err()
    );
}
