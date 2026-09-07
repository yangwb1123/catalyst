use std::{error::Error, io, path::PathBuf, sync::Arc};

use crate::{
    agent_run_limits, agent_workspace,
    args::Args,
    human_event_sink::HumanEventSink,
    run_provider,
    run_selection::validate_project_binding,
    runtime_application::{
        AgentRuntime, ConversationHistory, ConversationHistoryBridge, HubService, RunService,
        RuntimeError,
    },
    runtime_domain::{
        BeginRun, BeginRunDisposition, CURRENT_AGENT_TOOLSET_VERSION, Cancellation, Capability,
        RUN_STORE_VERSION, RunExecution, RunInspection, RunLimits, RunOutcome, RunProvider,
        RunRecord, RunRecoveryState, RunRequest, RunResult,
    },
    state_path::{
        canonical_project, hub_database_path, idempotency_key, unique_id, unix_time_millis,
    },
};
use forge_runtime_infrastructure::{
    CapStdAgentWorkspace, DurableFirstEventSink, JsonlEventSink, RunExecutionGuard, SqliteHubStore,
};

#[path = "run_cancellation.rs"]
mod cancellation_signal;
use cancellation_signal::cancellation_listener;

#[path = "run_agent_start.rs"]
mod agent_start;
pub(crate) use agent_start::{SeededAgentReplayError, start_seeded_agent};
#[path = "run_resume_execution.rs"]
mod resume_execution;
use resume_execution::execute_resume;

struct PreparedRun {
    store: Arc<SqliteHubStore>,
    workspace: PathBuf,
    agent_workspace: Option<CapStdAgentWorkspace>,
    run: RunRecord,
    prompt: String,
    history: ConversationHistory,
    disposition: BeginRunDisposition,
}
struct RunSetup {
    store: Arc<SqliteHubStore>,
    workspace: PathBuf,
    agent_workspace: Option<CapStdAgentWorkspace>,
    begin: BeginRun,
}
struct PreparedResume {
    _execution_guard: RunExecutionGuard,
    store: Arc<SqliteHubStore>,
    runtime: AgentRuntime,
    request: RunRequest,
    inspection: RunInspection,
    history: ConversationHistory,
}
enum ResumePreparation {
    Execute(Box<PreparedResume>),
    ReconcileCompleted {
        store: Arc<SqliteHubStore>,
        _execution_guard: RunExecutionGuard,
    },
}
enum ResumeAction {
    Execute,
    ReconcileCompleted,
}
const MAX_HISTORY_CONTENT_BYTES: usize = 512 * 1024;
const READ_SYSTEM_PROMPT: &str = "Use only the available read-only tools to answer the user.";
const NO_TOOL_SYSTEM_PROMPT: &str =
    "Answer the user without tools. No workspace access is available.";
