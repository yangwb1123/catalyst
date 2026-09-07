use forge_runtime_domain::{
    CURRENT_AGENT_TOOLSET_VERSION, Capability, LEGACY_AGENT_TOOLSET_VERSION, RunExecution,
    RunLimits, RunProvider, WorkspaceIdentity,
};

fn agent_execution(dev: bool) -> RunExecution {
    RunExecution {
        provider: RunProvider::OpenAiAgent {
            endpoint: "https://api.openai.com/v1".into(),
            model: "test-model".into(),
            dev,
            toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: Some(WorkspaceIdentity::Unix {
                device: 7,
                inode: 11,
            }),
        },
        system_prompt: "test agent".into(),
        allowed_read_paths: Vec::new(),
        limits: RunLimits::default(),
    }
}

#[test]
fn legacy_agent_wire_without_identity_remains_readable() {
    let json = r#"{
        "provider":{"kind":"open_ai_agent","endpoint":"local","model":"test","dev":true},
        "system_prompt":"test agent","allowed_read_paths":[],
        "limits":{"max_turns":8,"max_tool_calls":16,"max_tool_output_bytes":65536,
        "max_model_output_bytes":65536,"max_model_events":4096,
        "max_output_tokens_per_turn":4096}}
    "#;
    let execution: RunExecution = serde_json::from_str(json).expect("legacy Agent execution");

    assert!(matches!(
        execution.provider,
        RunProvider::OpenAiAgent {
            toolset_version: LEGACY_AGENT_TOOLSET_VERSION,
            workspace_identity: None,
            ..
        }
    ));
}

#[test]
fn agent_mode_round_trips_with_its_capability_boundary() {
    let dev = agent_execution(true);
    let encoded = serde_json::to_vec(&dev).expect("serialize dev execution");
    let value: serde_json::Value = serde_json::from_slice(&encoded).expect("execution JSON");
    let decoded: RunExecution = serde_json::from_slice(&encoded).expect("deserialize execution");

    assert_eq!(decoded, dev);
    assert_eq!(
        value["provider"]["toolset_version"],
        CURRENT_AGENT_TOOLSET_VERSION
    );
    assert!(decoded.is_agent());
    assert_eq!(
        decoded.allowed_capabilities(),
        [
            Capability::WorkspaceRead,
            Capability::WorkspaceWrite,
            Capability::Process,
        ]
    );
}

#[test]
fn read_only_agent_does_not_gain_mutating_capabilities() {
    let execution = agent_execution(false);

    assert!(execution.is_agent());
    assert_eq!(
        execution.allowed_capabilities(),
        [Capability::WorkspaceRead]
    );
}
