use std::{error::Error, fmt, io, path::PathBuf, sync::Arc};

use forge_runtime_infrastructure::{
    CapStdAgentWorkspace, DurableFirstEventSink, JsonlEventSink, SqliteHubStore,
};

use super::{
    AgentRuntime, BeginRunDisposition, Cancellation, ConversationHistoryBridge, HumanEventSink,
    MAX_HISTORY_CONTENT_BYTES, PreparedRun, RunInspection, RunRecoveryState, RunResult, RunService,
    RuntimeError, StartMode, StartOptions, StartOutput, agent_workspace, cancellation_listener,
    execution_for_opened, reconcile_terminal, reconcile_terminal_by_id, run_provider,
    runtime_request,
};
use crate::runtime_domain::{
    BeginRunResult, EventSink, Message, PROTOCOL_VERSION, RuntimeEventKind,
};

#[derive(Debug)]
pub(crate) struct SeededAgentReplayError {
    code: &'static str,
}

impl SeededAgentReplayError {
    #[must_use]
    pub(crate) const fn code(&self) -> &'static str {
        self.code
    }
}

impl fmt::Display for SeededAgentReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code)
    }
}

impl Error for SeededAgentReplayError {}

pub(crate) async fn start_seeded_agent(
    options: StartOptions<'_>,
    seed: BeginRunResult,
    store: Arc<SqliteHubStore>,
    workspace: PathBuf,
    agent_workspace: CapStdAgentWorkspace,
) -> Result<crate::runtime_domain::RunOutcome, Box<dyn Error>> {
    validate_seed_selection(&options, &seed, &workspace, &agent_workspace)?;
    if seed.disposition != BeginRunDisposition::Created {
        let inspection = RunService::new(store.clone()).inspect_run(&seed.run.run_id)?;
        return reconcile_without_automatic_execution(
            &store,
            &seed.run.run_id,
            &inspection,
            options.output,
        );
    }
    start_created_seeded_agent(options, seed, store, workspace, agent_workspace).await
}

async fn start_created_seeded_agent(
    options: StartOptions<'_>,
    seed: BeginRunResult,
    store: Arc<SqliteHubStore>,
    workspace: PathBuf,
    agent_workspace: CapStdAgentWorkspace,
) -> Result<crate::runtime_domain::RunOutcome, Box<dyn Error>> {
    let _execution_guard = store.try_acquire_run_execution_guard()?;
    let inspection = RunService::new(store.clone()).inspect_run(&seed.run.run_id)?;
    if !is_pristine_seed(&inspection, &seed) {
        return reconcile_without_automatic_execution(
            &store,
            &seed.run.run_id,
            &inspection,
            options.output,
        );
    }
    let provider = run_provider::for_execution(&seed.run.execution)?;
    let history = ConversationHistoryBridge::new(store.clone()).load_before(
        &seed.run.conversation_id,
        &seed.run.prompt_id,
        MAX_HISTORY_CONTENT_BYTES,
    )?;
    let prepared = PreparedRun {
        store,
        workspace,
        agent_workspace: Some(agent_workspace),
        run: seed.run,
        prompt: seed.prompt.content,
        history,
        disposition: seed.disposition,
    };
    let (tools, allowed_capabilities) =
        agent_workspace::runtime_tools(&prepared.run.execution, prepared.agent_workspace.as_ref())?;
    let workspace_factory = agent_workspace::runtime_factory(
        &prepared.run.execution,
        prepared.agent_workspace.clone(),
    )?;
    let runtime = AgentRuntime::new(provider, tools, workspace_factory);
    let request = runtime_request(&prepared, allowed_capabilities);
    let result = execute_seeded_resume(
        &runtime,
        request,
        inspection,
        &prepared,
        options.output == StartOutput::Human,
    )
    .await;
    reconcile_seed_result(&prepared, result)
}

fn reconcile_seed_result(
    prepared: &PreparedRun,
    result: Result<RunResult, RuntimeError>,
) -> Result<crate::runtime_domain::RunOutcome, Box<dyn Error>> {
    match result {
        Ok(_) => reconcile_terminal(prepared),
        Err(error) => {
            let _ = reconcile_terminal(prepared);
            Err(Box::new(error))
        }
    }
}

