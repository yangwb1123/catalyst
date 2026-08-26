use std::sync::Arc;

use forge_runtime_domain::{
    Cancellation, EventSink, LimitKind, Message, ModelProvider, ModelRequest, RunOutcome,
    RunRequest, RunResult, RuntimeEventKind, ToolCall, ToolContext, ToolOutput,
    WorkspaceReadCapability, WorkspaceReadFactory,
};

use crate::{
    ConversationHistory, RuntimeError, ToolCatalog,
    emitter::EventEmitter,
    model_turn::{ModelBudget, collect_model_turn},
    run_state::{
        AssistantAction, RunState, classify_assistant_turn, finish_without_tools, limit_outcome,
        pre_turn_outcome,
    },
};

#[path = "engine_resume.rs"]
mod resume;
#[path = "engine_tools.rs"]
mod tool_events;

use tool_events::{commit_tool_result, reject_calls};

pub struct AgentRuntime {
    provider: Arc<dyn ModelProvider>,
    tools: ToolCatalog,
    workspace_factory: Arc<dyn WorkspaceReadFactory>,
}

struct ResumeDriver<'request, 'borrow, 'sink> {
    request: &'request RunRequest,
    workspace: &'request WorkspaceReadCapability,
    cancellation: &'request Cancellation,
    emitter: &'borrow mut EventEmitter<'sink>,
}

impl AgentRuntime {
    #[must_use]
    pub fn new(
        provider: Arc<dyn ModelProvider>,
        tools: ToolCatalog,
        workspace_factory: Arc<dyn WorkspaceReadFactory>,
    ) -> Self {
        Self {
            provider,
            tools,
            workspace_factory,
        }
    }

    /// Runs one prompt until completion, cancellation, a limit, or a runtime error.
    ///
    /// # Errors
    ///
    /// Returns an error when the provider protocol, event sink, or runtime
    /// infrastructure fails. Normal limits and cancellation are returned as
    /// terminal outcomes.
    pub async fn run(
        &self,
        request: RunRequest,
        cancellation: Cancellation,
        sink: &mut dyn EventSink,
    ) -> Result<RunResult, RuntimeError> {
        self.run_with_history(request, ConversationHistory::default(), cancellation, sink)
            .await
    }

    /// Runs one prompt with a validated, bounded persisted Conversation history.
    ///
    /// # Errors
    ///
    /// Returns the same structured failures as `run`.
    pub async fn run_with_history(
        &self,
        request: RunRequest,
        history: ConversationHistory,
        cancellation: Cancellation,
        sink: &mut dyn EventSink,
    ) -> Result<RunResult, RuntimeError> {
        let mut emitter =
            EventEmitter::new(sink, request.session_id.clone(), request.run_id.clone());
        Self::emit_run_start(&request, &mut emitter)?;
        let result = match self.workspace_factory.open(&request.workspace) {
            Ok(workspace) => {
                self.drive(
                    &request,
                    history.into_messages(),
                    &workspace,
                    &cancellation,
                    &mut emitter,
                )
                .await
            }
            Err(error) => Err(RuntimeError::Workspace(error.to_string())),
        };
        Self::finish_run(result, &mut emitter)
    }

