use forge_runtime_domain::{
    EventSink, EventSinkError, Message, RunExecution, RunLimits, RunProvider, RunStore,
    RuntimeEvent, RuntimeEventKind, ToolCall,
};
use forge_runtime_infrastructure::SqliteHubStore;
use serde_json::json;

#[derive(Default)]
pub struct CountingSink {
    pub events: usize,
}

impl EventSink for CountingSink {
    fn emit(&mut self, _event: &RuntimeEvent) -> Result<(), EventSinkError> {
        self.events += 1;
        Ok(())
    }
}

pub struct ObservingFailSink {
    pub store: SqliteHubStore,
    pub observed_durable_event: bool,
}

impl EventSink for ObservingFailSink {
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError> {
        self.observed_durable_event = self
            .store
            .inspect_run(&event.run_id)
            .is_ok_and(|inspection| inspection.events.contains(event));
        Err(EventSinkError::new("deterministic downstream failure"))
    }
}

pub fn tool_call() -> ToolCall {
    ToolCall {
        id: "call-1".into(),
        name: "read_file".into(),
        arguments: json!({"path": "README.md"}),
    }
}

pub fn run_started() -> RuntimeEventKind {
    RuntimeEventKind::RunStarted {
        prompt: "inspect README".into(),
    }
}

pub fn current_user() -> RuntimeEventKind {
    RuntimeEventKind::MessageCommitted {
        message: Message::User {
            text: "inspect README".into(),
        },
    }
}

pub fn assistant(text: &str, tool_calls: Vec<ToolCall>) -> RuntimeEventKind {
    RuntimeEventKind::MessageCommitted {
        message: Message::Assistant {
            text: text.into(),
            tool_calls,
        },
    }
}

pub fn execution() -> RunExecution {
    RunExecution {
        provider: RunProvider::DeterministicRead {
            path: "README.md".into(),
        },
        system_prompt: "Read only what is authorized.".into(),
        allowed_read_paths: vec!["README.md".into()],
        limits: RunLimits::default(),
    }
}
