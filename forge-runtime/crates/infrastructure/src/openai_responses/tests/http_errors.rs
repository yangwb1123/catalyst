use forge_runtime_domain::{Cancellation, Message, ModelProvider, ModelRequest};
use futures_util::StreamExt;
use serde_json::json;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use super::OpenAiResponsesProvider;

const SECRET: &str = "sk-mock-secret-never-log";

#[tokio::test]
async fn redacts_the_api_key_from_http_errors() {
    let server = MockServer::start().await;
    let body = json!({
        "error": {
            "code": "invalid_api_key",
            "message": format!("credential {SECRET} was rejected")
        }
    });
    mount_response(&server, ResponseTemplate::new(401).set_body_json(body)).await;

    let errors = provider(&server)
        .stream(empty_request())
        .collect::<Vec<_>>()
        .await;
    let error = errors[0].as_ref().expect_err("HTTP failure");

    assert_eq!(error.code, "http_401");
    assert!(!error.retryable);
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(error.message.contains("[REDACTED]"));
}

#[tokio::test]
async fn http_status_controls_error_code_even_when_vendor_code_disagrees() {
    let cases = [
        (429, "invalid_api_key", "http_429", true),
        (500, "bad_request", "http_500", true),
        (400, "http_500", "http_400", false),
    ];
    for (status, vendor_code, expected_code, retryable) in cases {
        let server = MockServer::start().await;
        let body = json!({
            "error": {
                "code": vendor_code,
                "message": "bounded provider failure"
            }
        });
        mount_response(&server, ResponseTemplate::new(status).set_body_json(body)).await;

        let errors = provider(&server)
            .stream(empty_request())
            .collect::<Vec<_>>()
            .await;
        let error = errors[0].as_ref().expect_err("HTTP failure");

        assert_eq!(error.code, expected_code, "HTTP {status}");
        assert_eq!(error.retryable, retryable, "HTTP {status}");
    }
}

async fn mount_response(server: &MockServer, response: ResponseTemplate) {
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {SECRET}")))
        .and(header("content-type", "application/json"))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}

fn provider(server: &MockServer) -> OpenAiResponsesProvider {
    OpenAiResponsesProvider::new_insecure_for_test(
        format!("{}/v1", server.uri()),
        "explicit-test-model",
        SECRET,
    )
    .expect("valid provider")
}

fn empty_request() -> ModelRequest {
    ModelRequest {
        system_prompt: "system".into(),
        messages: vec![Message::User { text: "go".into() }],
        tools: Vec::new(),
        max_output_tokens: 1_024,
        cancellation: Cancellation::default(),
    }
}
