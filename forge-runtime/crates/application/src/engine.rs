use std::sync::Arc;

use forge_runtime_domain::{
    Cancellation, EventSink, LimitKind, Message, ModelFinishReason, ModelProvider, ModelRequest,
    RunOutcome, RunRequest, RunResult, RuntimeEventKind, ToolCall, ToolContext, ToolOutput, Usage,
    WorkspaceReadCapability, WorkspaceReadFactory,
};

use crate::{RuntimeError, ToolCatalog, emitter::EventEmitter, model_turn::collect_model_turn};

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
        let mut emitter =
            EventEmitter::new(sink, request.session_id.clone(), request.run_id.clone());
        Self::emit_run_start(&request, &mut emitter)?;
        let result = match self.workspace_factory.open(&request.workspace) {
            Ok(workspace) => {
                self.drive(&request, &workspace, &cancellation, &mut emitter)
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
        workspace: &WorkspaceReadCapability,
        cancellation: &Cancellation,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<RunResult, RuntimeError> {
        let mut state = RunState::new(request.prompt.clone());
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
            cancellation: cancellation.clone(),
        };
        collect_model_turn(self.provider.as_ref(), model_request, cancellation, emitter).await
    }

    fn commit_assistant(
        turn: crate::model_turn::ModelTurn,
        state: &mut RunState,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<AssistantAction, RuntimeError> {
        state.turns = state.turns.saturating_add(1);
        state.usage.add(turn.usage);
        let calls = turn.tool_calls;
        let action = classify_assistant_turn(turn.finish_reason, &calls)?;
        if matches!(action, AssistantAction::Limit(_)) {
            return Ok(action);
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

fn pre_turn_outcome(
    request: &RunRequest,
    cancellation: &Cancellation,
    state: &RunState,
) -> Option<RunOutcome> {
    if cancellation.is_cancelled() {
        Some(RunOutcome::Cancelled)
    } else if state.turns >= request.limits.max_turns {
        Some(limit_outcome(LimitKind::Turns))
    } else {
        None
    }
}

struct RunState {
    messages: Vec<Message>,
    usage: Usage,
    turns: u32,
    tool_calls: u32,
}

enum AssistantAction {
    Execute(Vec<ToolCall>),
    Reject(Vec<ToolCall>, &'static str),
    Finish,
    Limit(LimitKind),
}

impl AssistantAction {
    fn tool_calls(&self) -> &[ToolCall] {
        match self {
            Self::Execute(calls) | Self::Reject(calls, _) => calls,
            Self::Finish | Self::Limit(_) => &[],
        }
    }
}

impl RunState {
    fn new(prompt: String) -> Self {
        Self {
            messages: vec![Message::User { text: prompt }],
            usage: Usage::default(),
            turns: 0,
            tool_calls: 0,
        }
    }

    fn result(self, outcome: RunOutcome) -> RunResult {
        RunResult {
            outcome,
            messages: self.messages,
            usage: self.usage,
        }
    }

    fn charge_tool_calls(&mut self, count: usize, max: u32) -> bool {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        self.tool_calls = self.tool_calls.saturating_add(count);
        self.tool_calls > max
    }
}

fn finish_without_tools(state: RunState) -> Result<RunResult, RuntimeError> {
    let Some(Message::Assistant { text, .. }) = state.messages.last() else {
        return Err(RuntimeError::Protocol(
            "run ended without an assistant message".into(),
        ));
    };
    if text.trim().is_empty() {
        return Err(RuntimeError::Protocol(
            "assistant returned neither text nor tool calls".into(),
        ));
    }
    let answer = text.clone();
    Ok(state.result(RunOutcome::Completed { answer }))
}

fn classify_assistant_turn(
    finish_reason: ModelFinishReason,
    calls: &[ToolCall],
) -> Result<AssistantAction, RuntimeError> {
    match (finish_reason, calls.is_empty()) {
        (ModelFinishReason::Completed, true) => Ok(AssistantAction::Finish),
        (ModelFinishReason::ToolUse, false) => Ok(AssistantAction::Execute(calls.to_vec())),
        (ModelFinishReason::Length, true) => Ok(AssistantAction::Limit(LimitKind::ModelOutput)),
        (ModelFinishReason::Length, false) => Ok(AssistantAction::Reject(
            calls.to_vec(),
            "truncated_tool_call",
        )),
        (ModelFinishReason::Completed, false) => Err(RuntimeError::Protocol(
            "provider marked a turn completed while emitting tool calls".into(),
        )),
        (ModelFinishReason::ToolUse, true) => Err(RuntimeError::Protocol(
            "provider marked a turn as tool use without a tool call".into(),
        )),
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

fn truncate_output(mut output: ToolOutput, max_bytes: usize) -> ToolOutput {
    if output.content.len() <= max_bytes {
        return output;
    }
    let mut boundary = max_bytes.min(output.content.len());
    while !output.content.is_char_boundary(boundary) {
        boundary = boundary.saturating_sub(1);
    }
    output.content.truncate(boundary);
    output.truncated = true;
    output
}

fn limit_outcome(kind: LimitKind) -> RunOutcome {
    RunOutcome::LimitExceeded { kind }
}

#[cfg(test)]
mod tests {
    use super::truncate_output;

    #[test]
    fn truncation_sets_the_flag_and_preserves_a_utf8_boundary() {
        let output = truncate_output(
            forge_runtime_domain::ToolOutput {
                content: "ééé".into(),
                truncated: false,
            },
            3,
        );
        assert_eq!(output.content, "é");
        assert!(output.truncated);
    }
}
