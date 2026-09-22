use forge_runtime_domain::{
    AgentTool, Cancellation, Capability, ToolContext, WorkspaceReadFactory,
    execution::fabric::{
        ENVIRONMENT_DIGEST_ALGORITHM, EnvironmentDigest, ExecutionAttempt, ToolInvocationRef,
    },
};
use serde_json::json;
use tempfile::TempDir;

use crate::CapStdWorkspaceFactory;

use super::{ExecCommandTool, LocalExecutionTarget, SafeEnvironment};

fn context(root: &std::path::Path, max_output_bytes: usize) -> ToolContext {
    ToolContext {
        workspace: CapStdWorkspaceFactory
            .open(root)
            .expect("workspace capability"),
        cancellation: Cancellation::default(),
        max_output_bytes,
    }
}

fn invocation() -> ToolInvocationRef {
    ToolInvocationRef {
        session_id: "session-1".into(),
        run_id: "run-1".into(),
        tool_call_id: "call-1".into(),
        tool_started_sequence: 4,
    }
}

#[test]
fn local_adapter_accepts_a_bounded_captured_environment_digest() {
    let root = TempDir::new().expect("temporary workspace");
    let target = LocalExecutionTarget::new(ExecCommandTool::new(root.path()).expect("tool"));
    let digest = SafeEnvironment::capture().digest();
    let attempt = ExecutionAttempt::local_process_with_environment(invocation(), digest);

    target
        .validate_attempt(&attempt)
        .expect("captured local environment remains valid");
    assert_eq!(attempt.effect.capability, Capability::Process);
    assert!(attempt.input_artifacts.is_empty());
    assert!(attempt.output_artifacts.is_empty());
}

#[test]
fn local_adapter_rejects_malformed_captured_environment_digest() {
    let root = TempDir::new().expect("temporary workspace");
    let target = LocalExecutionTarget::new(ExecCommandTool::new(root.path()).expect("tool"));
    let attempt = ExecutionAttempt::local_process_with_environment(
        invocation(),
        EnvironmentDigest::Captured {
            algorithm: ENVIRONMENT_DIGEST_ALGORITHM.into(),
            sha256: "not-a-digest".into(),
            entry_count: 1,
        },
    );

    let error = target
        .validate_attempt(&attempt)
        .expect_err("malformed digest must fail closed");
    assert_eq!(error.code, "invalid_execution_attempt");
}

#[cfg(unix)]
#[tokio::test]
async fn executed_attempt_uses_the_safe_environment_snapshot() {
    let root = TempDir::new().expect("temporary workspace");
    let tool = ExecCommandTool::new(root.path()).expect("tool");
    let output = tool
        .execute_with_invocation_evidence(
            json!({ "program": "printf", "argv": ["safe"] }),
            context(root.path(), 1024),
            invocation(),
        )
        .await
        .expect("local command succeeds");
    let evidence = output.execution_evidence.expect("evidence is bound");
    assert!(matches!(
        evidence.source,
        forge_runtime_domain::execution::fabric::ExecutionEvidenceSource::LocalProcessObservation
    ));
    assert!(output.output.content.contains("stdout:\nsafe"));
}
