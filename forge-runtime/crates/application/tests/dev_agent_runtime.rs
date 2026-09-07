use std::{fs, sync::Arc};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_domain::{
    Cancellation, Capability, ModelEvent, ModelFinishReason, RunLimits, RunOutcome, RunRequest,
    RuntimeEventKind, ToolCall,
};
use forge_runtime_infrastructure::{
    CapStdWorkspaceFactory, EditFileTool, ExecCommandTool, MemoryEventSink, ReadFileTool,
    ScriptedProvider,
};
use serde_json::json;
use tempfile::TempDir;

type ProviderEvent = Result<ModelEvent, forge_runtime_domain::ProviderError>;

#[tokio::test]
async fn coding_agent_reads_edits_verifies_and_finishes() {
    let workspace = TempDir::new().expect("temporary workspace");
    fs::write(workspace.path().join("note.txt"), "broken\n").expect("fixture file");
    let runtime = runtime(&workspace, scripted_turns());
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&workspace), Cancellation::default(), &mut sink)
        .await
        .expect("agent run succeeds");

    assert_eq!(
        fs::read_to_string(workspace.path().join("note.txt")).unwrap(),
        "fixed\n"
    );
    assert_eq!(
        result.outcome,
        RunOutcome::Completed {
            answer: "fixed and verified".into(),
        }
    );
    let terminal_count = sink
        .events()
        .iter()
        .filter(|event| matches!(event.kind, RuntimeEventKind::RunFinished { .. }))
        .count();
    assert_eq!(terminal_count, 1);
}

fn runtime(workspace: &TempDir, turns: Vec<Vec<ProviderEvent>>) -> AgentRuntime {
    let mut tools = ToolCatalog::default();
    tools.register(Arc::new(ReadFileTool)).expect("read tool");
    tools
        .register(Arc::new(
            EditFileTool::open(workspace.path()).expect("edit tool"),
        ))
        .expect("edit registration");
    tools
        .register(Arc::new(
            ExecCommandTool::new(workspace.path()).expect("exec tool"),
        ))
        .expect("exec registration");
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(turns)),
        tools,
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn request(workspace: &TempDir) -> RunRequest {
    RunRequest {
        session_id: "dev-session".into(),
        run_id: "dev-run".into(),
        prompt: "repair and verify note.txt".into(),
        system_prompt: "Use the coding tools.".into(),
        workspace: workspace.path().to_path_buf(),
        allowed_capabilities: vec![
            Capability::WorkspaceRead,
            Capability::WorkspaceWrite,
            Capability::Process,
        ],
        limits: RunLimits {
            max_turns: 8,
            max_tool_calls: 8,
            max_tool_output_bytes: 16 * 1024,
            max_model_output_bytes: 64 * 1024,
            max_model_events: 128,
            max_output_tokens_per_turn: 4_096,
        },
    }
}

fn scripted_turns() -> Vec<Vec<ProviderEvent>> {
    vec![
        tool_turn("read-1", "read_file", json!({"path": "note.txt"})),
        tool_turn(
            "edit-1",
            "edit_file",
            json!({"path": "note.txt", "old_text": "broken\n", "new_text": "fixed\n"}),
        ),
        tool_turn(
            "exec-1",
            "exec_command",
            json!({"program": "sh", "argv": ["-c", "test \"$(cat note.txt)\" = fixed"]}),
        ),
        vec![
            Ok(ModelEvent::TextDelta {
                delta: "fixed and verified".into(),
            }),
            Ok(ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            }),
        ],
    ]
}

fn tool_turn(id: &str, name: &str, arguments: serde_json::Value) -> Vec<ProviderEvent> {
    vec![
        Ok(ModelEvent::ToolCall {
            call: ToolCall {
                id: id.into(),
                name: name.into(),
                arguments,
            },
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::ToolUse,
        }),
    ]
}
