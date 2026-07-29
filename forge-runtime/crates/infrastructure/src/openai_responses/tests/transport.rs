use std::fmt::Write as _;

use forge_runtime_domain::{
    Cancellation, Capability, Message, ModelEvent, ModelFinishReason, ModelProvider, ModelRequest,
    ToolCall, ToolSpec, Usage,
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use super::{
    OpenAiResponsesProvider,
    response_fixtures::{
        function_output, function_stream, message_output, refusal_output, text_stream,
    },
    sse::{
        MAX_BUFFER_BYTES, MAX_PENDING_CALLS, MAX_RESPONSE_BYTES, MAX_RESPONSE_FRAMES, SseDecoder,
    },
};

const SECRET: &str = "sk-mock-secret-never-log";

#[tokio::test]
async fn streams_text_usage_and_encodes_stateless_request() {
    let server = MockServer::start().await;
    mount_stream(&server, text_stream()).await;
    let provider = provider(&server);

    let events = collect(&provider, representative_request()).await;

    assert_eq!(
        events,
        vec![
            ModelEvent::TextDelta {
                delta: "hello ".into()
            },
            ModelEvent::TextDelta {
                delta: "world".into()
            },
            ModelEvent::ProviderContext {
                provider: "openai.responses".into(),
                items: vec![message_output("message-1", "hello world")],
            },
            ModelEvent::Usage {
                usage: Usage {
                    input_tokens: 17,
                    output_tokens: 2,
                }
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::Completed
            },
        ]
    );
    assert_request(&server).await;
}

#[tokio::test]
async fn maps_function_done_to_the_provider_call_id() {
    let server = MockServer::start().await;
    mount_stream(&server, function_stream()).await;
    let provider = provider(&server);

    let events = collect(&provider, empty_request()).await;

    assert_eq!(
        events,
        vec![
            ModelEvent::ToolCall {
                call: ToolCall {
                    id: "call-provider-1".into(),
                    name: "read_file".into(),
                    arguments: json!({"path": "README.md"}),
                }
            },
            ModelEvent::ProviderContext {
                provider: "openai.responses".into(),
                items: vec![function_output()],
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::ToolUse
            },
        ]
    );
}

#[tokio::test]
async fn maps_incomplete_responses_to_the_length_finish_reason() {
    let server = MockServer::start().await;
    let stream = concat!(
        "event: response.incomplete\n",
        "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",",
        "\"output\":[],",
        "\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":",
        "{\"input_tokens\":7,\"output_tokens\":1024}}}\n\n",
    );
    mount_stream(&server, stream).await;

    let events = collect(&provider(&server), empty_request()).await;

    assert_eq!(
        events,
        vec![
            ModelEvent::Usage {
                usage: Usage {
                    input_tokens: 7,
                    output_tokens: 1_024,
                }
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::Length
            },
        ]
    );
}

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

    assert_eq!(error.code, "invalid_api_key");
    assert!(!error.retryable);
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(error.message.contains("[REDACTED]"));
}

#[tokio::test]
async fn redacts_the_api_key_from_stream_errors() {
    let server = MockServer::start().await;
    let stream = format!(
        "event: error\ndata: {{\"type\":\"error\",\"code\":\"server_error\",\
         \"message\":\"upstream echoed {SECRET}\"}}\n\n"
    );
    mount_stream(&server, &stream).await;

    let errors = provider(&server)
        .stream(empty_request())
        .collect::<Vec<_>>()
        .await;
    let error = errors[0].as_ref().expect_err("stream failure");

    assert_eq!(error.code, "server_error");
    assert!(!format!("{error:?}").contains(SECRET));
    assert!(error.message.contains("[REDACTED]"));
}

#[test]
fn configuration_is_explicit_and_origin_locked() {
    let official = "https://api.openai.com/v1";
    let empty_model = OpenAiResponsesProvider::new(official, "", SECRET);
    let empty_key = OpenAiResponsesProvider::new(official, "model", "");

    assert!(OpenAiResponsesProvider::new(official, "model", SECRET).is_ok());
    assert_eq!(
        empty_model.err().expect("empty model").code,
        "invalid_provider_config"
    );
    assert_eq!(
        empty_key.err().expect("empty key").code,
        "invalid_provider_config"
    );
    for rejected in [
        "https://user:pass@api.openai.com/v1",
        "http://api.openai.com/v1",
        "https://api.example/v1",
        "https://api.openai.com/v2",
        "https://api.openai.com/v1//",
    ] {
        let error = OpenAiResponsesProvider::new(rejected, "model", SECRET)
            .err()
            .expect("unofficial provider URL");
        assert_eq!(error.code, "invalid_provider_config", "{rejected}");
    }
}

