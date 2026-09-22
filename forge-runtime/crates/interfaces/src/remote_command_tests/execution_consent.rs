use serde_json::{Value, json};

use super::{ExpectedRequest, spawn_mock_server};

fn response() -> Value {
    json!({
        "conversation_id": "conversation-1",
        "project_id": "project-1",
        "profile_id": "profile-reviewed-v1",
        "profile_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "maximum_ttl_ms": 2_592_000_000_u64
    })
}

#[tokio::test]
async fn execution_consent_preview_reads_exact_get_and_validates_binding() {
    let response = response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/execution-consents ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: response.clone(),
    }]);
    let returned = client
        .preview_execution_consent("conversation-1")
        .await
        .expect("execution-consent preview");
    super::super::execution_consent_preview::validate_response(&returned, "conversation-1")
        .expect("strict response");
    assert_eq!(returned, response);
    server.join().unwrap();
}

#[tokio::test]
async fn execution_consent_preview_rejects_foreign_or_unknown_response_fields() {
    let response = response();
    let (client, server) = spawn_mock_server(vec![ExpectedRequest {
        request_prefix: "GET /api/v1/conversations/conversation-1/execution-consents ",
        required_headers: &[],
        body_fields: Value::Null,
        response_status: "200 OK",
        response: {
            let mut value = response.clone();
            value["run_id"] = json!("foreign");
            value
        },
    }]);
    let error = client
        .preview_execution_consent("conversation-1")
        .await
        .expect_err("unknown response field must fail closed");
    assert!(error.to_string().contains("execution-consent preview"));
    server.join().unwrap();
}
