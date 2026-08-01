use forge_runtime_domain::{
    Cancellation, GroupAgentNodeExecutionContract, Message, ModelRequest, PreparedModelProvider,
    PreparedModelRequest, ProviderError, group_agent_node_provider_request_sha256,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use super::{
    OpenAiResponsesProvider,
    request::{encode_request_bytes, validate_request_bytes},
    response_fixtures::text_stream,
};

const GOLDEN: &[u8] = br#"{"include":["reasoning.encrypted_content"],"input":[{"content":"go","role":"user","type":"message"}],"instructions":"system","max_output_tokens":1024,"model":"test-model","store":false,"stream":true,"tools":[]}"#;
const SECRET: &str = "sk-prepared-request-test";

#[derive(Deserialize)]
struct SharedContractFixture {
    expected: SharedContractExpected,
}

#[derive(Deserialize)]
struct SharedContractExpected {
    canonical_contract_json: String,
    canonical_provider_request_body_json: String,
    provider_request_bytes: usize,
    provider_request_sha256: String,
}

#[test]
fn shared_go_contract_encodes_to_the_exact_rust_provider_body_golden() {
    let fixture: SharedContractFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("shared Go contract fixture");
    let contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&fixture.expected.canonical_contract_json).expect("contract JSON");
    let request = ModelRequest {
        system_prompt: contract.request.system_prompt,
        messages: vec![Message::User {
            text: contract.request.user_prompt,
        }],
        tools: Vec::new(),
        max_output_tokens: contract.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    };

    let body = OpenAiResponsesProvider::encode_request_bytes(&contract.provider.model, &request)
        .expect("production Responses codec");

    assert_eq!(
        body,
        fixture
            .expected
            .canonical_provider_request_body_json
            .as_bytes()
    );
    assert_eq!(body.len(), fixture.expected.provider_request_bytes);
    assert_ne!(body.last(), Some(&b'\n'));
    assert_eq!(
        group_agent_node_provider_request_sha256(&body),
        fixture.expected.provider_request_sha256
    );
}

#[test]
fn request_encoding_is_deterministic_canonical_json() {
    let first =
        OpenAiResponsesProvider::encode_request_bytes("test-model", &request()).expect("encode");
    let second = OpenAiResponsesProvider::encode_request_bytes("test-model", &request())
        .expect("encode again");

    assert_eq!(first, GOLDEN);
    assert_eq!(second, first);
    OpenAiResponsesProvider::validate_exact_request_bytes("test-model", &request(), &first)
        .expect("exact prepared request validates");
    validate_request_bytes("test-model", &first).expect("prepared request validates");
}

#[test]
fn exact_validation_reencodes_the_expected_request_and_rejects_any_difference() {
    let expected = request();
    let canonical = OpenAiResponsesProvider::encode_request_bytes("test-model", &expected)
        .expect("encode expected request");
    let mut different_request = request();
    different_request.max_output_tokens += 1;
    let mut different_body = value(&canonical);
    different_body["instructions"] = Value::String("different system prompt".into());
    let different_body = bytes(&different_body);
    validate_request_bytes("test-model", &different_body)
        .expect("changed instructions remain structurally valid");

    assert_protocol_error(OpenAiResponsesProvider::validate_exact_request_bytes(
        "test-model",
        &different_request,
        &canonical,
    ));
    assert_protocol_error(OpenAiResponsesProvider::validate_exact_request_bytes(
        "test-model",
        &expected,
        &different_body,
    ));
    assert_protocol_error(OpenAiResponsesProvider::validate_exact_request_bytes(
        "other-model",
        &expected,
        &canonical,
    ));
}

#[test]
fn provider_prepare_delegates_to_the_pure_encoding_api() {
    let request = request();
    let exact = OpenAiResponsesProvider::encode_request_bytes("test-model", &request)
        .expect("pure request encoding");
    let provider = OpenAiResponsesProvider::new("https://api.openai.com/v1", "test-model", SECRET)
        .expect("provider configuration");

    let prepared = provider
        .prepare_request(request)
        .expect("provider preparation");

    assert_eq!(prepared.body(), exact);
}

