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
