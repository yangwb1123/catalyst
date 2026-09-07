use std::{fs, path::Path, sync::Arc};

use forge_runtime_application::{AgentRuntime, ConversationHistory, ToolCatalog};
use forge_runtime_domain::{
    CURRENT_AGENT_TOOLSET_VERSION, Cancellation, Capability, Message, ModelEvent,
    ModelFinishReason, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunInspection, RunLimits,
    RunOutcome, RunProvider, RunRecord, RunRequest, RuntimeEvent, RuntimeEventKind, ToolCall,
    WorkspaceIdentity, WorkspaceReadFactory,
};
use forge_runtime_infrastructure::{
    CapStdAgentWorkspace, CapStdWorkspaceFactory, MemoryEventSink, ReadFileTool, ScriptedProvider,
};
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn public_resume_rejects_caller_execution_drift() {
    let root = TempDir::new().expect("workspace");
    let inspection = durable_inspection();
    let baseline = request(&root);
    let mut variants = Vec::new();

    let mut limits = baseline.clone();
    limits.limits.max_tool_output_bytes += 1;
    variants.push(limits);
    let mut system_prompt = baseline.clone();
    system_prompt.system_prompt = "changed system prompt".into();
    variants.push(system_prompt);
    let mut capability = baseline;
    capability
        .allowed_capabilities
        .push(Capability::WorkspaceRead);
    variants.push(capability);

    for request in variants {
        let mut sink = MemoryEventSink::default();
        let error = runtime()
            .resume_with_inspection(
                request,
                inspection.clone(),
                ConversationHistory::default(),
                Cancellation::default(),
                &mut sink,
            )
            .await
            .expect_err("caller execution drift must fail");

        assert!(
            error
                .to_string()
                .contains("persisted execution configuration")
        );
        assert!(sink.events().is_empty());
    }
}

