use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{AgentRuntime, ConversationHistory, ToolCatalog};
use forge_runtime_domain::{
    AgentTool, CURRENT_AGENT_TOOLSET_VERSION, Cancellation, Capability,
    LEGACY_AGENT_TOOLSET_VERSION, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution,
    RunInspection, RunLimits, RunProvider, RunRecord, RunRequest, RuntimeEvent, RuntimeEventKind,
    ToolCall, ToolContext, ToolFuture, ToolOutput, ToolSpec, WorkspaceIdentity, WorkspaceOpenError,
    WorkspaceReadCapability, WorkspaceReadFactory,
};
use forge_runtime_infrastructure::{
    CapStdAgentWorkspace, CapStdWorkspaceFactory, MemoryEventSink, ReadFileTool, ScriptedProvider,
};
use serde_json::{Value, json};
use tempfile::TempDir;

mod support;

use support::ProbeTool;

#[tokio::test]
async fn legacy_pending_discovery_call_cannot_execute_with_a_current_catalog() {
    let root = TempDir::new().expect("workspace");
    let identity = workspace_identity(&root);
    let discovery = ProbeTool::succeeds("list_files", "must not execute");
    let mut catalog = ToolCatalog::default();
    for tool in [
        discovery.clone(),
        ProbeTool::succeeds("read_file", "unused"),
        ProbeTool::succeeds("search_text", "unused"),
    ] {
        catalog.register(tool).expect("current read-only tool");
    }
    let runtime = AgentRuntime::new(
        Arc::new(ScriptedProvider::new(Vec::new())),
        catalog,
        Arc::new(CapStdWorkspaceFactory),
    );
    let mut sink = MemoryEventSink::default();

    let error = runtime
        .resume_with_inspection(
            request(&root),
            pending_discovery_inspection(legacy_record(identity)),
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect_err("persisted legacy surface must reject a current catalog");

    assert!(
        error
            .to_string()
            .contains("does not match the runtime catalog")
    );
    assert_eq!(discovery.invocation_count(), 0);
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn unknown_toolset_fails_before_an_injected_workspace_factory_is_opened() {
    let root = TempDir::new().expect("workspace");
    let mut record = legacy_record(workspace_identity(&root));
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &mut record.execution.provider
    else {
        panic!("Agent provider expected");
    };
    *toolset_version = u16::MAX;
    let opens = Arc::new(AtomicUsize::new(0));
    let runtime = AgentRuntime::new(
        Arc::new(ScriptedProvider::new(Vec::new())),
        ToolCatalog::default(),
        Arc::new(CountingFactory {
            opens: opens.clone(),
        }),
    );
    let mut sink = MemoryEventSink::default();

    let error = runtime
        .resume_with_inspection(
            request(&root),
            pending_discovery_inspection(record),
            ConversationHistory::default(),
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect_err("unknown toolset must fail before workspace access");

    assert!(
        error
            .to_string()
            .contains("unsupported persisted Agent toolset")
    );
    assert_eq!(opens.load(Ordering::SeqCst), 0);
    assert!(sink.events().is_empty());
}

#[tokio::test]
async fn exact_surface_drift_fails_before_workspace_or_tool_effects() {
    for drift in [SpecDrift::Schema, SpecDrift::Description] {
        let root = TempDir::new().expect("workspace");
        let identity = workspace_identity(&root);
        let (catalog, invocations) = drifted_current_catalog(&root, drift);
        let opens = Arc::new(AtomicUsize::new(0));
        let runtime = AgentRuntime::new(
            Arc::new(ScriptedProvider::new(Vec::new())),
            catalog,
            Arc::new(CountingFactory {
                opens: opens.clone(),
            }),
        );
        let mut sink = MemoryEventSink::default();

        let error = runtime
            .resume_with_inspection(
                request(&root),
                pending_discovery_inspection(current_record(identity)),
                ConversationHistory::default(),
                Cancellation::default(),
                &mut sink,
            )
            .await
            .expect_err("schema and description drift must fail closed");

        assert!(
            error
                .to_string()
                .contains("does not match the runtime catalog")
        );
        assert_eq!(opens.load(Ordering::SeqCst), 0);
        assert_eq!(invocations.load(Ordering::SeqCst), 0);
        assert!(sink.events().is_empty());
    }
}

#[derive(Clone, Copy)]
enum SpecDrift {
    Schema,
    Description,
}

struct SpecProbe {
    spec: ToolSpec,
    invocations: Arc<AtomicUsize>,
}

impl AgentTool for SpecProbe {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn execute(&self, _arguments: Value, _context: ToolContext) -> ToolFuture<'_> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            Ok(ToolOutput {
                content: "unused".into(),
                truncated: false,
            })
        })
    }
}

