use forge_runtime_domain::{
    LimitKind, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunInspection,
    RunJournalCursor, RunLimits, RunOutcome, RunProvider, RunRecord, RunRecoveryState,
    RuntimeEvent, RuntimeEventKind, ToolCall,
    execution::fabric::{
        EXECUTION_FABRIC_ABI_VERSION, ExecutionEvidence, ExecutionEvidenceSource, ExecutionTarget,
        LocalProcessObservation,
    },
};
use serde_json::json;

const PROMPT: &str = "inspect README";
const REJECTION_MESSAGE: &str = "tool call was not executed";

#[path = "support/run_journal_transcript_cases.rs"]
mod tests;

fn run_started_event() -> RuntimeEventKind {
    RuntimeEventKind::RunStarted {
        prompt: PROMPT.into(),
    }
}

fn through_assistant(calls: Vec<ToolCall>) -> Vec<RuntimeEventKind> {
    started(vec![
        RuntimeEventKind::TurnStarted { turn: 1 },
        assistant("working", calls),
    ])
}

fn started(mut following: Vec<RuntimeEventKind>) -> Vec<RuntimeEventKind> {
    let mut transcript = vec![
        run_started_event(),
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: PROMPT.into(),
            },
        },
    ];
    transcript.append(&mut following);
    transcript
}

fn assistant(text: &str, tool_calls: Vec<ToolCall>) -> RuntimeEventKind {
    RuntimeEventKind::MessageCommitted {
        message: Message::Assistant {
            text: text.into(),
            tool_calls,
        },
    }
}

fn provider_context() -> RuntimeEventKind {
    RuntimeEventKind::MessageCommitted {
        message: Message::ProviderContext {
            provider: "openai_responses".into(),
            items: vec![json!({"type": "reasoning", "id": "reasoning-1"})],
        },
    }
}

fn finished(call: &ToolCall, output: &str) -> RuntimeEventKind {
    RuntimeEventKind::ToolFinished {
        call_id: call.id.clone(),
        name: call.name.clone(),
        output: output.into(),
        is_error: false,
        truncated: false,
        execution_evidence: None,
    }
}

fn rejected(call: &ToolCall, code: &str) -> RuntimeEventKind {
    RuntimeEventKind::ToolRejected {
        call: call.clone(),
        code: code.into(),
        message: REJECTION_MESSAGE.into(),
    }
}

fn tool_message(
    call: &ToolCall,
    output: &str,
    is_error: bool,
    truncated: bool,
) -> RuntimeEventKind {
    RuntimeEventKind::MessageCommitted {
        message: Message::Tool {
            call_id: call.id.clone(),
            name: call.name.clone(),
            output: output.into(),
            is_error,
            truncated,
        },
    }
}

fn validate(
    kinds: Vec<RuntimeEventKind>,
) -> Result<RunInspection, forge_runtime_domain::RunJournalError> {
    RunInspection::validate(record(), events(kinds))
}

fn events(kinds: Vec<RuntimeEventKind>) -> Vec<RuntimeEvent> {
    kinds
        .into_iter()
        .enumerate()
        .map(|(index, kind)| runtime_event(u64::try_from(index + 1).expect("event sequence"), kind))
        .collect()
}

fn runtime_event(seq: u64, kind: RuntimeEventKind) -> RuntimeEvent {
    RuntimeEvent {
        v: PROTOCOL_VERSION,
        session_id: "conversation-1".into(),
        run_id: "run-1".into(),
        seq,
        emitted_at_ms: seq,
        kind,
    }
}

fn record() -> RunRecord {
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

fn tool_call() -> ToolCall {
    ToolCall {
        id: "call-1".into(),
        name: "read_file".into(),
        arguments: json!({"path": "README.md"}),
    }
}