const AGENT_SYSTEM_PROMPT: &str = "You are Forge, a coding agent working in the selected local workspace. Work only on the user's current task; do not invent or continue a Sprint, Roadmap, or unrelated backlog unless the user explicitly asks. Use list_files and search_text to discover relevant project instructions and code before reading or changing files, make focused edits, run relevant checks, and report concrete results. Use tool arguments directly; do not assume a shell unless you explicitly invoke one.";
const READ_ONLY_AGENT_SYSTEM_PROMPT: &str = "You are Forge, a read-only coding assistant working in the selected local workspace. Work only on the user's current task; do not invent or continue a Sprint, Roadmap, or unrelated backlog unless the user explicitly asks. Use list_files and search_text to discover relevant project instructions and code, inspect the needed files, and give concrete guidance, but do not claim to have changed files or run commands.";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentAccess {
    ReadOnly,
    Dev,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartMode {
    Deterministic,
    Live,
    Agent(AgentAccess),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartOutput {
    JsonLines,
    Human,
}

pub struct StartOptions<'a> {
    pub conversation_id: &'a str,
    pub prompt_id: &'a str,
    pub read_path: &'a str,
    pub allowed_read_paths: &'a [String],
    pub mode: StartMode,
    pub model: Option<&'a str>,
    pub max_output_tokens: u32,
    pub output: StartOutput,
    pub max_turns: u32,
    pub max_tool_calls: u32,
}

pub async fn start(args: &Args, options: StartOptions<'_>) -> Result<RunOutcome, Box<dyn Error>> {
    let setup = prepare_setup(args, &options)?;
    let service = RunService::new(setup.store.clone());
    if service
        .find_run_by_idempotency_key(&setup.begin.idempotency_key)?
        .is_some()
    {
        return reconcile_terminal(&begin_prepared(setup, ConversationHistory::default())?);
    }
    let _execution_guard = setup.store.try_acquire_run_execution_guard()?;
    let provider = run_provider::for_execution(&setup.begin.execution)?;
    let history = load_history(&setup)?;
    let prepared = begin_prepared(setup, history)?;
    if prepared.disposition == BeginRunDisposition::Replayed {
        return reconcile_terminal(&prepared);
    }
    let (tools, allowed_capabilities) =
        agent_workspace::runtime_tools(&prepared.run.execution, prepared.agent_workspace.as_ref())?;
    let workspace_factory = agent_workspace::runtime_factory(
        &prepared.run.execution,
        prepared.agent_workspace.clone(),
    )?;
    let runtime = AgentRuntime::new(provider, tools, workspace_factory);
    let request = runtime_request(&prepared, allowed_capabilities);
    let result = execute_runtime(
        &runtime,
        request,
        &prepared,
        options.output == StartOutput::Human,
        matches!(options.mode, StartMode::Agent(_)),
    )
    .await;
    match result {
        Ok(_) => reconcile_terminal(&prepared),
        Err(error) => match reconcile_terminal(&prepared) {
            Ok(_) => Err(Box::new(error)),
            Err(reconcile_error) => Err(format!(
                "runtime failed: {error}; durable reconciliation failed: {reconcile_error}"
            )
            .into()),
        },
    }
}

/// Explicitly resumes one incomplete durable Run from its last committed
/// journal event, or reconciles a completed Run whose assistant writeback was
/// interrupted. Provider credentials and tools are constructed only for an
/// executable incomplete prefix.
pub async fn resume(args: &Args, run_id: &str) -> Result<RunOutcome, Box<dyn Error>> {
    match prepare_resume(args, run_id)? {
        ResumePreparation::Execute(prepared) => {
            let human = !args.json && prepared.inspection.run.execution.is_agent();
            execute_resume(*prepared, run_id, human).await
        }
        ResumePreparation::ReconcileCompleted { store, .. } => {
            reconcile_terminal_by_id(&store, run_id)
        }
    }
}

fn prepare_resume(args: &Args, run_id: &str) -> Result<ResumePreparation, Box<dyn Error>> {
    let selected = args
        .project
        .as_deref()
        .ok_or("run resume requires a Project")?;
    let workspace = canonical_project(selected)?;
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let execution_guard = store.try_acquire_run_execution_guard()?;
    let service = RunService::new(store.clone());
    let inspection = service.inspect_run(run_id)?;
    let action = validate_resume_selection(&store, run_id, &workspace, &inspection)?;
    let agent_workspace = agent_workspace::open(inspection.run.execution.is_agent(), &workspace)?;
    agent_workspace::validate_execution(&inspection.run.execution, agent_workspace.as_ref())?;
    if matches!(action, ResumeAction::ReconcileCompleted) {
        return Ok(ResumePreparation::ReconcileCompleted {
            store,
            _execution_guard: execution_guard,
        });
    }
    let prompt = persisted_resume_prompt(&inspection)?;
    let history = ConversationHistoryBridge::new(store.clone()).load_before(
        &inspection.run.conversation_id,
        &inspection.run.prompt_id,
        MAX_HISTORY_CONTENT_BYTES,
    )?;
    let provider = run_provider::for_execution(&inspection.run.execution)?;
    let (tools, allowed_capabilities) =
        agent_workspace::runtime_tools(&inspection.run.execution, agent_workspace.as_ref())?;
    let workspace_factory =
        agent_workspace::runtime_factory(&inspection.run.execution, agent_workspace)?;
    let runtime = AgentRuntime::new(provider, tools, workspace_factory);
    let request = RunRequest {
        session_id: inspection.run.conversation_id.clone(),
        run_id: inspection.run.run_id.clone(),
        prompt,
        system_prompt: inspection.run.execution.system_prompt.clone(),
        workspace,
        allowed_capabilities,
        limits: inspection.run.execution.limits.clone(),
    };
    Ok(ResumePreparation::Execute(Box::new(PreparedResume {
        _execution_guard: execution_guard,
        store,
        runtime,
        request,
        inspection,
        history,
    })))
}

fn validate_resume_selection(
    store: &Arc<SqliteHubStore>,
    run_id: &str,
    workspace: &std::path::Path,
    inspection: &RunInspection,
) -> Result<ResumeAction, Box<dyn Error>> {
    validate_project_binding(store, run_id, workspace, inspection)?;
    match &inspection.recovery.state {
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { .. },
        } => Ok(ResumeAction::ReconcileCompleted),
        RunRecoveryState::Terminal { .. } => {
            Err(format!("Run {run_id} is already terminal; resume is not applicable").into())
        }
        RunRecoveryState::PendingTool { calls } => {
            let name = calls.first().map_or("unknown", |call| call.name.as_str());
            Err(format!(
                "Run {run_id} has a pending tool effect ({name}); resume refuses automatic replay"
            )
            .into())
        }
        RunRecoveryState::Incomplete => Ok(ResumeAction::Execute),
    }
}