    fn emit_run_start(
        request: &RunRequest,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<(), RuntimeError> {
        emitter.emit(RuntimeEventKind::RunStarted {
            prompt: request.prompt.clone(),
        })?;
        emitter.emit(RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: request.prompt.clone(),
            },
        })
    }

    async fn drive(
        &self,
        request: &RunRequest,
        history: Vec<Message>,
        workspace: &WorkspaceReadCapability,
        cancellation: &Cancellation,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<RunResult, RuntimeError> {
        let state = RunState::new(history, request.prompt.clone());
        let mut driver = ResumeDriver {
            request,
            workspace,
            cancellation,
            emitter,
        };
        self.resume_from_state(state, &mut driver, false, None)
            .await
    }

    async fn resume_from_state(
        &self,
        mut state: RunState,
        driver: &mut ResumeDriver<'_, '_, '_>,
        active_turn: bool,
        initial_calls: Option<Vec<ToolCall>>,
    ) -> Result<RunResult, RuntimeError> {
        if let Some(calls) = initial_calls
            && let Some(outcome) = self
                .execute_initial_calls(&mut state, driver, calls)
                .await?
        {
            return Ok(state.result(outcome));
        }
        self.resume_loop(state, driver, active_turn).await
    }

    async fn execute_initial_calls(
        &self,
        state: &mut RunState,
        driver: &mut ResumeDriver<'_, '_, '_>,
        calls: Vec<ToolCall>,
    ) -> Result<Option<RunOutcome>, RuntimeError> {
        let action = AssistantAction::Execute(calls.clone());
        if Self::tool_call_limit_reached(&action, driver.request, state, driver.emitter)? {
            return Ok(Some(limit_outcome(LimitKind::ToolCalls)));
        }
        if self
            .execute_calls(
                driver.request,
                driver.workspace,
                driver.cancellation,
                calls,
                state,
                driver.emitter,
            )
            .await?
        {
            return Ok(Some(RunOutcome::Cancelled));
        }
        Ok(None)
    }

    async fn resume_loop(
        &self,
        mut state: RunState,
        driver: &mut ResumeDriver<'_, '_, '_>,
        mut active_turn: bool,
    ) -> Result<RunResult, RuntimeError> {
        loop {
            if !active_turn
                && let Some(outcome) = pre_turn_outcome(driver.request, driver.cancellation, &state)
            {
                return Ok(state.result(outcome));
            }
            let Some(turn) = self.next_resume_turn(&state, driver, !active_turn).await? else {
                return Ok(state.result(RunOutcome::Cancelled));
            };
            active_turn = false;
            let action = Self::commit_assistant(turn, &mut state, driver.emitter)?;
            if Self::tool_call_limit_reached(&action, driver.request, &mut state, driver.emitter)? {
                return Ok(state.result(limit_outcome(LimitKind::ToolCalls)));
            }
            if Self::reject_assistant_action(&action, driver.request, &mut state, driver.emitter)? {
                continue;
            }
            let calls = match action {
                AssistantAction::Finish => return finish_without_tools(state),
                AssistantAction::Limit(kind) => {
                    return Ok(state.result(limit_outcome(kind)));
                }
                AssistantAction::Execute(calls) => calls,
                AssistantAction::Reject(..) => unreachable!("rejected action was handled above"),
            };
            if self
                .execute_calls(
                    driver.request,
                    driver.workspace,
                    driver.cancellation,
                    calls,
                    &mut state,
                    driver.emitter,
                )
                .await?
            {
                return Ok(state.result(RunOutcome::Cancelled));
            }
        }
    }

    async fn next_resume_turn(
        &self,
        state: &RunState,
        driver: &mut ResumeDriver<'_, '_, '_>,
        emit_turn_started: bool,
    ) -> Result<Option<crate::model_turn::ModelTurn>, RuntimeError> {
        match self
            .next_turn(
                driver.request,
                driver.cancellation,
                state,
                driver.emitter,
                emit_turn_started,
            )
            .await
        {
            Ok(turn) => Ok(Some(turn)),
            Err(RuntimeError::Cancelled) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn reject_assistant_action(
        action: &AssistantAction,
        request: &RunRequest,
        state: &mut RunState,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<bool, RuntimeError> {
        let AssistantAction::Reject(calls, code) = action else {
            return Ok(false);
        };
        reject_calls(
            calls,
            code,
            request.limits.max_tool_output_bytes,
            state,
            emitter,
        )?;
        Ok(true)
    }

    fn tool_call_limit_reached(
        action: &AssistantAction,
        request: &RunRequest,
        state: &mut RunState,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<bool, RuntimeError> {
        let calls = action.tool_calls();
        if !state.charge_tool_calls(calls.len(), request.limits.max_tool_calls) {
            return Ok(false);
        }
        reject_calls(
            calls,
            "tool_call_limit",
            request.limits.max_tool_output_bytes,
            state,
            emitter,
        )?;
        Ok(true)
    }

    async fn next_turn(
        &self,
        request: &RunRequest,
        cancellation: &Cancellation,
        state: &RunState,
        emitter: &mut EventEmitter<'_>,
        emit_turn_started: bool,
    ) -> Result<crate::model_turn::ModelTurn, RuntimeError> {
        let turn = state.turns.saturating_add(1);
        if emit_turn_started {
            emitter.emit(RuntimeEventKind::TurnStarted { turn })?;
        }
        let model_request = ModelRequest {
            system_prompt: request.system_prompt.clone(),
            messages: state.messages.clone(),
            tools: self.tools.specs(),
            max_output_tokens: request.limits.max_output_tokens_per_turn,
            cancellation: cancellation.clone(),
        };
        let budget = ModelBudget {
            remaining_bytes: state.remaining_model_bytes(request),
            remaining_events: state.remaining_model_events(request),
        };
        collect_model_turn(
            self.provider.as_ref(),
            model_request,
            cancellation,
            budget,
            emitter,
        )
        .await
    }

    fn commit_assistant(
        turn: crate::model_turn::ModelTurn,
        state: &mut RunState,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<AssistantAction, RuntimeError> {
        state.turns = state.turns.saturating_add(1);
        state.usage.add(turn.usage);
        state.charge_model_output(turn.output_bytes, turn.output_events);
        let calls = turn.tool_calls;
        let action = classify_assistant_turn(turn.finish_reason, &calls)?;
        if matches!(action, AssistantAction::Limit(_)) {
            return Ok(action);
        }
        for message in turn.provider_context {
            state.messages.push(message.clone());
            emitter.emit(RuntimeEventKind::MessageCommitted { message })?;
        }
        let message = Message::Assistant {
            text: turn.text,
            tool_calls: calls,
        };
        state.messages.push(message.clone());
        emitter.emit(RuntimeEventKind::MessageCommitted { message })?;
        Ok(action)
    }

    async fn execute_calls(
        &self,
        request: &RunRequest,
        workspace: &WorkspaceReadCapability,
        cancellation: &Cancellation,
        calls: Vec<ToolCall>,
        state: &mut RunState,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<bool, RuntimeError> {
        let mut calls = calls.into_iter();
        while let Some(call) = calls.next() {
            if cancellation.is_cancelled() {
                let rejected: Vec<_> = std::iter::once(call).chain(calls).collect();
                reject_calls(
                    &rejected,
                    "cancelled",
                    request.limits.max_tool_output_bytes,
                    state,
                    emitter,
                )?;
                return Ok(true);
            }
            emitter.emit(RuntimeEventKind::ToolStarted { call: call.clone() })?;
            let result = self
                .execute_call(request, workspace, cancellation, &call)
                .await;
            commit_tool_result(
                call,
                result,
                request.limits.max_tool_output_bytes,
                state,
                emitter,
            )?;
        }
        Ok(cancellation.is_cancelled())
    }

    async fn execute_call(
        &self,
        request: &RunRequest,
        workspace: &WorkspaceReadCapability,
        cancellation: &Cancellation,
        call: &ToolCall,
    ) -> Result<ToolOutput, (String, String)> {
        let tool = self.tools.get(&call.name).ok_or_else(|| {
            (
                "unknown_tool".into(),
                format!("unknown tool '{}'", call.name),
            )
        })?;
        let spec = tool.spec();
        if !request.allowed_capabilities.contains(&spec.capability) {
            return Err((
                "capability_denied".into(),
                format!("capability {:?} was not granted", spec.capability),
            ));
        }
        let context = ToolContext {
            workspace: workspace.clone(),
            cancellation: cancellation.clone(),
            max_output_bytes: request.limits.max_tool_output_bytes,
        };
        let execution = tool.execute(call.arguments.clone(), context);
        let cancelled = Box::pin(cancellation.cancelled());
        match futures_util::future::select(execution, cancelled).await {
            futures_util::future::Either::Left((result, _)) => {
                result.map_err(|error| (error.code, error.message))
            }
            futures_util::future::Either::Right(((), _)) => Err((
                "cancelled".into(),
                "run cancelled during tool execution".into(),
            )),
        }
    }

    fn finish_run(
        result: Result<RunResult, RuntimeError>,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<RunResult, RuntimeError> {
        match result {
            Ok(result) => {
                emitter.emit(RuntimeEventKind::RunFinished {
                    outcome: result.outcome.clone(),
                })?;
                Ok(result)
            }
            Err(error @ RuntimeError::EventSink(_)) => Err(error),
            Err(error) => {
                let outcome = RunOutcome::Failed {
                    code: error.code().into(),
                    message: error.to_string(),
                };
                emitter.emit(RuntimeEventKind::RuntimeError {
                    code: error.code().into(),
                    message: error.to_string(),
                })?;
                emitter.emit(RuntimeEventKind::RunFinished { outcome })?;
                Err(error)
            }
        }
    }
}
