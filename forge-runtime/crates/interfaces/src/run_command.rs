use std::{env, error::Error, io, path::PathBuf, sync::Arc};

use forge_runtime_application::{
    AgentRuntime, ConversationHistory, ConversationHistoryBridge, HubService, RunService,
    RuntimeError, ToolCatalog,
};
use forge_runtime_infrastructure::{
    CapStdWorkspaceFactory, DurableFirstEventSink, JsonlEventSink, OpenAiResponsesProvider,
    ReadFileTool, ReadThenAnswerProvider, SqliteHubStore,
};

use crate::{
    args::Args,
    run_selection::validate_project_binding,
    runtime_domain::{
        BeginRun, BeginRunDisposition, Cancellation, Capability, ModelProvider, RUN_STORE_VERSION,
        RunExecution, RunInspection, RunLimits, RunOutcome, RunProvider, RunRecord,
        RunRecoveryState, RunRequest, RunResult,
    },
    state_path::{
        canonical_project, hub_database_path, idempotency_key, unique_id, unix_time_millis,
    },
};

struct PreparedRun {
    store: Arc<SqliteHubStore>,
    workspace: PathBuf,
    run: RunRecord,
    prompt: String,
    history: ConversationHistory,
    disposition: BeginRunDisposition,
}

struct RunSetup {
    store: Arc<SqliteHubStore>,
    workspace: PathBuf,
    begin: BeginRun,
}

struct PreparedResume {
    store: Arc<SqliteHubStore>,
    runtime: AgentRuntime,
    request: RunRequest,
    inspection: RunInspection,
    history: ConversationHistory,
}

enum ResumePreparation {
    Execute(Box<PreparedResume>),
    ReconcileCompleted { store: Arc<SqliteHubStore> },
}

enum ResumeAction {
    Execute,
    ReconcileCompleted,
}

const MAX_HISTORY_CONTENT_BYTES: usize = 512 * 1024;
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const OPENAI_BASE_URL_ENV: &str = "OPENAI_BASE_URL";

/// `effective_openai_base_url` honours an explicit `OPENAI_BASE_URL` opt-in for a
/// self-hosted `/v1` gateway (`LiteLLM`/`Ollama`), falling back to the official
/// endpoint. The env value is validated downstream by the provider's
/// endpoint policy (https anywhere, http loopback-only).
fn effective_openai_base_url() -> String {
    env::var(OPENAI_BASE_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| OPENAI_BASE_URL.to_owned())
}
const DEFAULT_OPENAI_MODEL: &str = "gpt-5.6-sol";
const READ_SYSTEM_PROMPT: &str = "Use only the available read-only tools to answer the user.";
const NO_TOOL_SYSTEM_PROMPT: &str =
    "Answer the user without tools. No workspace access is available.";

pub struct StartOptions<'a> {
    pub conversation_id: &'a str,
    pub prompt_id: &'a str,
    pub read_path: &'a str,
    pub allowed_read_paths: &'a [String],
    pub live: bool,
    pub model: Option<&'a str>,
    pub max_output_tokens: u32,
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
    let provider = provider_for(&setup.begin.execution)?;
    let history = load_history(&setup)?;
    let prepared = begin_prepared(setup, history)?;
    if prepared.disposition == BeginRunDisposition::Replayed {
        return reconcile_terminal(&prepared);
    }
    let (tools, allowed_capabilities) = runtime_tools(&prepared.run.execution.allowed_read_paths)?;
    let runtime = AgentRuntime::new(provider, tools, Arc::new(CapStdWorkspaceFactory));
    let request = runtime_request(&prepared, allowed_capabilities);
    let result = execute_runtime(&runtime, request, &prepared).await;
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
        ResumePreparation::Execute(prepared) => execute_resume(*prepared, run_id).await,
        ResumePreparation::ReconcileCompleted { store } => reconcile_terminal_by_id(&store, run_id),
    }
}