fn reconcile_without_automatic_execution(
    store: &Arc<SqliteHubStore>,
    run_id: &str,
    inspection: &RunInspection,
    output: StartOutput,
) -> Result<crate::runtime_domain::RunOutcome, Box<dyn Error>> {
    let code = match &inspection.recovery.state {
        RunRecoveryState::Terminal { .. } => {
            let outcome = reconcile_terminal_by_id(store, run_id)?;
            display_replayed_inspection(inspection, output)?;
            return Ok(outcome);
        }
        RunRecoveryState::Incomplete => "explicit_resume_required",
        RunRecoveryState::PendingTool { .. } => "pending_tool_effect",
    };
    Err(Box::new(SeededAgentReplayError { code }))
}

fn display_replayed_inspection(
    inspection: &RunInspection,
    output: StartOutput,
) -> Result<(), RuntimeError> {
    let stdout = io::stdout();
    match output {
        StartOutput::Human => replay_seed(inspection, &mut HumanEventSink::new(stdout.lock())),
        StartOutput::JsonLines => replay_seed(inspection, &mut JsonlEventSink::new(stdout.lock())),
    }
}

fn validate_seed_selection(
    options: &StartOptions<'_>,
    seed: &BeginRunResult,
    workspace: &std::path::Path,
    agent_workspace: &CapStdAgentWorkspace,
) -> Result<(), Box<dyn Error>> {
    let expected_execution = execution_for_opened(options, Some(agent_workspace))?;
    let matches = matches!(options.mode, StartMode::Agent(_))
        && workspace == agent_workspace.canonical_path()
        && options.conversation_id == seed.run.conversation_id
        && options.prompt_id == seed.run.prompt_id
        && expected_execution == seed.run.execution
        && seed.prompt.prompt_id == seed.run.prompt_id
        && seed.prompt.conversation_id == seed.run.conversation_id;
    matches
        .then_some(())
        .ok_or_else(|| "agent atomic seed disagrees with the requested execution".into())
}

fn is_pristine_seed(inspection: &RunInspection, seed: &BeginRunResult) -> bool {
    let [started, user] = inspection.events.as_slice() else {
        return false;
    };
    inspection.run == seed.run
        && matches!(inspection.recovery.state, RunRecoveryState::Incomplete)
        && started.v == PROTOCOL_VERSION
        && started.session_id == seed.run.conversation_id
        && started.run_id == seed.run.run_id
        && started.seq == 1
        && matches!(
            &started.kind,
            RuntimeEventKind::RunStarted { prompt } if prompt == &seed.prompt.content
        )
        && user.v == PROTOCOL_VERSION
        && user.session_id == seed.run.conversation_id
        && user.run_id == seed.run.run_id
        && user.seq == 2
        && user.emitted_at_ms == started.emitted_at_ms
        && matches!(
            &user.kind,
            RuntimeEventKind::MessageCommitted {
                message: Message::User { text }
            } if text == &seed.prompt.content
        )
}

async fn execute_seeded_resume(
    runtime: &AgentRuntime,
    request: crate::runtime_domain::RunRequest,
    inspection: RunInspection,
    prepared: &PreparedRun,
    human_output: bool,
) -> Result<RunResult, RuntimeError> {
    let cancellation = Cancellation::default();
    let listener = cancellation_listener(cancellation.clone());
    let stdout = io::stdout();
    let result = if human_output {
        let mut downstream = HumanEventSink::new(stdout.lock());
        let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
        replay_seed(&inspection, &mut sink)?;
        runtime
            .resume_with_inspection(
                request,
                inspection,
                prepared.history.clone(),
                cancellation,
                &mut sink,
            )
            .await
    } else {
        let mut downstream = JsonlEventSink::new(stdout.lock());
        let mut sink = DurableFirstEventSink::new(prepared.store.as_ref(), &mut downstream);
        replay_seed(&inspection, &mut sink)?;
        runtime
            .resume_with_inspection(
                request,
                inspection,
                prepared.history.clone(),
                cancellation,
                &mut sink,
            )
            .await
    };
    listener.abort();
    result
}

fn replay_seed(inspection: &RunInspection, sink: &mut dyn EventSink) -> Result<(), RuntimeError> {
    for event in &inspection.events {
        sink.emit(event)?;
    }
    Ok(())
}