async fn execute_runtime(
    runtime: &AgentRuntime,
    request: RunRequest,
    prepared: &PreparedRun,
    human_output: bool,
    handle_ctrl_c: bool,
) -> Result<RunResult, RuntimeError> {
    let cancellation = Cancellation::default();
    let listener = handle_ctrl_c.then(|| cancellation_listener(cancellation.clone()));
    let stdout = io::stdout();
    let result = if human_output {
        let mut downstream = HumanEventSink::new(stdout.lock());
        let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
        runtime
            .run_with_history(request, prepared.history.clone(), cancellation, &mut sink)
            .await
    } else {
        let mut downstream = JsonlEventSink::new(stdout.lock());
        let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
        runtime
            .run_with_history(request, prepared.history.clone(), cancellation, &mut sink)
            .await
    };
    if let Some(listener) = listener {
        listener.abort();
    }
    result
}

fn prepare_setup(args: &Args, options: &StartOptions<'_>) -> Result<RunSetup, Box<dyn Error>> {
    let selected = args
        .project
        .as_deref()
        .ok_or("run start requires a Project")?;
    let workspace = canonical_project(selected)?;
    let agent_workspace =
        agent_workspace::open(matches!(options.mode, StartMode::Agent(_)), &workspace)?;
    let execution = execution_for_opened(options, agent_workspace.as_ref())?;
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let project = HubService::new(store.clone()).open_project(&workspace)?;
    let begin = begin_request(
        args,
        options.conversation_id,
        options.prompt_id,
        &project.id,
        execution,
    );
    Ok(RunSetup {
        store,
        workspace,
        agent_workspace,
        begin,
    })
}

fn load_history(setup: &RunSetup) -> Result<ConversationHistory, Box<dyn Error>> {
    Ok(
        ConversationHistoryBridge::new(setup.store.clone()).load_before(
            &setup.begin.conversation_id,
            &setup.begin.prompt_id,
            MAX_HISTORY_CONTENT_BYTES,
        )?,
    )
}

fn begin_prepared(
    setup: RunSetup,
    history: ConversationHistory,
) -> Result<PreparedRun, Box<dyn Error>> {
    let result = RunService::new(setup.store.clone()).begin_run(&setup.begin)?;
    Ok(PreparedRun {
        store: setup.store,
        workspace: setup.workspace,
        agent_workspace: setup.agent_workspace,
        run: result.run,
        prompt: result.prompt.content,
        history,
        disposition: result.disposition,
    })
}

fn begin_request(
    args: &Args,
    conversation_id: &str,
    prompt_id: &str,
    project_id: &str,
    execution: RunExecution,
) -> BeginRun {
    BeginRun {
        v: RUN_STORE_VERSION,
        run_id: unique_id("run"),
        conversation_id: conversation_id.into(),
        prompt_id: prompt_id.into(),
        project_id: project_id.into(),
        execution,
        idempotency_key: args
            .idempotency_key
            .clone()
            .unwrap_or_else(|| idempotency_key("run")),
        created_at_ms: unix_time_millis(),
    }
}

