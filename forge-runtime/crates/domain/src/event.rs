use serde::{Deserialize, Serialize};

use crate::{Message, RunOutcome, ToolCall};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeEvent {
    pub v: u16,
    pub session_id: String,
    pub run_id: String,
    pub seq: u64,
    pub emitted_at_ms: u64,
    #[serde(flatten)]
    pub kind: RuntimeEventKind,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEventKind {
    RunStarted {
        prompt: String,
    },
    TurnStarted {
        turn: u32,
    },
    AssistantDelta {
        delta: String,
    },
    MessageCommitted {
        message: Message,
    },
    ToolStarted {
        call: ToolCall,
    },
    ToolFinished {
        call_id: String,
        name: String,
        output: String,
        is_error: bool,
        truncated: bool,
    },
    ToolRejected {
        call: ToolCall,
        code: String,
        message: String,
    },
    RuntimeError {
        code: String,
        message: String,
    },
    RunFinished {
        outcome: RunOutcome,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventSinkError {
    pub message: String,
}

impl EventSinkError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for EventSinkError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for EventSinkError {}

pub trait EventSink {
    /// Appends one complete event to the sink.
    ///
    /// # Errors
    ///
    /// Returns an error when the sink cannot accept or write the event.
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError>;
}

#[cfg(test)]
mod tests {
    use super::{PROTOCOL_VERSION, RuntimeEvent, RuntimeEventKind};

    #[test]
    fn event_kind_is_flattened_into_envelope() {
        let event = RuntimeEvent {
            v: PROTOCOL_VERSION,
            session_id: "session-1".into(),
            run_id: "run-1".into(),
            seq: 1,
            emitted_at_ms: 0,
            kind: RuntimeEventKind::TurnStarted { turn: 2 },
        };

        let value = serde_json::to_value(event).expect("event serializes");
        assert_eq!(value["type"], "turn_started");
        assert_eq!(value["turn"], 2);
        assert_eq!(value["v"], PROTOCOL_VERSION);
    }
}