#[test]
fn rejects_combined_input_before_growing_the_sse_buffer() {
    let mut decoder = SseDecoder::new(SECRET);
    let partial_frame = vec![b'x'; MAX_BUFFER_BYTES];
    assert!(
        decoder
            .push(&partial_frame)
            .expect("bounded input")
            .is_empty()
    );

    let error = decoder.push(b"x").expect_err("combined oversized input");

    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("size limit"));
}

#[tokio::test]
async fn does_not_follow_redirects() {
    let server = MockServer::start().await;
    let redirect = format!("{}/redirect-target", server.uri());
    mount_response(
        &server,
        ResponseTemplate::new(307).insert_header("location", redirect.as_str()),
    )
    .await;

    let items = provider(&server)
        .stream(empty_request())
        .collect::<Vec<_>>()
        .await;
    let error = items[0].as_ref().expect_err("redirect response");
    let requests = server.received_requests().await.expect("recorded request");

    assert_eq!(error.code, "http_307");
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.path(), "/v1/responses");
}

#[tokio::test]
async fn rejects_success_with_the_wrong_content_type() {
    let server = MockServer::start().await;
    let response =
        ResponseTemplate::new(200).set_body_raw(text_stream().as_bytes(), "application/json");
    mount_response(&server, response).await;

    let items = provider(&server)
        .stream(empty_request())
        .collect::<Vec<_>>()
        .await;
    let error = items[0].as_ref().expect_err("wrong content type");

    assert_eq!(error.code, "provider_protocol");
    assert!(!error.retryable);
}

#[test]
fn response_failed_is_a_definitive_non_retryable_error() {
    let mut decoder = SseDecoder::new(SECRET);
    let frame = concat!(
        "event: response.failed\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"error\":",
        "{\"code\":\"server_error\",\"message\":\"definitive failure\"}}}\n\n",
    );

    let error = decoder.push(frame.as_bytes()).expect_err("failed response");

    assert_eq!(error.code, "server_error");
    assert!(!error.retryable);
    assert!(decoder.is_terminal());
}

#[test]
fn refusal_deltas_are_emitted_as_text() {
    let mut decoder = SseDecoder::new(SECRET);
    let frames = concat!(
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",",
        "\"id\":\"refusal-1\",\"role\":\"assistant\"}}\n\n",
        "event: response.refusal.delta\n",
        "data: {\"type\":\"response.refusal.delta\",\"item_id\":\"refusal-1\",",
        "\"delta\":\"cannot comply\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",",
        "\"output\":[",
        "{\"type\":\"message\",\"id\":\"refusal-1\",\"status\":\"completed\",",
        "\"role\":\"assistant\",\"content\":[{\"type\":\"refusal\",",
        "\"refusal\":\"cannot comply\"}]}],\"usage\":null}}\n\n",
    );

    let events = decoder.push(frames.as_bytes()).expect("valid refusal");

    assert_eq!(
        events,
        vec![
            ModelEvent::TextDelta {
                delta: "cannot comply".into()
            },
            ModelEvent::ProviderContext {
                provider: "openai.responses".into(),
                items: vec![refusal_output()],
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::Completed
            },
        ]
    );
}

#[tokio::test]
async fn eof_after_a_decoded_event_is_not_reported_as_retryable() {
    let server = MockServer::start().await;
    let frame = concat!(
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",",
        "\"id\":\"partial-1\",\"role\":\"assistant\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"partial-1\",",
        "\"delta\":\"partial\"}\n\n",
    );
    mount_stream(&server, frame).await;

    let items = provider(&server)
        .stream(empty_request())
        .collect::<Vec<_>>()
        .await;

    assert!(matches!(
        &items[0],
        Ok(ModelEvent::TextDelta { delta }) if delta == "partial"
    ));
    let error = items[1].as_ref().expect_err("unterminated stream");
    assert_eq!(error.code, "stream_ended");
    assert!(!error.retryable);
}

