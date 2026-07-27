use std::path::Path;

use forge_runtime_domain::{
    Message, ModelEvent, ModelEventStream, ModelFinishReason, ModelProvider, ModelRequest,
    ToolCall, Usage,
};
use futures_util::stream;
use serde_json::json;

#[derive(Clone, Debug)]
pub struct ReadThenAnswerProvider {
    relative_path: String,
}

impl ReadThenAnswerProvider {
    #[must_use]
    pub fn new(relative_path: impl Into<String>) -> Self {
        Self {
            relative_path: relative_path.into(),
        }
    }

    fn events_for(&self, request: &ModelRequest) -> Vec<ModelEvent> {
        request
            .messages
            .iter()
            .rev()
            .find_map(tool_message)
            .map_or_else(|| self.read_turn(), |result| self.answer_turn(&result))
    }

    fn read_turn(&self) -> Vec<ModelEvent> {
        vec![
            ModelEvent::TextDelta {
                delta: "Inspecting ".into(),
            },
            ModelEvent::TextDelta {
                delta: format!("{}.", self.relative_path),
            },
            ModelEvent::ToolCall {
                call: ToolCall {
                    id: "fixture-call-1".into(),
                    name: "read_file".into(),
                    arguments: json!({ "path": self.relative_path }),
                },
            },
            ModelEvent::Usage {
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::ToolUse,
            },
        ]
    }

    fn answer_turn(&self, result: &ToolMessage) -> Vec<ModelEvent> {
        let answer = if result.is_error {
            format!("The read failed: {}", result.output)
        } else {
            let preview = result.output.lines().next().unwrap_or_default();
            format!(
                "Read {} successfully. First line: {}",
                Path::new(&self.relative_path).display(),
                preview
            )
        };
        vec![
            ModelEvent::TextDelta { delta: answer },
            ModelEvent::Usage {
                usage: Usage {
                    input_tokens: 15,
                    output_tokens: 8,
                },
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            },
        ]
    }
}

impl ModelProvider for ReadThenAnswerProvider {
    fn stream(&self, request: ModelRequest) -> ModelEventStream {
        let events = self.events_for(&request).into_iter().map(Ok);
        Box::pin(stream::iter(events))
    }
}

struct ToolMessage {
    output: String,
    is_error: bool,
}

fn tool_message(message: &Message) -> Option<ToolMessage> {
    let Message::Tool {
        output, is_error, ..
    } = message
    else {
        return None;
    };
    Some(ToolMessage {
        output: output.clone(),
        is_error: *is_error,
    })
}