pub(crate) fn execution_for_opened(
    options: &StartOptions<'_>,
    agent_workspace: Option<&CapStdAgentWorkspace>,
) -> Result<RunExecution, RuntimeError> {
    #[cfg(not(unix))]
    agent_workspace::ensure_dev_supported(matches!(
        options.mode,
        StartMode::Agent(AgentAccess::Dev)
    ))?;
    let allowed_read_paths = configured_read_paths(options);
    let workspace_read_enabled = !allowed_read_paths.is_empty();
    let workspace_identity = match options.mode {
        StartMode::Agent(_) => Some(agent_workspace::identity(agent_workspace)?.clone()),
        _ => None,
    };
    Ok(RunExecution {
        provider: match options.mode {
            StartMode::Agent(access) => RunProvider::OpenAiAgent {
                endpoint: run_provider::endpoint(),
                model: options.model.unwrap_or(run_provider::DEFAULT_MODEL).into(),
                dev: access == AgentAccess::Dev,
                toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
                workspace_identity,
            },
            StartMode::Live => RunProvider::OpenAiResponses {
                endpoint: run_provider::endpoint(),
                model: options.model.unwrap_or(run_provider::DEFAULT_MODEL).into(),
            },
            StartMode::Deterministic => RunProvider::DeterministicRead {
                path: options.read_path.into(),
            },
        },
        system_prompt: system_prompt(options, workspace_read_enabled).into(),
        allowed_read_paths,
        limits: run_limits(options),
    })
}

fn system_prompt(options: &StartOptions<'_>, workspace_read_enabled: bool) -> &'static str {
    match options.mode {
        StartMode::Agent(AgentAccess::Dev) => AGENT_SYSTEM_PROMPT,
        StartMode::Agent(AgentAccess::ReadOnly) => READ_ONLY_AGENT_SYSTEM_PROMPT,
        _ if workspace_read_enabled => READ_SYSTEM_PROMPT,
        _ => NO_TOOL_SYSTEM_PROMPT,
    }
}

fn run_limits(options: &StartOptions<'_>) -> RunLimits {
    let is_agent = matches!(options.mode, StartMode::Agent(_));
    if is_agent {
        return agent_run_limits::bounded(
            options.max_turns,
            options.max_tool_calls,
            options.max_output_tokens,
        );
    }
    RunLimits {
        max_turns: options.max_turns,
        max_tool_calls: options.max_tool_calls,
        max_tool_output_bytes: 64 * 1024,
        max_model_output_bytes: 64 * 1024,
        max_model_events: 4_096,
        max_output_tokens_per_turn: options.max_output_tokens,
    }
}

fn configured_read_paths(options: &StartOptions<'_>) -> Vec<String> {
    match options.mode {
        StartMode::Agent(_) => Vec::new(),
        StartMode::Live => options.allowed_read_paths.to_vec(),
        StartMode::Deterministic => vec![options.read_path.to_owned()],
    }
}

fn runtime_request(prepared: &PreparedRun, allowed_capabilities: Vec<Capability>) -> RunRequest {
    RunRequest {
        session_id: prepared.run.conversation_id.clone(),
        run_id: prepared.run.run_id.clone(),
        prompt: prepared.prompt.clone(),
        system_prompt: prepared.run.execution.system_prompt.clone(),
        workspace: prepared.workspace.clone(),
        allowed_capabilities,
        limits: prepared.run.execution.limits.clone(),
    }
}

fn reconcile_terminal(prepared: &PreparedRun) -> Result<RunOutcome, Box<dyn Error>> {
    reconcile_terminal_by_id(&prepared.store, &prepared.run.run_id)
}

fn reconcile_terminal_by_id(
    store: &Arc<SqliteHubStore>,
    run_id: &str,
) -> Result<RunOutcome, Box<dyn Error>> {
    let service = RunService::new(store.clone());
    let inspection = service.inspect_run(run_id)?;
    match inspection.recovery.state {
        RunRecoveryState::Terminal { outcome } => {
            if matches!(&outcome, RunOutcome::Completed { .. }) {
                service.reconcile_completed_assistant(run_id)?;
            }
            Ok(outcome)
        }
        RunRecoveryState::Incomplete => {
            Err(format!("Run {run_id} is incomplete; explicit run resume is required").into())
        }
        RunRecoveryState::PendingTool { calls } => Err(format!(
            "Run {run_id} is blocked on {} pending tool call(s); automatic replay is disabled",
            calls.len()
        )
        .into()),
    }
}

fn persisted_resume_prompt(
    inspection: &crate::runtime_domain::RunInspection,
) -> Result<String, Box<dyn Error>> {
    let Some(first) = inspection.events.first() else {
        return Err(Box::new(RuntimeError::ResumeWithoutJournal));
    };
    match &first.kind {
        crate::runtime_domain::RuntimeEventKind::RunStarted { prompt } => Ok(prompt.clone()),
        _ => Err(Box::new(RuntimeError::ResumeWithoutJournal)),
    }
}

#[cfg(test)]
#[path = "run_command_tests.rs"]
mod tests;