#[test]
fn ignored_and_unknown_frames_still_consume_the_frame_budget() {
    let mut decoder = SseDecoder::new(SECRET);
    let pair = concat!(
        ": keepalive\n\n",
        "event: response.created\n",
        "data: {\"type\":\"response.created\"}\n\n",
    );
    let batch = pair.repeat(MAX_RESPONSE_FRAMES / 2);
    assert!(
        decoder
            .push(batch.as_bytes())
            .expect("frame budget")
            .is_empty()
    );

    let error = decoder
        .push(b": one-too-many\n\n")
        .expect_err("exhausted frame budget");

    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("frame limit"));
}

#[test]
fn ignored_frames_still_consume_the_total_byte_budget() {
    let mut decoder = SseDecoder::new(SECRET);
    let mut frame = vec![b'x'; 256 * 1024];
    frame[0] = b':';
    frame.extend_from_slice(b"\n\n");
    for _ in 0..MAX_RESPONSE_BYTES / frame.len() {
        assert!(decoder.push(&frame).expect("byte budget").is_empty());
    }

    let error = decoder
        .push(&frame)
        .expect_err("exhausted response byte budget");

    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("total byte limit"));
}

#[test]
fn pending_function_calls_are_bounded() {
    let mut decoder = SseDecoder::new(SECRET);
    let mut frames = String::new();
    for index in 0..MAX_PENDING_CALLS {
        write!(
            frames,
            "data: {{\"type\":\"response.output_item.added\",\"item\":{{\"id\":\"item-{index}\",\
             \"type\":\"function_call\",\"call_id\":\"call-{index}\",\"name\":\"tool\"}}}}\n\n"
        )
        .expect("write test frame");
    }
    assert!(
        decoder
            .push(frames.as_bytes())
            .expect("call budget")
            .is_empty()
    );
    let extra = b"data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"extra\",\
                  \"type\":\"function_call\",\"call_id\":\"extra\",\"name\":\"tool\"}}\n\n";

    let error = decoder.push(extra).expect_err("exhausted call budget");

    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("pending function call limit"));
}

async fn mount_stream(server: &MockServer, body: &str) {
    let response = ResponseTemplate::new(200).set_body_raw(body.as_bytes(), "text/event-stream");
    mount_response(server, response).await;
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

async fn collect(provider: &OpenAiResponsesProvider, request: ModelRequest) -> Vec<ModelEvent> {
    provider
        .stream(request)
        .map(|event| event.expect("valid stream event"))
        .collect()
        .await
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

fn representative_request() -> ModelRequest {
    ModelRequest {
        system_prompt: "stay concise".into(),
        messages: vec![
            Message::User {
                text: "read it".into(),
            },
            Message::Assistant {
                text: "checking".into(),
                tool_calls: vec![ToolCall {
                    id: "prior-call".into(),
                    name: "read_file".into(),
                    arguments: json!({"path": "old.md"}),
                }],
            },
            Message::Tool {
                call_id: "prior-call".into(),
                name: "read_file".into(),
                output: "old contents".into(),
                is_error: false,
                truncated: false,
            },
        ],
        tools: vec![ToolSpec {
            name: "read_file".into(),
            description: "Read a workspace file".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"]
            }),
            capability: Capability::WorkspaceRead,
        }],
        max_output_tokens: 1_024,
        cancellation: Cancellation::default(),
    }
}

async fn assert_request(server: &MockServer) {
    let requests = server.received_requests().await.expect("request recording");
    let body: Value = serde_json::from_slice(&requests[0].body).expect("request JSON");

    assert_eq!(body["model"], "explicit-test-model");
    assert_eq!(body["instructions"], "stay concise");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["max_output_tokens"], 1_024);
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["name"], "read_file");
    assert_eq!(body["input"][0]["role"], "user");
    assert_eq!(body["input"][1]["role"], "assistant");
    assert_eq!(body["input"][2]["type"], "function_call");
    assert_eq!(body["input"][2]["call_id"], "prior-call");
    assert_eq!(body["input"][3]["type"], "function_call_output");
}