#[tokio::test]
async fn public_resume_rejects_a_journal_less_run() {
    let root = TempDir::new().expect("workspace");
    let inspection = RunInspection::validate(record(), Vec::new()).expect("empty prefix");
    let mut sink = MemoryEventSink::default();

    let error = runtime()
        .resume_with_inspection(
            request(&root),
            inspection,
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect_err("journal-less resume must fail");

    assert!(error.to_string().contains("non-empty durable Run prefix"));
    assert!(sink.events().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn dev_resume_rejects_a_replaced_workspace_before_any_effect() {
    let base = TempDir::new().expect("temporary root");
    let workspace = base.path().join("workspace");
    let original = base.path().join("original-workspace");
    fs::create_dir(&workspace).expect("workspace");
    fs::write(workspace.join("note.txt"), "original\n").expect("original fixture");
    let identity = CapStdWorkspaceFactory
        .open(&workspace)
        .expect("workspace capability")
        .workspace_identity()
        .cloned()
        .expect("workspace identity");
    fs::rename(&workspace, &original).expect("move original workspace");
    fs::create_dir(&workspace).expect("replacement workspace");
    fs::write(workspace.join("note.txt"), "replacement\n").expect("replacement fixture");
    let mut sink = MemoryEventSink::default();

    let error = dev_runtime(&workspace)
        .resume_with_inspection(
            dev_request(&workspace),
            inspection(dev_record(Some(identity))),
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect_err("replaced workspace must fail closed");

    assert!(error.to_string().contains("workspace identity changed"));
    assert!(sink.events().is_empty());
    assert_eq!(
        fs::read_to_string(workspace.join("note.txt")).unwrap(),
        "replacement\n"
    );
    assert_eq!(
        fs::read_to_string(original.join("note.txt")).unwrap(),
        "original\n"
    );
}

#[tokio::test]
async fn legacy_dev_resume_without_identity_fails_closed() {
    let root = TempDir::new().expect("workspace");
    let mut sink = MemoryEventSink::default();

    let error = dev_runtime(root.path())
        .resume_with_inspection(
            dev_request(root.path()),
            finish_inspection(dev_record(None)),
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect_err("identity-less dev resume must fail closed");

    assert!(error.to_string().contains("persisted workspace identity"));
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn agent_resume_accepts_the_same_workspace_identity() {
    let root = TempDir::new().expect("workspace");
    let identity = CapStdWorkspaceFactory
        .open(root.path())
        .expect("workspace capability")
        .workspace_identity()
        .cloned()
        .expect("workspace identity");
    let mut sink = MemoryEventSink::default();

    let result = finishing_runtime()
        .resume_with_inspection(
            read_only_request(root.path()),
            inspection(agent_record(false, Some(identity))),
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect("same workspace identity resumes");

    assert_eq!(
        result.outcome,
        RunOutcome::Completed {
            answer: "done".into()
        }
    );
    assert!(!sink.events().is_empty());
}

#[tokio::test]
async fn unknown_agent_toolset_resume_fails_before_any_event() {
    let root = TempDir::new().expect("workspace");
    let identity = CapStdWorkspaceFactory
        .open(root.path())
        .expect("workspace capability")
        .workspace_identity()
        .cloned()
        .expect("workspace identity");
    let mut record = agent_record(false, Some(identity));
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &mut record.execution.provider
    else {
        panic!("Agent provider expected");
    };
    *toolset_version = u16::MAX;
    let mut sink = MemoryEventSink::default();

    let error = runtime()
        .resume_with_inspection(
            read_only_request(root.path()),
            inspection(record),
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect_err("unknown toolset must fail closed");

    assert!(
        error
            .to_string()
            .contains("unsupported persisted Agent toolset")
    );
    assert!(sink.events().is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn agent_finish_resume_rejects_replacement_before_terminal_event() {
    let base = TempDir::new().expect("temporary root");
    let workspace = base.path().join("workspace");
    let original = base.path().join("original-workspace");
    fs::create_dir(&workspace).expect("workspace");
    fs::write(workspace.join("note.txt"), "original secret\n").expect("original fixture");
    let identity = CapStdWorkspaceFactory
        .open(&workspace)
        .expect("workspace capability")
        .workspace_identity()
        .cloned()
        .expect("workspace identity");
    fs::rename(&workspace, &original).expect("move original workspace");
    fs::create_dir(&workspace).expect("replacement workspace");
    fs::write(workspace.join("note.txt"), "replacement secret\n").expect("replacement fixture");
    let mut sink = MemoryEventSink::default();

    let error = read_only_runtime()
        .resume_with_inspection(
            read_only_request(&workspace),
            finish_inspection(agent_record(false, Some(identity))),
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect_err("replaced workspace must fail before terminal repair");

    assert!(error.to_string().contains("workspace identity changed"));
    assert!(sink.events().is_empty());
}

fn runtime() -> AgentRuntime {
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(Vec::new())),
        ToolCatalog::default(),
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn finishing_runtime() -> AgentRuntime {
    let turn = vec![
        Ok(ModelEvent::TextDelta {
            delta: "done".into(),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ];
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(vec![turn])),
        current_read_only_catalog(),
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn read_only_runtime() -> AgentRuntime {
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(Vec::new())),
        current_read_only_catalog(),
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn current_read_only_catalog() -> ToolCatalog {
    let root = TempDir::new().expect("tool surface workspace");
    let workspace = CapStdAgentWorkspace::open(root.path()).expect("Agent workspace");
    let mut catalog = ToolCatalog::default();
    catalog
        .register(Arc::new(workspace.list_files_tool()))
        .expect("list tool");
    catalog.register(Arc::new(ReadFileTool)).expect("read tool");
    catalog
        .register(Arc::new(workspace.search_text_tool()))
        .expect("search tool");
    catalog
}

fn durable_inspection() -> RunInspection {
    inspection(record())
}

fn finish_inspection(record: RunRecord) -> RunInspection {
    RunInspection::validate(
        record,
        vec![
            event(
                1,
                RuntimeEventKind::RunStarted {
                    prompt: "durable prompt".into(),
                },
            ),
            event(
                2,
                RuntimeEventKind::MessageCommitted {
                    message: Message::User {
                        text: "durable prompt".into(),
                    },
                },
            ),
            event(3, RuntimeEventKind::TurnStarted { turn: 1 }),
            event(
                4,
                RuntimeEventKind::MessageCommitted {
                    message: Message::Assistant {
                        text: "done".into(),
                        tool_calls: Vec::new(),
                    },
                },
            ),
        ],
    )
    .expect("valid finish prefix")
}

fn inspection(record: RunRecord) -> RunInspection {
    RunInspection::validate(
        record,
        vec![
            event(
                1,
                RuntimeEventKind::RunStarted {
                    prompt: "durable prompt".into(),
                },
            ),
            event(
                2,
                RuntimeEventKind::MessageCommitted {
                    message: Message::User {
                        text: "durable prompt".into(),
                    },
                },
            ),
        ],
    )
    .expect("valid durable prefix")
}

fn dev_runtime(workspace: &Path) -> AgentRuntime {
    let bundle = CapStdAgentWorkspace::open(workspace).expect("Agent workspace");
    let mut tools = ToolCatalog::default();
    tools.register(Arc::new(ReadFileTool)).expect("read tool");
    tools
        .register(Arc::new(bundle.list_files_tool()))
        .expect("list tool");
    tools
        .register(Arc::new(bundle.search_text_tool()))
        .expect("search tool");
    tools
        .register(Arc::new(bundle.edit_file_tool()))
        .expect("edit tool");
    tools
        .register(Arc::new(bundle.exec_command_tool()))
        .expect("process tool");
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(dev_turns())),
        tools,
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn dev_turns() -> Vec<Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>>> {
    vec![vec![
        Ok(ModelEvent::ToolCall {
            call: ToolCall {
                id: "edit-1".into(),
                name: "edit_file".into(),
                arguments: json!({
                    "path": "note.txt",
                    "old_text": "replacement\n",
                    "new_text": "effect\n"
                }),
            },
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::ToolUse,
        }),
    ]]
}

fn request(root: &TempDir) -> RunRequest {
    RunRequest {
        session_id: "conversation-1".into(),
        run_id: "run-1".into(),
        prompt: "durable prompt".into(),
        system_prompt: "persisted system prompt".into(),
        workspace: root.path().to_path_buf(),
        allowed_capabilities: Vec::new(),
        limits: RunLimits::default(),
    }
}

fn dev_request(workspace: &Path) -> RunRequest {
    RunRequest {
        session_id: "conversation-1".into(),
        run_id: "run-1".into(),
        prompt: "durable prompt".into(),
        system_prompt: "persisted system prompt".into(),
        workspace: workspace.to_path_buf(),
        allowed_capabilities: vec![
            Capability::WorkspaceRead,
            Capability::WorkspaceWrite,
            Capability::Process,
        ],
        limits: RunLimits::default(),
    }
}

fn read_only_request(workspace: &Path) -> RunRequest {
    RunRequest {
        allowed_capabilities: vec![Capability::WorkspaceRead],
        workspace: workspace.to_path_buf(),
        ..dev_request(workspace)
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
            system_prompt: "persisted system prompt".into(),
            allowed_read_paths: Vec::new(),
            limits: RunLimits::default(),
        },
        protocol_version: PROTOCOL_VERSION,
        created_at_ms: 1,
    }
}

fn dev_record(workspace_identity: Option<WorkspaceIdentity>) -> RunRecord {
    agent_record(true, workspace_identity)
}

fn agent_record(dev: bool, workspace_identity: Option<WorkspaceIdentity>) -> RunRecord {
    RunRecord {
        execution: RunExecution {
            provider: RunProvider::OpenAiAgent {
                endpoint: "http://127.0.0.1".into(),
                model: "test-model".into(),
                dev,
                toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
                workspace_identity,
            },
            system_prompt: "persisted system prompt".into(),
            allowed_read_paths: Vec::new(),
            limits: RunLimits::default(),
        },
        ..record()
    }
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
