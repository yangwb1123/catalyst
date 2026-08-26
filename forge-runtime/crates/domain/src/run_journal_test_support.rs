use crate::{
    Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits, RunProvider, RunRecord,
    RuntimeEvent, RuntimeEventKind, ToolCall,
};
use serde_json::json;

pub(super) fn record() -> RunRecord {
    RunRecord {
        v: RUN_STORE_VERSION,
        run_id: "run-1".into(),
        conversation_id: "conversation-1".into(),
        prompt_id: "prompt-1".into(),
        project_id: "project-1".into(),
        execution: RunExecution {
            provider: RunProvider::DeterministicRead {
                path: "README.md".into(),
            },
            system_prompt: "answer".into(),
            allowed_read_paths: vec!["README.md".into()],
            limits: RunLimits::default(),
        },
        protocol_version: PROTOCOL_VERSION,
        created_at_ms: 1,
    }
}

pub(super) fn event(seq: u64, kind: RuntimeEventKind) -> RuntimeEvent {
    RuntimeEvent {
        v: PROTOCOL_VERSION,
        session_id: "conversation-1".into(),
        run_id: "run-1".into(),
        seq,
        emitted_at_ms: seq,
        kind,
    }
}

pub(super) fn user_event(seq: u64) -> RuntimeEvent {
    event(
        seq,
        RuntimeEventKind::MessageCommitted {
            message: Message::User { text: "p".into() },
        },
    )
}

pub(super) fn assistant_event(seq: u64, tool_calls: Vec<ToolCall>) -> RuntimeEvent {
    event(
        seq,
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: "done".into(),
                tool_calls,
            },
        },
    )
}

pub(super) fn tool_call() -> ToolCall {
    ToolCall {
        id: "call-1".into(),
        name: "read_file".into(),
        arguments: json!({"path": "README.md"}),
    }
}
