use super::{RunExplanationView, write_run_explanation};
use forge_runtime_domain::{
    CURRENT_AGENT_TOOLSET_VERSION, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution,
    RunInspection, RunLimits, RunProvider, RunRecord, RuntimeEvent, RuntimeEventKind, ToolCall,
    WorkspaceIdentity,
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
    assert!(explanation.continuation.safe);
    assert_eq!(
        explanation.continuation.command.as_deref(),
        Some("run resume run-1")
    );
    assert!(explanation.continuation.reason.contains("writeback"));
    assert!(human.contains("content_sha256="));
    assert!(human.contains("scope\tworkspace_read\tREADME.md"));
    assert!(!human.contains(PRIVATE_ANSWER));
    assert!(!human.contains(PRIVATE_SYSTEM_PROMPT));
}

#[test]
fn in_turn_continuation_discloses_possible_duplicate_provider_effect() {
    let events = vec![
        event(
            1,
            RuntimeEventKind::RunStarted {
                prompt: PRIVATE_PROMPT.into(),
            },
        ),
        user_event(2),
        event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
    ];
    let explanation = explain(events);

    assert!(!explanation.continuation.safe);
    assert_eq!(
        explanation.continuation.command.as_deref(),
        Some("run resume run-1")
    );
    assert!(explanation.continuation.reason.contains("disclosure"));
    assert!(explanation.continuation.reason.contains("cost"));
    assert!(explanation.continuation.reason.contains("will not replay"));
}

#[test]
fn read_only_agent_explanation_projects_capabilities_and_boundary() {
    let explanation = explain_agent(false);
    let json = serde_json::to_value(&explanation).expect("explanation JSON");
    let authorization = &json["authorization"];

    assert_eq!(json["provider"], "openai_agent");
    assert_capability(
        authorization,
        "workspace_read",
        "declared_and_runtime_exposed",
        &["selected_workspace"],
    );
    assert_capability(
        authorization,
        "workspace_write",
        "not_exposed_by_project_run_v1",
        &[],
    );
    assert_capability(
        authorization,
        "process",
        "not_exposed_by_project_run_v1",
        &[],
    );
    assert_capability(
        authorization,
        "network",
        "ambient_egress_without_network_tool_or_containment",
        &["provider egress"],
    );
    assert_eq!(
        json["context"]["workspace_outside_configured_read_scope"]["status"],
        "read_only_agent"
    );
    assert!(
        json["context"]["workspace_outside_configured_read_scope"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("not an OS sandbox"))
    );
    assert_human_agent_boundary(&explanation, false);
}

#[test]
fn dev_agent_explanation_projects_effectful_capabilities_and_boundary() {
    let explanation = explain_agent(true);
    let json = serde_json::to_value(&explanation).expect("explanation JSON");
    let authorization = &json["authorization"];

    assert_eq!(json["provider"], "openai_agent");
    assert_capability(
        authorization,
        "workspace_read",
        "declared_and_runtime_exposed",
        &["selected_workspace"],
    );
    assert_capability(
        authorization,
        "workspace_write",
        "declared_and_runtime_exposed",
        &["selected_workspace"],
    );
    assert_capability(
        authorization,
        "process",
        "declared_and_runtime_exposed",
        &[
            "initial_cwd_anchored_to_selected_workspace; subprocess retains ambient same-user filesystem and network access",
        ],
    );
    assert_capability(
        authorization,
        "network",
        "ambient_egress_without_network_tool_or_containment",
        &["provider egress plus possible subprocess ambient network access"],
    );
    assert_eq!(
        json["context"]["workspace_outside_configured_read_scope"]["status"],
        "explicit_dev_mode"
    );
    assert!(
        json["context"]["workspace_outside_configured_read_scope"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("not an OS sandbox"))
    );
    assert_human_agent_boundary(&explanation, true);
}

#[test]
fn unsupported_agent_toolset_is_not_reported_as_runtime_exposed() {
    let mut record = agent_run_record(true);
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &mut record.execution.provider
    else {
        unreachable!("agent fixture")
    };
    *toolset_version = CURRENT_AGENT_TOOLSET_VERSION.saturating_add(99);
    let explanation = explain_record(
        record,
        vec![
            event(
                1,
                RuntimeEventKind::RunStarted {
                    prompt: PRIVATE_PROMPT.into(),
                },
            ),
            user_event(2),
        ],
    );
    let json = serde_json::to_value(explanation).expect("explanation JSON");

    for capability in ["workspace_read", "workspace_write", "process", "network"] {
        assert_eq!(
            json["authorization"][capability]["status"],
            "declared_but_runtime_unavailable"
        );
    }
}

#[test]
fn first_party_agent_tool_names_receive_trusted_labels() {
    for name in [
        "list_files",
        "search_text",
        "read_file",
        "edit_file",
        "exec_command",
    ] {
        assert_eq!(super::projection::tool_name_label(name), name);
    }
}

fn assert_capability(authorization: &serde_json::Value, name: &str, status: &str, scope: &[&str]) {
    assert_eq!(authorization[name]["status"], status);
    assert_eq!(authorization[name]["scope"], json!(scope));
}

fn assert_human_agent_boundary(explanation: &RunExplanationView, dev: bool) {
    let human = render_human(explanation);
    let boundary = if dev {
        "agent workspace boundary: status=explicit_dev_mode"
    } else {
        "agent workspace boundary: status=read_only_agent"
    };
    for capability in ["workspace_read", "workspace_write", "process", "network"] {
        assert!(human.contains(&format!("{capability}: status=")));
    }
    assert!(human.contains("scope="));
    assert!(human.contains(boundary));
    assert!(human.contains("not an OS sandbox"));
    if dev {
        assert!(human.contains("scope\tworkspace_write\tselected_workspace"));
        assert!(human.contains("scope\tprocess\tinitial_cwd_anchored_to_selected_workspace"));
    }
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
    assert!(human.contains("scope\tworkspace_read\tREADME.md"));
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
    explain_record(run_record(), events)
}

fn explain_agent(dev: bool) -> RunExplanationView {
    explain_record(
        agent_run_record(dev),
        vec![
            event(
                1,
                RuntimeEventKind::RunStarted {
                    prompt: PRIVATE_PROMPT.into(),
                },
            ),
            user_event(2),
        ],
    )
}

fn explain_record(record: RunRecord, events: Vec<RuntimeEvent>) -> RunExplanationView {
    let inspection = RunInspection::validate(record, events).expect("valid Run prefix");
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

fn agent_run_record(dev: bool) -> RunRecord {
    let mut record = run_record();
    record.execution.provider = RunProvider::OpenAiAgent {
        endpoint: "https://provider.invalid/v1".into(),
        model: "private-model".into(),
        dev,
        toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
        workspace_identity: Some(WorkspaceIdentity::Unix {
            device: 7,
            inode: 11,
        }),
    };
    record.execution.allowed_read_paths.clear();
    record
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
