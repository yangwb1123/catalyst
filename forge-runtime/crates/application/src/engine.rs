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
    output_limit::truncate_output,
    run_state::{
        AssistantAction, RunState, classify_assistant_turn, finish_without_tools, limit_outcome,
        pre_turn_outcome,
    },
};

pub struct AgentRuntime {
    provider: Arc<dyn ModelProvider>,
    tools: ToolCatalog,
    workspace_factory: Arc<dyn WorkspaceReadFactory>,
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
        let mut state = RunState::new(history, request.prompt.clone());
        loop {
            if let Some(outcome) = pre_turn_outcome(request, cancellation, &state) {
                return Ok(state.result(outcome));
            }
            let turn = match self.next_turn(request, cancellation, &state, emitter).await {
                Ok(turn) => turn,
                Err(RuntimeError::Cancelled) => return Ok(state.result(RunOutcome::Cancelled)),
                Err(error) => return Err(error),
            };
            let action = Self::commit_assistant(turn, &mut state, emitter)?;
            if Self::tool_call_limit_reached(&action, request, &mut state, emitter)? {
                return Ok(state.result(limit_outcome(LimitKind::ToolCalls)));
            }
            let calls = match action {
                AssistantAction::Finish => return finish_without_tools(state),
                AssistantAction::Limit(kind) => {
                    return Ok(state.result(limit_outcome(kind)));
                }
                AssistantAction::Execute(calls) => calls,
                AssistantAction::Reject(calls, code) => {
                    reject_calls(
                        &calls,
                        code,
                        request.limits.max_tool_output_bytes,
                        &mut state,
                        emitter,
                    )?;
                    continue;
                }
            };
            if self
                .execute_calls(request, workspace, cancellation, calls, &mut state, emitter)
                .await?
            {
                return Ok(state.result(RunOutcome::Cancelled));
            }
        }
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
    ) -> Result<crate::model_turn::ModelTurn, RuntimeError> {
        let turn = state.turns.saturating_add(1);
        emitter.emit(RuntimeEventKind::TurnStarted { turn })?;
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

fn reject_calls(
    calls: &[ToolCall],
    code: &str,
    max_output_bytes: usize,
    state: &mut RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<(), RuntimeError> {
    for call in calls {
        let message = "tool call was not executed";
        emitter.emit(RuntimeEventKind::ToolRejected {
            call: call.clone(),
            code: code.into(),
            message: message.into(),
        })?;
        let output = truncate_output(
            ToolOutput {
                content: format!("{code}: {message}"),
                truncated: false,
            },
            max_output_bytes,
        );
        commit_tool_message(
            call.clone(),
            output.content,
            true,
            output.truncated,
            state,
            emitter,
        )?;
    }
    Ok(())
}

fn commit_tool_result(
    call: ToolCall,
    result: Result<ToolOutput, (String, String)>,
    max_output_bytes: usize,
    state: &mut RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<(), RuntimeError> {
    let (output, is_error) = match result {
        Ok(output) => (output, false),
        Err((code, message)) => (
            ToolOutput {
                content: format!("{code}: {message}"),
                truncated: false,
            },
            true,
        ),
    };
    let output = truncate_output(output, max_output_bytes);
    emitter.emit(RuntimeEventKind::ToolFinished {
        call_id: call.id.clone(),
        name: call.name.clone(),
        output: output.content.clone(),
        is_error,
        truncated: output.truncated,
    })?;
    commit_tool_message(
        call,
        output.content,
        is_error,
        output.truncated,
        state,
        emitter,
    )
}

fn commit_tool_message(
    call: ToolCall,
    output: String,
    is_error: bool,
    truncated: bool,
    state: &mut RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<(), RuntimeError> {
    let message = Message::Tool {
        call_id: call.id,
        name: call.name,
        output,
        is_error,
        truncated,
    };
    state.messages.push(message.clone());
    emitter.emit(RuntimeEventKind::MessageCommitted { message })
}
