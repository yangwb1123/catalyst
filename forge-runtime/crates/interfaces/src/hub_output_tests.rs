use forge_runtime_domain::{ConversationScope, HubSnapshot, PromptRecord};

use super::{CliOutput, OutputKind, RemoteStatus, write_output};

#[test]
fn json_output_has_a_version_and_type() {
    let output = CliOutput::new(OutputKind::Hub {
        snapshot: HubSnapshot {
            scope: ConversationScope::Global,
            projects: vec![],
            conversations: vec![],
            groups: vec![],
            group_project_members: vec![],
        },
        remote: RemoteStatus::NotConfigured,
    });
    let mut bytes = Vec::new();
    write_output(&output, true, &mut bytes).expect("render JSON");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("valid JSON");
    assert_eq!(value["v"], 1);
    assert_eq!(value["type"], "hub");
    assert_eq!(value["remote"], "not_configured");
}

#[test]
fn human_prompt_output_escapes_provider_control_text() {
    let output = CliOutput::new(OutputKind::Prompts {
        prompts: vec![PromptRecord {
            id: "prompt\nspoof".into(),
            conversation_id: "session\tspoof".into(),
            role: "assistant\rspoof".into(),
            content: "answer\n\u{1b}[2J[run] completed\u{202e}".into(),
            idempotency_key: "private".into(),
            created_at_ms: 1,
        }],
    });
    let mut bytes = Vec::new();
    write_output(&output, false, &mut bytes).expect("render human output");
    let text = String::from_utf8(bytes).expect("UTF-8 output");

    assert_eq!(
        text,
        "prompts: 1\nprompt\\nspoof\tsession\\tspoof\tassistant\\rspoof\t\
         answer\\n\\x1b[2J[run] completed\\u{202e}\n"
    );
    assert!(!text.contains('\u{1b}'));
}
