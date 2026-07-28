use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_domain::{
    Cancellation, Message, ModelEvent, ModelFinishReason, ModelProvider, ModelRequest, ToolCall,
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{header, method, path},
};

use super::OpenAiResponsesProvider;

const SECRET: &str = "sk-two-turn-test";

#[tokio::test]
async fn encrypted_reasoning_and_tool_results_survive_a_two_request_loop() {
    let server = MockServer::start().await;
    mount_two_turn_responder(&server).await;
    let provider = test_provider(&server);

    let first_events = collect(&provider, initial_request()).await;
    let (context, call) = continuation_parts(&first_events);
    let final_events = collect(&provider, followup_request(context, call)).await;

    assert_eq!(
        final_events,
        vec![
            ModelEvent::TextDelta {
                delta: "final answer".into()
            },
            ModelEvent::ProviderContext {
                provider: "openai.responses".into(),
                items: vec![final_message_item()],
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::Completed
            },
        ]
    );
    assert_followup_body(&server).await;
}

async fn mount_two_turn_responder(server: &MockServer) {
    let request_number = Arc::new(AtomicUsize::new(0));
    let responder_number = Arc::clone(&request_number);
    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(header("authorization", format!("Bearer {SECRET}")))
        .respond_with(move |_: &Request| {
            let body = if responder_number.fetch_add(1, Ordering::SeqCst) == 0 {
                first_stream()
            } else {
                final_stream()
            };
            ResponseTemplate::new(200).set_body_raw(body.as_bytes(), "text/event-stream")
        })
        .expect(2)
        .mount(server)
        .await;
}

fn continuation_parts(events: &[ModelEvent]) -> (Message, ToolCall) {
    let context = events
        .iter()
        .find_map(|event| match event {
            ModelEvent::ProviderContext { provider, items } => Some(Message::ProviderContext {
                provider: provider.clone(),
                items: items.clone(),
            }),
            _ => None,
        })
        .expect("reasoning provider context");
    let call = events
        .iter()
        .find_map(|event| match event {
            ModelEvent::ToolCall { call } => Some(call.clone()),
            _ => None,
        })
        .expect("function call");
    (context, call)
}

async fn assert_followup_body(server: &MockServer) {
    let requests = server.received_requests().await.expect("recorded requests");
    assert_eq!(requests.len(), 2);
    let body: Value = serde_json::from_slice(&requests[1].body).expect("second request JSON");
    let input = body["input"].as_array().expect("input array");

    assert_eq!(input, &expected_followup_input());
    assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
}

fn expected_followup_input() -> Vec<Value> {
    vec![
        json!({"type": "message", "role": "user", "content": "first question"}),
        json!({
            "type": "reasoning",
            "id": "rs-1",
            "summary": [],
            "content": [{"type": "reasoning_text", "text": "opaque"}],
            "encrypted_content": "encrypted-state",
            "status": "completed"
        }),
        json!({
            "type": "message",
            "id": "msg-1",
            "status": "completed",
            "role": "assistant",
            "phase": "commentary",
            "content": [{"type": "output_text", "text": "checking", "annotations": []}]
        }),
        json!({
            "type": "function_call",
            "id": "fc-1",
            "call_id": "call-1",
            "name": "read_file",
            "arguments": "{\"path\":\"README.md\"}",
            "status": "completed",
            "caller": {"type": "direct"}
        }),
        json!({
            "type": "function_call_output",
            "call_id": "call-1",
            "output": "tool result"
        }),
    ]
}

fn initial_request() -> ModelRequest {
    request(vec![Message::User {
        text: "first question".into(),
    }])
}

fn followup_request(context: Message, call: ToolCall) -> ModelRequest {
    let call_id = call.id.clone();
    let name = call.name.clone();
    request(vec![
        Message::User {
            text: "first question".into(),
        },
        context,
        Message::Assistant {
            text: String::new(),
            tool_calls: vec![call],
        },
        Message::Tool {
            call_id,
            name,
            output: "tool result".into(),
            is_error: false,
            truncated: false,
        },
    ])
}

fn request(messages: Vec<Message>) -> ModelRequest {
    ModelRequest {
        system_prompt: "system".into(),
        messages,
        tools: Vec::new(),
        max_output_tokens: 128,
        cancellation: Cancellation::default(),
    }
}

fn test_provider(server: &MockServer) -> OpenAiResponsesProvider {
    OpenAiResponsesProvider::new_insecure_for_test(
        format!("{}/v1", server.uri()),
        "test-model",
        SECRET,
    )
    .expect("loopback provider")
}

async fn collect(provider: &OpenAiResponsesProvider, request: ModelRequest) -> Vec<ModelEvent> {
    provider
        .stream(request)
        .map(|event| event.expect("valid provider event"))
        .collect()
        .await
}

fn first_stream() -> &'static str {
    concat!(
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"msg-1\",",
        "\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"commentary\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg-1\",",
        "\"delta\":\"checking\"}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"fc-1\",",
        "\"type\":\"function_call\",\"call_id\":\"call-1\",\"name\":\"read_file\"}}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",\"item_id\":\"fc-1\",",
        "\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",",
        "\"output\":[",
        "{\"type\":\"reasoning\",\"id\":\"rs-1\",\"summary\":[],",
        "\"content\":[{\"type\":\"reasoning_text\",\"text\":\"opaque\"}],",
        "\"encrypted_content\":\"encrypted-state\",\"status\":\"completed\"},",
        "{\"type\":\"message\",\"id\":\"msg-1\",\"status\":\"completed\",",
        "\"role\":\"assistant\",\"phase\":\"commentary\",\"content\":[",
        "{\"type\":\"output_text\",\"text\":\"checking\",\"annotations\":[]}]},",
        "{\"type\":\"function_call\",\"id\":\"fc-1\",\"call_id\":\"call-1\",",
        "\"name\":\"read_file\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\",",
        "\"status\":\"completed\",\"caller\":{\"type\":\"direct\"}}],",
        "\"usage\":null}}\n\n",
    )
}

fn final_stream() -> &'static str {
    concat!(
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"id\":\"msg-final\",",
        "\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"final_answer\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"msg-final\",",
        "\"delta\":\"final answer\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",",
        "\"output\":[",
        "{\"type\":\"message\",\"id\":\"msg-final\",\"status\":\"completed\",",
        "\"role\":\"assistant\",\"phase\":\"final_answer\",\"content\":[",
        "{\"type\":\"output_text\",\"text\":\"final answer\",\"annotations\":[]}]}],",
        "\"usage\":null}}\n\n",
    )
}

fn final_message_item() -> Value {
    json!({
        "type": "message",
        "id": "msg-final",
        "status": "completed",
        "role": "assistant",
        "phase": "final_answer",
        "content": [{
            "type": "output_text",
            "text": "final answer",
            "annotations": []
        }]
    })
}
