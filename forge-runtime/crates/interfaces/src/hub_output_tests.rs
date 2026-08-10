use forge_runtime_domain::{ConversationScope, HubSnapshot};

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
