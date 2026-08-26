use std::sync::Arc;

use forge_runtime_application::{AgentRuntime, ConversationHistory, ToolCatalog};
use forge_runtime_domain::{
    Cancellation, Capability, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution,
    RunInspection, RunLimits, RunProvider, RunRecord, RunRequest, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::{CapStdWorkspaceFactory, MemoryEventSink, ScriptedProvider};
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

fn runtime() -> AgentRuntime {
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(Vec::new())),
        ToolCatalog::default(),
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn durable_inspection() -> RunInspection {
    RunInspection::validate(
        record(),
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
