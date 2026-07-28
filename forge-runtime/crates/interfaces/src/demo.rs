use std::{
    error::Error,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_infrastructure::{
    CapStdWorkspaceFactory, JsonlEventSink, ReadFileTool, ReadThenAnswerProvider,
};

use crate::{
    args::DemoArgs,
    runtime_domain::{Cancellation, Capability, RunLimits, RunOutcome, RunRequest},
    state_path::{canonical_project, unique_id},
};

pub async fn run(args: &DemoArgs, project: Option<&Path>) -> Result<RunOutcome, Box<dyn Error>> {
    let workspace = canonical_project(project.unwrap_or_else(|| Path::new(".")))?;
    let provider = Arc::new(ReadThenAnswerProvider::new(args.read_path.clone()));
    let mut tools = ToolCatalog::default();
    tools.register(Arc::new(ReadFileTool))?;
    let runtime = AgentRuntime::new(provider, tools, Arc::new(CapStdWorkspaceFactory));
    let request = request_from(args, workspace);
    let stdout = io::stdout();
    let mut sink = JsonlEventSink::new(stdout.lock());
    let result = runtime
        .run(request, Cancellation::default(), &mut sink)
        .await?;
    Ok(result.outcome)
}

fn request_from(args: &DemoArgs, workspace: PathBuf) -> RunRequest {
    RunRequest {
        session_id: unique_id("session"),
        run_id: unique_id("run"),
        prompt: args.prompt.clone(),
        system_prompt: "Use the available read-only tools to answer the user.".into(),
        workspace,
        allowed_capabilities: vec![Capability::WorkspaceRead],
        limits: RunLimits {
            max_turns: 4,
            max_tool_calls: 4,
            max_tool_output_bytes: 64 * 1024,
            max_model_output_bytes: 64 * 1024,
            max_model_events: 4_096,
            max_output_tokens_per_turn: 4_096,
        },
    }
}