async fn execute_resume(
    prepared: PreparedResume,
    run_id: &str,
) -> Result<RunOutcome, Box<dyn Error>> {
    let stdout = io::stdout();
    let mut downstream = JsonlEventSink::new(stdout.lock());
    let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
    let result = prepared
        .runtime
        .resume_with_inspection(
            prepared.request,
            prepared.inspection,
            prepared.history,
            Cancellation::default(),
            &mut sink,
        )
        .await;
    match result {
        Ok(_) => reconcile_terminal_by_id(&prepared.store, run_id),
        Err(error) => match reconcile_terminal_by_id(&prepared.store, run_id) {
            Ok(_) => Err(Box::new(error)),
            Err(reconcile_error) => Err(format!(
                "runtime failed: {error}; durable reconciliation failed: {reconcile_error}"
            )
            .into()),
        },
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
    let service = RunService::new(store.clone());
    let inspection = service.inspect_run(run_id)?;
    let action = validate_resume_selection(&store, run_id, &workspace, &inspection)?;
    if matches!(action, ResumeAction::ReconcileCompleted) {
        return Ok(ResumePreparation::ReconcileCompleted { store });
    }
    let prompt = persisted_resume_prompt(&inspection)?;
    let history = ConversationHistoryBridge::new(store.clone()).load_before(
        &inspection.run.conversation_id,
        &inspection.run.prompt_id,
        MAX_HISTORY_CONTENT_BYTES,
    )?;
    let provider = provider_for(&inspection.run.execution)?;
    let (tools, allowed_capabilities) =
        runtime_tools(&inspection.run.execution.allowed_read_paths)?;
    let runtime = AgentRuntime::new(provider, tools, Arc::new(CapStdWorkspaceFactory));
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
) -> Result<RunResult, RuntimeError> {
    let stdout = io::stdout();
    let mut downstream = JsonlEventSink::new(stdout.lock());
    let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
    runtime
        .run_with_history(
            request,
            prepared.history.clone(),
            Cancellation::default(),
            &mut sink,
        )
        .await
}

fn prepare_setup(args: &Args, options: &StartOptions<'_>) -> Result<RunSetup, Box<dyn Error>> {
    let selected = args
        .project
        .as_deref()
        .ok_or("run start requires a Project")?;
    let workspace = canonical_project(selected)?;
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let project = HubService::new(store.clone()).open_project(&workspace)?;
    let begin = begin_request(
        args,
        options.conversation_id,
        options.prompt_id,
        &project.id,
        execution_for(options),
    );
    Ok(RunSetup {
        store,
        workspace,
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

fn execution_for(options: &StartOptions<'_>) -> RunExecution {
    let allowed_read_paths = configured_read_paths(options);
    let workspace_read_enabled = !allowed_read_paths.is_empty();
    RunExecution {
        provider: if options.live {
            RunProvider::OpenAiResponses {
                endpoint: effective_openai_base_url(),
                model: options.model.unwrap_or(DEFAULT_OPENAI_MODEL).into(),
            }
        } else {
            RunProvider::DeterministicRead {
                path: options.read_path.into(),
            }
        },
        system_prompt: system_prompt(workspace_read_enabled).into(),
        allowed_read_paths,
        limits: run_limits(options.max_output_tokens, workspace_read_enabled),
    }
}

fn system_prompt(workspace_read_enabled: bool) -> &'static str {
    if workspace_read_enabled {
        READ_SYSTEM_PROMPT
    } else {
        NO_TOOL_SYSTEM_PROMPT
    }
}

fn run_limits(max_output_tokens: u32, workspace_read_enabled: bool) -> RunLimits {
    RunLimits {
        max_turns: 4,
        max_tool_calls: if workspace_read_enabled { 4 } else { 0 },
        max_tool_output_bytes: 64 * 1024,
        max_model_output_bytes: 64 * 1024,
        max_model_events: 4_096,
        max_output_tokens_per_turn: max_output_tokens,
    }
}

fn configured_read_paths(options: &StartOptions<'_>) -> Vec<String> {
    if options.live {
        options.allowed_read_paths.to_vec()
    } else {
        vec![options.read_path.to_owned()]
    }
}

fn runtime_tools(
    allowed_read_paths: &[String],
) -> Result<(ToolCatalog, Vec<Capability>), RuntimeError> {
    let mut tools = ToolCatalog::default();
    if allowed_read_paths.is_empty() {
        return Ok((tools, Vec::new()));
    }
    tools.register(Arc::new(ReadFileTool::restricted(
        allowed_read_paths.iter().cloned(),
    )))?;
    Ok((tools, vec![Capability::WorkspaceRead]))
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

fn provider_for(execution: &RunExecution) -> Result<Arc<dyn ModelProvider>, Box<dyn Error>> {
    match &execution.provider {
        RunProvider::DeterministicRead { path } => Ok(Arc::new(ReadThenAnswerProvider::new(path))),
        RunProvider::OpenAiResponses { endpoint, model } => {
            let api_key = env::var("OPENAI_API_KEY")
                .map_err(|_| "OPENAI_API_KEY is required for explicit --live execution")?;
            if api_key.trim().is_empty() {
                return Err("OPENAI_API_KEY must not be empty for --live execution".into());
            }
            let provider = if endpoint == OPENAI_BASE_URL {
                OpenAiResponsesProvider::new(endpoint, model, api_key)?
            } else {
                OpenAiResponsesProvider::new_self_hosted(endpoint, model, api_key)?
            };
            Ok(Arc::new(provider))
        }
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
mod tests {
    use super::{StartOptions, configured_read_paths, runtime_tools};
    use crate::runtime_domain::Capability;

    fn live_options(allowed_read_paths: &[String]) -> StartOptions<'_> {
        StartOptions {
            conversation_id: "conversation",
            prompt_id: "prompt",
            read_path: "README.md",
            allowed_read_paths,
            live: true,
            model: None,
            max_output_tokens: 4_096,
        }
    }

    #[test]
    fn live_without_read_consent_exposes_no_tool_or_capability() {
        let options = live_options(&[]);
        let paths = configured_read_paths(&options);
        let (tools, capabilities) = runtime_tools(&paths).expect("runtime tools");

        assert!(paths.is_empty());
        assert!(tools.specs().is_empty());
        assert!(capabilities.is_empty());
    }

    #[test]
    fn explicit_live_read_consent_registers_only_workspace_read() {
        let allowed = vec!["src/lib.rs".to_owned(), ".env".to_owned()];
        let options = live_options(&allowed);
        let paths = configured_read_paths(&options);
        let (tools, capabilities) = runtime_tools(&paths).expect("runtime tools");

        assert_eq!(paths, allowed);
        assert_eq!(tools.specs().len(), 1);
        assert_eq!(capabilities, vec![Capability::WorkspaceRead]);
    }

    #[test]
    fn offline_read_is_scoped_to_its_deterministic_target() {
        let options = StartOptions {
            live: false,
            ..live_options(&[])
        };

        assert_eq!(configured_read_paths(&options), ["README.md"]);
    }
}
