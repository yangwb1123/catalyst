use std::{error::Error, fmt, io::Read, sync::Arc};

use crate::{
    args::{AgentArgs, AgentCommand, Args, MAX_PROMPT_BYTES, validate_prompt},
    run_command::{self, AgentAccess, StartMode, StartOptions, StartOutput},
    run_provider,
    runtime_application::{HubService, RunService, validate_idempotency_key},
    runtime_domain::{
        BeginRun, BeginRunDisposition, BeginRunResult, BeginRunWithPrompt, Conversation,
        ConversationScope, RUN_STORE_VERSION, RunExecution, RunOutcome, RunStoreError,
    },
    state_path::{hub_database_path, idempotency_key, unique_id, unix_time_millis},
};
use forge_runtime_infrastructure::{CapStdAgentWorkspace, SqliteHubStore};

#[path = "agent_start_output.rs"]
mod start_output;

const DEFAULT_SESSION_TITLE: &str = "Forge Dev Agent";

#[derive(Debug)]
pub(crate) struct AgentExecutionError {
    code: &'static str,
}

impl AgentExecutionError {
    #[must_use]
    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for AgentExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.code == "explicit_resume_required" {
            return formatter.write_str(
                "agent replay is incomplete; explicit run resume is required after inspection",
            );
        }
        if self.code == "execution_conflict" {
            return formatter.write_str(
                "another Run is executing for this Hub; inspect the announced durable Run and continue it with explicit run resume",
            );
        }
        write!(
            formatter,
            "agent execution failed ({}); inspect the durable Run with run list and run explain",
            self.code
        )
    }
}

impl Error for AgentExecutionError {}

struct PreparedPrompt {
    session_id: String,
    prompt_id: String,
}

#[derive(Clone, Copy)]
struct AgentRequest<'a> {
    prompt: &'a str,
    session: Option<&'a str>,
    model: Option<&'a str>,
    dev: bool,
    max_turns: u32,
    max_tool_calls: u32,
    max_output_tokens: u32,
}

pub async fn run(args: &Args, agent: &AgentArgs) -> Result<RunOutcome, Box<dyn Error>> {
    let request = agent_request(agent);
    validate_agent_preflight(args, &request)?;
    let prompt = resolve_prompt(request.prompt, &mut std::io::stdin().lock())?;
    let workspace = selected_workspace(args)?;
    execute(args, request, prompt, workspace).await
}

async fn execute(
    args: &Args,
    request: AgentRequest<'_>,
    prompt: String,
    agent_workspace: CapStdAgentWorkspace,
) -> Result<RunOutcome, Box<dyn Error>> {
    let project_path = agent_workspace.canonical_path().to_path_buf();
    let access = agent_access(request.dev);
    let execution = requested_execution(
        &agent_workspace,
        access,
        request.model,
        request.max_turns,
        request.max_tool_calls,
        request.max_output_tokens,
    )?;
    let (prepared, seed, store) =
        prepare_prompt(args, &prompt, request.session, &project_path, &execution)?;
    if seed.disposition == BeginRunDisposition::Created {
        start_output::announce(args, &prepared, &seed.run.run_id, request.dev);
    }
    run_command::start_seeded_agent(
        start_options(args, &prepared, request, access),
        seed,
        store,
        project_path,
        agent_workspace,
    )
    .await
    .map_err(stable_execution_error)
}

fn agent_request(agent: &AgentArgs) -> AgentRequest<'_> {
    let AgentCommand::Run {
        prompt,
        session,
        model,
        dev,
        max_turns,
        max_tool_calls,
        max_output_tokens,
    } = &agent.command;
    AgentRequest {
        prompt,
        session: session.as_deref(),
        model: model.as_deref(),
        dev: *dev,
        max_turns: *max_turns,
        max_tool_calls: *max_tool_calls,
        max_output_tokens: *max_output_tokens,
    }
}

fn validate_agent_preflight(args: &Args, request: &AgentRequest<'_>) -> Result<(), Box<dyn Error>> {
    if let Some(key) = args.idempotency_key.as_deref() {
        validate_idempotency_key(key)?;
    }
    run_provider::preflight_agent(request.model)
}

fn start_options<'a>(
    args: &Args,
    prepared: &'a PreparedPrompt,
    request: AgentRequest<'a>,
    access: AgentAccess,
) -> StartOptions<'a> {
    StartOptions {
        conversation_id: &prepared.session_id,
        prompt_id: &prepared.prompt_id,
        read_path: "README.md",
        allowed_read_paths: &[],
        mode: StartMode::Agent(access),
        model: request.model,
        max_output_tokens: request.max_output_tokens,
        output: if args.json {
            StartOutput::JsonLines
        } else {
            StartOutput::Human
        },
        max_turns: request.max_turns,
        max_tool_calls: request.max_tool_calls,
    }
}