#[test]
fn provider_rejects_an_api_key_that_cannot_form_an_authorization_header() {
    let result = OpenAiResponsesProvider::new(
        "https://api.openai.com/v1",
        "test-model",
        "secret\ninjected",
    );
    let Err(error) = result else {
        panic!("control characters must fail before dispatch");
    };

    assert_eq!(error.code, "invalid_provider_config");
    assert!(!error.retryable);
    assert!(!error.message.contains("secret"));
}

#[test]
fn restored_request_rejects_noncanonical_or_tampered_controls() {
    let canonical = encode_request_bytes("test-model", &request()).expect("encode request");
    let mut noncanonical = canonical.clone();
    noncanonical.push(b'\n');
    assert_protocol_error(validate_request_bytes("test-model", &noncanonical));

    let mut store = value(&canonical);
    store["store"] = Value::Bool(true);
    assert_protocol_error(validate_request_bytes("test-model", &bytes(&store)));

    let mut stream = value(&canonical);
    stream["stream"] = Value::Bool(false);
    assert_protocol_error(validate_request_bytes("test-model", &bytes(&stream)));
}

#[test]
fn restored_request_rejects_a_configured_model_mismatch() {
    let body = encode_request_bytes("persisted-model", &request()).expect("encode request");

    let error = validate_request_bytes("configured-model", &body)
        .expect_err("model mismatch must fail closed");

    assert_eq!(error.code, "provider_protocol");
    assert!(!error.retryable);
    assert!(error.message.contains("configured model"));
    assert!(!error.message.contains("persisted-model"));
}

#[tokio::test]
async fn prepared_request_dispatches_the_exact_bytes_with_json_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("content-type", "application/json"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(text_stream().as_bytes(), "text/event-stream"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let provider = loopback_provider(&server);
    let prepared = provider
        .prepare_request(request())
        .expect("prepare request");
    let exact_body = prepared.body().to_vec();

    let events = provider.stream_prepared(prepared).collect::<Vec<_>>().await;

    assert!(events.iter().all(Result::is_ok));
    let requests = server.received_requests().await.expect("record request");
    assert_eq!(requests[0].body, exact_body);
    assert_eq!(
        requests[0].headers.get_all("content-type").iter().count(),
        1
    );
}

#[tokio::test]
async fn prepared_dispatch_rejects_tampering_and_model_mismatch_before_http() {
    let server = MockServer::start().await;
    let provider = loopback_provider(&server);
    let canonical = encode_request_bytes("test-model", &request()).expect("encode request");
    let mut tampered = value(&canonical);
    tampered["store"] = Value::Bool(true);

    let tampered_error = prepared_error(&provider, bytes(&tampered)).await;
    let mismatch = encode_request_bytes("other-model", &request()).expect("encode mismatch");
    let mismatch_error = prepared_error(&provider, mismatch).await;

    assert!(tampered_error.message.contains("store"));
    assert!(mismatch_error.message.contains("configured model"));
    assert!(
        server
            .received_requests()
            .await
            .expect("request recording")
            .is_empty()
    );
}

fn request() -> ModelRequest {
    ModelRequest {
        system_prompt: "system".into(),
        messages: vec![Message::User { text: "go".into() }],
        tools: Vec::new(),
        max_output_tokens: 1_024,
        cancellation: Cancellation::default(),
    }
}

fn value(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("canonical fixture JSON")
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).expect("canonical JSON bytes")
}

fn assert_protocol_error(result: Result<(), ProviderError>) {
    let error = result.expect_err("tampered request must fail closed");
    assert_eq!(error.code, "provider_protocol");
    assert!(!error.retryable);
}

fn loopback_provider(server: &MockServer) -> OpenAiResponsesProvider {
    OpenAiResponsesProvider::new_insecure_for_test(
        format!("{}/v1", server.uri()),
        "test-model",
        SECRET,
    )
    .expect("loopback provider")
}

async fn prepared_error(provider: &OpenAiResponsesProvider, body: Vec<u8>) -> ProviderError {
    let request = PreparedModelRequest::new(body, Cancellation::default());
    let events = provider.stream_prepared(request).collect::<Vec<_>>().await;
    assert_eq!(events.len(), 1);
    let error = events
        .first()
        .expect("one prepared error")
        .as_ref()
        .expect_err("prepared body must fail closed");
    assert_eq!(error.code, "provider_protocol");
    assert!(!error.retryable);
    error.clone()
}