fn drifted_current_catalog(root: &TempDir, drift: SpecDrift) -> (ToolCatalog, Arc<AtomicUsize>) {
    let workspace = CapStdAgentWorkspace::open(root.path()).expect("Agent workspace");
    let mut specs = vec![
        workspace.list_files_tool().spec(),
        ReadFileTool.spec(),
        workspace.search_text_tool().spec(),
    ];
    match drift {
        SpecDrift::Schema => specs[0].input_schema = json!({"type": "string"}),
        SpecDrift::Description => specs[0].description.push_str(" drifted"),
    }
    let invocations = Arc::new(AtomicUsize::new(0));
    let mut catalog = ToolCatalog::default();
    for spec in specs {
        catalog
            .register(Arc::new(SpecProbe {
                spec,
                invocations: invocations.clone(),
            }))
            .expect("drifted tool registration");
    }
    (catalog, invocations)
}

struct CountingFactory {
    opens: Arc<AtomicUsize>,
}

impl WorkspaceReadFactory for CountingFactory {
    fn open(
        &self,
        _workspace: &std::path::Path,
    ) -> Result<WorkspaceReadCapability, WorkspaceOpenError> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        Err(WorkspaceOpenError::new("must not open"))
    }
}

fn workspace_identity(root: &TempDir) -> WorkspaceIdentity {
    CapStdWorkspaceFactory
        .open(root.path())
        .expect("workspace capability")
        .workspace_identity()
        .cloned()
        .expect("workspace identity")
}

fn request(root: &TempDir) -> RunRequest {
    RunRequest {
        session_id: "conversation-1".into(),
        run_id: "run-1".into(),
        prompt: "durable prompt".into(),
        system_prompt: "persisted system prompt".into(),
        workspace: root.path().to_path_buf(),
        allowed_capabilities: vec![Capability::WorkspaceRead],
        limits: RunLimits::default(),
    }
}

fn legacy_record(workspace_identity: WorkspaceIdentity) -> RunRecord {
    RunRecord {
        v: RUN_STORE_VERSION,
        run_id: "run-1".into(),
        conversation_id: "conversation-1".into(),
        prompt_id: "prompt-1".into(),
        project_id: "project-1".into(),
        execution: RunExecution {
            provider: RunProvider::OpenAiAgent {
                endpoint: "http://127.0.0.1".into(),
                model: "test-model".into(),
                dev: false,
                toolset_version: LEGACY_AGENT_TOOLSET_VERSION,
                workspace_identity: Some(workspace_identity),
            },
            system_prompt: "persisted system prompt".into(),
            allowed_read_paths: Vec::new(),
            limits: RunLimits::default(),
        },
        protocol_version: PROTOCOL_VERSION,
        created_at_ms: 1,
    }
}

fn current_record(workspace_identity: WorkspaceIdentity) -> RunRecord {
    let mut record = legacy_record(workspace_identity);
    let RunProvider::OpenAiAgent {
        toolset_version, ..
    } = &mut record.execution.provider
    else {
        unreachable!("Agent record")
    };
    *toolset_version = CURRENT_AGENT_TOOLSET_VERSION;
    record
}

fn pending_discovery_inspection(record: RunRecord) -> RunInspection {
    let call = ToolCall {
        id: "list-1".into(),
        name: "list_files".into(),
        arguments: json!({}),
    };
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
                        text: String::new(),
                        tool_calls: vec![call],
                    },
                },
            ),
        ],
    )
    .expect("valid pre-ToolStarted legacy prefix")
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