fn stable_execution_error(error: Box<dyn Error>) -> Box<dyn Error> {
    let code = error
        .downcast_ref::<run_command::SeededAgentReplayError>()
        .map(run_command::SeededAgentReplayError::code)
        .or_else(|| {
            error
                .downcast_ref::<crate::runtime_application::RuntimeError>()
                .map(crate::runtime_application::RuntimeError::code)
        })
        .or_else(|| {
            error
                .downcast_ref::<RunStoreError>()
                .and_then(run_store_execution_code)
        })
        .unwrap_or("agent_execution_failed");
    drop(error);
    Box::new(AgentExecutionError { code })
}

fn run_store_execution_code(error: &RunStoreError) -> Option<&'static str> {
    matches!(error, RunStoreError::Conflict { .. }).then_some("execution_conflict")
}

fn resolve_prompt(argument: &str, input: &mut impl Read) -> Result<String, Box<dyn Error>> {
    if argument != "-" {
        return Ok(argument.into());
    }
    let limit = u64::try_from(MAX_PROMPT_BYTES.saturating_add(1)).unwrap_or(u64::MAX);
    let mut bytes = Vec::with_capacity(MAX_PROMPT_BYTES.min(8 * 1024));
    input.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_PROMPT_BYTES {
        return Err(format!("agent prompt may contain at most {MAX_PROMPT_BYTES} bytes").into());
    }
    let prompt = String::from_utf8(bytes).map_err(|_| "agent stdin prompt must be valid UTF-8")?;
    validate_prompt(&prompt).map_err(|error| -> Box<dyn Error> { error.into() })?;
    Ok(prompt)
}

fn agent_access(dev: bool) -> AgentAccess {
    if dev {
        AgentAccess::Dev
    } else {
        AgentAccess::ReadOnly
    }
}

fn prepare_prompt(
    args: &Args,
    prompt: &str,
    selected_session: Option<&str>,
    project_path: &std::path::Path,
    execution: &RunExecution,
) -> Result<(PreparedPrompt, BeginRunResult, Arc<SqliteHubStore>), Box<dyn Error>> {
    let database = hub_database_path(args.state_dir.as_deref())?;
    let store = Arc::new(SqliteHubStore::open(database)?);
    let service = HubService::new(store.clone());
    if let Some(key) = args.idempotency_key.as_deref()
        && let Some(prepared) = replay_existing_prompt(
            &service,
            &RunService::new(store.clone()),
            key,
            project_path,
            selected_session,
            prompt,
            execution,
        )?
    {
        let (prepared, seed) = prepared;
        return Ok((prepared, seed, store));
    }
    let (prepared, seed) = begin_new_prompt(
        args,
        prompt,
        selected_session,
        project_path,
        execution,
        &service,
        &store,
    )?;
    Ok((prepared, seed, store))
}

fn begin_new_prompt(
    args: &Args,
    prompt: &str,
    selected_session: Option<&str>,
    project_path: &std::path::Path,
    execution: &RunExecution,
    service: &HubService,
    store: &Arc<SqliteHubStore>,
) -> Result<(PreparedPrompt, BeginRunResult), Box<dyn Error>> {
    let project = service.open_project(project_path)?;
    let scope = ConversationScope::Project(project.id.clone());
    let session = resolve_session(service, &scope, selected_session, &project.id)?;
    let prompt_key = args
        .idempotency_key
        .clone()
        .unwrap_or_else(|| idempotency_key("agent-prompt"));
    let run_key = args
        .idempotency_key
        .clone()
        .unwrap_or_else(|| idempotency_key("run"));
    let seed = RunService::new(store.clone()).begin_run_with_prompt(&BeginRunWithPrompt {
        run: BeginRun {
            v: RUN_STORE_VERSION,
            run_id: unique_id("run"),
            conversation_id: session.id,
            prompt_id: unique_id("prompt"),
            project_id: project.id,
            execution: execution.clone(),
            idempotency_key: run_key,
            created_at_ms: unix_time_millis(),
        },
        prompt_content: prompt.into(),
        prompt_idempotency_key: prompt_key,
    })?;
    Ok(prepared_seed(seed))
}

#[allow(clippy::too_many_arguments)]
fn requested_execution(
    workspace: &CapStdAgentWorkspace,
    access: AgentAccess,
    model: Option<&str>,
    max_turns: u32,
    max_tool_calls: u32,
    max_output_tokens: u32,
) -> Result<RunExecution, Box<dyn Error>> {
    Ok(run_command::execution_for_opened(
        &StartOptions {
            conversation_id: "preflight",
            prompt_id: "preflight",
            read_path: "README.md",
            allowed_read_paths: &[],
            mode: StartMode::Agent(access),
            model,
            max_output_tokens,
            output: StartOutput::Human,
            max_turns,
            max_tool_calls,
        },
        Some(workspace),
    )?)
}

