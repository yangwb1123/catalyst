use super::{RunExplanationView, write_run_explanation};
use forge_runtime_domain::{
    Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunInspection, RunLimits,
    RunProvider, RunRecord, RuntimeEvent, RuntimeEventKind, ToolCall,
};
use serde_json::json;

const PRIVATE_CALL_ID: &str = "provider-secret-call-id";
const PRIVATE_PROMPT: &str = "private user prompt";
const PRIVATE_OUTPUT: &str = "private tool output";
const PRIVATE_ANSWER: &str = "private assistant answer";
const PRIVATE_SYSTEM_PROMPT: &str = "private system prompt";

#[test]
fn rejected_tool_is_summarized_without_exposing_provider_call_id() {
    let call = tool_call();
    let events = vec![
        event(
            1,
            RuntimeEventKind::RunStarted {
                prompt: PRIVATE_PROMPT.into(),
            },
        ),
        user_event(2),
        event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
        assistant_call_event(4, &call),
        event(
            5,
            RuntimeEventKind::ToolRejected {
                call: call.clone(),
                code: "capability_denied".into(),
                message: "read capability was not granted".into(),
            },
        ),
        event(
            6,
            RuntimeEventKind::MessageCommitted {
                message: Message::Tool {
                    call_id: call.id,
                    name: call.name,
                    output: "capability_denied: read capability was not granted".into(),
                    is_error: true,
                    truncated: false,
                },
            },
        ),
    ];
    let explanation = explain(events);

    assert_eq!(explanation.context.observed_tool_calls.len(), 1);
    let observed = &explanation.context.observed_tool_calls[0];
    assert_eq!(observed.outcome, "rejected");
    assert_eq!(observed.call_id_fingerprint.bytes, PRIVATE_CALL_ID.len());
    assert_eq!(observed.name_label, "unrecognized");
    assert_private_content_absent(&explanation);
}

#[test]
fn finished_tool_output_is_fingerprinted_before_message_commit() {
    let call = tool_call();
    let events = vec![
        event(
            1,
            RuntimeEventKind::RunStarted {
                prompt: PRIVATE_PROMPT.into(),
            },
        ),
        user_event(2),
        event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
        assistant_call_event(4, &call),
        event(5, RuntimeEventKind::ToolStarted { call: call.clone() }),
        event(
            6,
            RuntimeEventKind::ToolFinished {
                call_id: call.id,
                name: call.name,
                output: PRIVATE_OUTPUT.into(),
                is_error: false,
                truncated: false,
            },
        ),
    ];
    let explanation = explain(events);
    let observed = &explanation.context.observed_tool_calls[0];

    assert_eq!(observed.outcome, "finished");
    assert_eq!(
        observed.output.as_ref().map(|value| value.bytes),
        Some(PRIVATE_OUTPUT.len())
    );
    assert!(render_human(&explanation).contains("output_sha256="));
    assert_private_content_absent(&explanation);
}

#[test]
fn human_output_includes_hashed_context_scope_and_terminal_outcome() {
    let events = vec![
        event(
            1,
            RuntimeEventKind::RunStarted {
                prompt: PRIVATE_PROMPT.into(),
            },
        ),
        user_event(2),
        event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
        event(
            4,
            RuntimeEventKind::MessageCommitted {
                message: Message::Assistant {
                    text: PRIVATE_ANSWER.into(),
                    tool_calls: Vec::new(),
                },
            },
        ),
        event(
            5,
            RuntimeEventKind::RunFinished {
                outcome: forge_runtime_domain::RunOutcome::Completed {
                    answer: PRIVATE_ANSWER.into(),
                },
            },
        ),
    ];
    let explanation = explain(events);
    let human = render_human(&explanation);

    assert!(human.contains("status=terminal outcome=completed"));
    assert!(human.contains("content_sha256="));
    assert!(human.contains("scope\tREADME.md"));
    assert!(!human.contains(PRIVATE_ANSWER));
    assert!(!human.contains(PRIVATE_SYSTEM_PROMPT));
}

fn assert_private_content_absent(explanation: &RunExplanationView) {
    let json = serde_json::to_string(explanation).expect("explanation serializes");
    assert!(!json.contains(PRIVATE_CALL_ID));
    assert!(!json.contains(PRIVATE_PROMPT));
    assert!(!json.contains(PRIVATE_OUTPUT));
    assert!(!json.contains(PRIVATE_ANSWER));
    assert!(!json.contains(PRIVATE_SYSTEM_PROMPT));

    let human = render_human(explanation);
    assert!(human.contains("continuation safe=true"));
    assert!(human.contains("call_id_sha256="));
    assert!(human.contains("content_sha256="));
    assert!(human.contains("scope\tREADME.md"));
    assert!(!human.contains(PRIVATE_CALL_ID));
    assert!(!human.contains(PRIVATE_PROMPT));
    assert!(!human.contains(PRIVATE_OUTPUT));
    assert!(!human.contains(PRIVATE_ANSWER));
    assert!(!human.contains(PRIVATE_SYSTEM_PROMPT));
}

fn render_human(explanation: &RunExplanationView) -> String {
    let mut human = Vec::new();
    write_run_explanation(explanation, &mut human).expect("human output");
    String::from_utf8(human).expect("UTF-8 output")
}

fn explain(events: Vec<RuntimeEvent>) -> RunExplanationView {
    let inspection = RunInspection::validate(run_record(), events).expect("valid Run prefix");
    RunExplanationView::from_inspection(&inspection).expect("Run explanation")
}

fn run_record() -> RunRecord {
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
            system_prompt: PRIVATE_SYSTEM_PROMPT.into(),
            allowed_read_paths: vec!["README.md".into()],
            limits: RunLimits::default(),
        },
        protocol_version: PROTOCOL_VERSION,
        created_at_ms: 1,
    }
}

fn tool_call() -> ToolCall {
    ToolCall {
        id: PRIVATE_CALL_ID.into(),
        name: format!("untrusted\n{PRIVATE_PROMPT}\t{PRIVATE_OUTPUT}\r{PRIVATE_CALL_ID}"),
        arguments: json!({"path": "README.md"}),
    }
}

fn user_event(seq: u64) -> RuntimeEvent {
    event(
        seq,
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: PRIVATE_PROMPT.into(),
            },
        },
    )
}

fn assistant_call_event(seq: u64, call: &ToolCall) -> RuntimeEvent {
    event(
        seq,
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: String::new(),
                tool_calls: vec![call.clone()],
            },
        },
    )
}

fn event(seq: u64, kind: RuntimeEventKind) -> RuntimeEvent {
    RuntimeEvent {
        v: PROTOCOL_VERSION,
        session_id: "conversation-1".into(),
        run_id: "run-1".into(),
        seq,
        emitted_at_ms: seq,
        kind,
    }
}
