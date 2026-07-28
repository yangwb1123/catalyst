use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_domain::{
    Cancellation, Message, ModelEvent, ModelEventStream, ModelFinishReason, ModelProvider,
    ModelRequest, ProviderError, RuntimeEventKind,
};
use forge_runtime_infrastructure::{CapStdWorkspaceFactory, MemoryEventSink};
use futures_util::stream;
use serde_json::json;
use tempfile::TempDir;

mod support;

use support::{ProbeTool, completed, request, tool_call};

#[tokio::test]
async fn provider_context_is_committed_and_replayed_before_tool_results() {
    let context = reasoning_context();
    let first_turn = first_turn(&context);
    let provider = Arc::new(RecordingProvider::new(vec![
        first_turn,
        completed("final answer"),
    ]));
    let root = TempDir::new().expect("workspace");
    let runtime = runtime(provider.clone());
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect("two-turn run succeeds");

    assert_second_request(&provider.requests());
    assert!(result.messages.contains(&context));
    assert_committed_order(&sink);
}

fn reasoning_context() -> Message {
    Message::ProviderContext {
        provider: "openai.responses".into(),
        items: vec![json!({
            "type": "reasoning",
            "id": "reasoning-1",
            "encrypted_content": "opaque-fixture",
            "summary": []
        })],
    }
}

fn first_turn(context: &Message) -> Vec<Result<ModelEvent, ProviderError>> {
    let context = Message::ProviderContext {
        provider: "openai.responses".into(),
        items: match context {
            Message::ProviderContext { items, .. } => items.clone(),
            _ => unreachable!(),
        },
    };
    vec![
        Ok(ModelEvent::ProviderContext {
            provider: "openai.responses".into(),
            items: match &context {
                Message::ProviderContext { items, .. } => items.clone(),
                _ => unreachable!(),
            },
        }),
        Ok(ModelEvent::ToolCall {
            call: tool_call("call-1", "read_file", json!({"path": "README.md"})),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::ToolUse,
        }),
    ]
}

fn assert_second_request(requests: &[Vec<Message>]) {
    assert_eq!(requests.len(), 2);
    assert!(matches!(
        requests[1].get(1),
        Some(Message::ProviderContext { provider, .. })
            if provider == "openai.responses"
    ));
    assert!(matches!(
        requests[1].get(2),
        Some(Message::Assistant { tool_calls, .. }) if tool_calls.len() == 1
    ));
    assert!(matches!(requests[1].get(3), Some(Message::Tool { .. })));
}

fn assert_committed_order(sink: &MemoryEventSink) {
    let committed: Vec<_> = sink
        .events()
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::MessageCommitted { message } => Some(message),
            _ => None,
        })
        .collect();
    assert!(matches!(committed[1], Message::ProviderContext { .. }));
    assert!(matches!(committed[2], Message::Assistant { .. }));
}

fn runtime(provider: Arc<RecordingProvider>) -> AgentRuntime {
    let mut tools = ToolCatalog::default();
    tools
        .register(ProbeTool::succeeds("read_file", "fixture contents"))
        .expect("tool registration");
    AgentRuntime::new(provider, tools, Arc::new(CapStdWorkspaceFactory))
}

struct RecordingProvider {
    turns: Mutex<VecDeque<Vec<Result<ModelEvent, ProviderError>>>>,
    requests: Mutex<Vec<Vec<Message>>>,
}

impl RecordingProvider {
    fn new(turns: Vec<Vec<Result<ModelEvent, ProviderError>>>) -> Self {
        Self {
            turns: Mutex::new(turns.into()),
            requests: Mutex::default(),
        }
    }

    fn requests(&self) -> Vec<Vec<Message>> {
        self.requests.lock().expect("request lock").clone()
    }
}

impl ModelProvider for RecordingProvider {
    fn stream(&self, request: ModelRequest) -> ModelEventStream {
        self.requests
            .lock()
            .expect("request lock")
            .push(request.messages);
        let events = self
            .turns
            .lock()
            .expect("turn lock")
            .pop_front()
            .expect("scripted turn");
        Box::pin(stream::iter(events))
    }
}