#[allow(clippy::too_many_arguments)]
fn replay_existing_prompt(
    hub: &HubService,
    runs: &RunService,
    key: &str,
    project_path: &std::path::Path,
    selected_session: Option<&str>,
    prompt: &str,
    execution: &RunExecution,
) -> Result<Option<(PreparedPrompt, BeginRunResult)>, Box<dyn Error>> {
    let Some(existing) = runs.find_run_by_idempotency_key(key)? else {
        return Ok(None);
    };
    validate_existing_project(hub, project_path, &existing.project_id)?;
    validate_existing_session(hub, &existing, selected_session)?;
    if existing.execution != *execution {
        return Err(idempotency_conflict("configuration"));
    }
    let replayed = runs.begin_run_with_prompt(&BeginRunWithPrompt {
        run: BeginRun {
            v: existing.v,
            run_id: existing.run_id,
            conversation_id: existing.conversation_id,
            prompt_id: existing.prompt_id,
            project_id: existing.project_id,
            execution: execution.clone(),
            idempotency_key: key.into(),
            created_at_ms: existing.created_at_ms,
        },
        prompt_content: prompt.into(),
        prompt_idempotency_key: key.into(),
    })?;
    Ok(Some(prepared_seed(replayed)))
}

fn prepared_seed(seed: BeginRunResult) -> (PreparedPrompt, BeginRunResult) {
    (
        PreparedPrompt {
            session_id: seed.run.conversation_id.clone(),
            prompt_id: seed.run.prompt_id.clone(),
        },
        seed,
    )
}

fn validate_existing_project(
    hub: &HubService,
    requested_path: &std::path::Path,
    existing_id: &str,
) -> Result<(), Box<dyn Error>> {
    let matches = hub
        .global_snapshot()?
        .projects
        .into_iter()
        .any(|project| project.id == existing_id && project.path == requested_path);
    matches
        .then_some(())
        .ok_or_else(|| idempotency_conflict("project"))
}

fn validate_existing_session(
    hub: &HubService,
    existing: &crate::runtime_domain::RunRecord,
    selected: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    if selected.is_some_and(|session| session != existing.conversation_id) {
        return Err(idempotency_conflict("session"));
    }
    let scope = ConversationScope::Project(existing.project_id.clone());
    let expected = hub.list_sessions(&scope)?.into_iter().any(|session| {
        session.id == existing.conversation_id
            && (selected.is_some() || session.title == DEFAULT_SESSION_TITLE)
    });
    expected
        .then_some(())
        .ok_or_else(|| idempotency_conflict("session"))
}

fn idempotency_conflict(field: &str) -> Box<dyn Error> {
    format!("agent idempotency conflict: {field} differs from the existing Run").into()
}

fn selected_workspace(args: &Args) -> Result<CapStdAgentWorkspace, Box<dyn Error>> {
    let path = args
        .project
        .as_deref()
        .ok_or("agent requires a selected Project")?;
    Ok(CapStdAgentWorkspace::open_selected(path)?)
}

fn resolve_session(
    service: &HubService,
    scope: &ConversationScope,
    selected: Option<&str>,
    project_id: &str,
) -> Result<Conversation, Box<dyn Error>> {
    if let Some(id) = selected {
        return service
            .list_sessions(scope)?
            .into_iter()
            .find(|session| session.id == id)
            .ok_or_else(|| {
                format!("session '{id}' does not belong to the selected Project").into()
            });
    }
    let key = format!("forge-dev-agent-session:{project_id}");
    Ok(service.create_session(scope, DEFAULT_SESSION_TITLE, &key)?)
}

#[cfg(test)]
mod tests {
    use super::resolve_prompt;
    use crate::args::MAX_PROMPT_BYTES;

    #[test]
    fn stdin_prompt_is_bounded_nonempty_utf8() {
        assert_eq!(
            resolve_prompt("-", &mut "sensitive\nrequest".as_bytes()).unwrap(),
            "sensitive\nrequest"
        );
        assert!(resolve_prompt("-", &mut "   ".as_bytes()).is_err());
        assert!(resolve_prompt("-", &mut [0xff].as_slice()).is_err());
        assert!(resolve_prompt("-", &mut vec![b'x'; MAX_PROMPT_BYTES + 1].as_slice()).is_err());
        assert_eq!(
            resolve_prompt("literal", &mut std::io::empty()).unwrap(),
            "literal"
        );
    }
}
