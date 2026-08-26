use forge_runtime_domain::{
    Cancellation, Capability, EventSink, LimitKind, Message, PROTOCOL_VERSION, RunExecution,
    RunInspection, RunOutcome, RunRecoveryState, RunRequest, RunResult, RunResumePoint,
    RunToolContinuation, RuntimeEvent, RuntimeEventKind, ToolCall, ToolOutput,
};

use super::{AgentRuntime, ResumeDriver, tool_events::reject_calls_with_message};
use crate::{
    ConversationHistory, RuntimeError, emitter::EventEmitter, output_limit::truncate_output,
    run_state::RunState,
};

struct ResumeDispatch {
    state: RunState,
    active_turn: bool,
    initial_calls: Option<Vec<ToolCall>>,
    immediate_outcome: Option<RunOutcome>,
}

impl AgentRuntime {
    /// Continues one explicitly requested, validated durable Run prefix.
    ///
    /// Already committed messages and tool effects are replayed into memory;
    /// no committed `tool_started` call is executed again. A prefix with an
    /// in-flight tool effect is refused before opening the workspace because
    /// its external outcome is unknowable.
    ///
    /// # Errors
    ///
    /// Returns an error when the prefix cannot be continued safely, the
    /// workspace/provider fails, or the durable sink rejects a new event.
    pub async fn resume_with_inspection(
        &self,
        request: RunRequest,
        inspection: RunInspection,
        history: ConversationHistory,
        cancellation: Cancellation,
        sink: &mut dyn EventSink,
    ) -> Result<RunResult, RuntimeError> {
        validate_resume_request(&request, &inspection)?;
        let (point, next_sequence) = resume_position(&inspection)?;
        let mut emitter = EventEmitter::resumed(
            sink,
            request.session_id.clone(),
            request.run_id.clone(),
            next_sequence,
        );
        let state = replay_state(&inspection.events, history.into_messages())?;
        reject_unsafe_resume_point(&point)?;
        if let Some(result) =
            finish_resume_terminal(&point, &inspection.recovery.state, &mut emitter)?
        {
            return Ok(state.result(result));
        }

        let workspace = self
            .workspace_factory
            .open(&request.workspace)
            .map_err(|error| RuntimeError::Workspace(error.to_string()))?;
        let dispatch = prepare_resume_dispatch(&point, &request, state, &mut emitter)?;
        if let Some(outcome) = dispatch.immediate_outcome {
            return Self::finish_run(Ok(dispatch.state.result(outcome)), &mut emitter);
        }
        let mut driver = ResumeDriver {
            request: &request,
            workspace: &workspace,
            cancellation: &cancellation,
            emitter: &mut emitter,
        };
        let result = self
            .resume_from_state(
                dispatch.state,
                &mut driver,
                dispatch.active_turn,
                dispatch.initial_calls,
            )
            .await;
        Self::finish_run(result, driver.emitter)
    }
}

fn resume_position(inspection: &RunInspection) -> Result<(RunResumePoint, u64), RuntimeError> {
    let point = inspection
        .resume_point()
        .map_err(|error| RuntimeError::Protocol(format!("invalid resume prefix: {error}")))?;
    Ok((point, next_resume_sequence(inspection)?))
}

fn reject_unsafe_resume_point(point: &RunResumePoint) -> Result<(), RuntimeError> {
    if let RunResumePoint::PendingTool { call } = point {
        return Err(RuntimeError::ResumePendingTool {
            name: call.name.clone(),
        });
    }
    Ok(())
}

fn finish_resume_terminal(
    point: &RunResumePoint,
    recovery: &RunRecoveryState,
    emitter: &mut EventEmitter<'_>,
) -> Result<Option<RunOutcome>, RuntimeError> {
    let RunResumePoint::Finish { outcome } = point else {
        return Ok(None);
    };
    if matches!(recovery, RunRecoveryState::Terminal { .. }) {
        return Err(RuntimeError::Protocol(
            "Run is already terminal; resume is not applicable".into(),
        ));
    }
    emitter.emit(RuntimeEventKind::RunFinished {
        outcome: outcome.clone(),
    })?;
    Ok(Some(outcome.clone()))
}

fn prepare_resume_dispatch(
    point: &RunResumePoint,
    request: &RunRequest,
    state: RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<ResumeDispatch, RuntimeError> {
    match point {
        RunResumePoint::Start | RunResumePoint::CommitUser { .. } => {
            prepare_user_resume(point, request, state, emitter)
        }
        RunResumePoint::StartTurn { .. } | RunResumePoint::ContinueTurn { .. } => {
            prepare_turn_resume(point, state)
        }
        RunResumePoint::ExecuteTools { calls } => Ok(ResumeDispatch {
            state,
            active_turn: false,
            initial_calls: Some(calls.clone()),
            immediate_outcome: None,
        }),
        RunResumePoint::RejectTools {
            calls,
            code,
            message,
        } => prepare_rejection_resume(calls, code, message, request, state, emitter),
        RunResumePoint::CommitToolMessage {
            message,
            continuation,
        } => prepare_tool_message_resume(message, continuation, request, state, emitter),
        RunResumePoint::PendingTool { .. } | RunResumePoint::Finish { .. } => {
            unreachable!("resume safety points handled before dispatch preparation")
        }
    }
}

fn prepare_rejection_resume(
    calls: &[ToolCall],
    code: &str,
    message: &str,
    request: &RunRequest,
    mut state: RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<ResumeDispatch, RuntimeError> {
    state.charge_tool_calls(calls.len(), request.limits.max_tool_calls);
    reject_calls_with_message(
        calls,
        code,
        message,
        request.limits.max_tool_output_bytes,
        &mut state,
        emitter,
    )?;
    Ok(ResumeDispatch {
        state,
        active_turn: false,
        initial_calls: None,
        immediate_outcome: rejection_outcome(code),
    })
}

fn rejection_outcome(code: &str) -> Option<RunOutcome> {
    match code {
        "tool_call_limit" => Some(RunOutcome::LimitExceeded {
            kind: LimitKind::ToolCalls,
        }),
        "cancelled" => Some(RunOutcome::Cancelled),
        _ => None,
    }
}

fn prepare_user_resume(
    point: &RunResumePoint,
    request: &RunRequest,
    mut state: RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<ResumeDispatch, RuntimeError> {
    let prompt = match point {
        RunResumePoint::Start => request.prompt.clone(),
        RunResumePoint::CommitUser { prompt } if prompt == &request.prompt => prompt.clone(),
        RunResumePoint::CommitUser { .. } => {
            return Err(RuntimeError::Protocol(
                "resume prompt disagrees with the durable Run prefix".into(),
            ));
        }
        _ => unreachable!("user resume helper received a different point"),
    };
    if matches!(point, RunResumePoint::Start) {
        emitter.emit(RuntimeEventKind::RunStarted {
            prompt: prompt.clone(),
        })?;
    }
    let message = Message::User { text: prompt };
    emitter.emit(RuntimeEventKind::MessageCommitted {
        message: message.clone(),
    })?;
    state.messages.push(message);
    Ok(ResumeDispatch {
        state,
        active_turn: false,
        initial_calls: None,
        immediate_outcome: None,
    })
}

fn prepare_turn_resume(
    point: &RunResumePoint,
    mut state: RunState,
) -> Result<ResumeDispatch, RuntimeError> {
    let active_turn = matches!(point, RunResumePoint::ContinueTurn { .. });
    if active_turn {
        // A durable turn_started admits the in-flight turn, but the in-memory
        // counter tracks completed turns. The current model response charges
        // this turn exactly once when it is committed.
        state.turns = state.turns.saturating_sub(1);
    }
    let expected = state.turns.saturating_add(1);
    let turn = match point {
        RunResumePoint::StartTurn { turn } | RunResumePoint::ContinueTurn { turn } => *turn,
        _ => unreachable!("turn resume helper received a different point"),
    };
    if turn != expected || (active_turn && turn == 0) {
        return Err(RuntimeError::Protocol(
            "resume turn number disagrees with the durable Run prefix".into(),
        ));
    }
    Ok(ResumeDispatch {
        state,
        active_turn,
        initial_calls: None,
        immediate_outcome: None,
    })
}

fn prepare_tool_message_resume(
    message: &Message,
    continuation: &RunToolContinuation,
    request: &RunRequest,
    mut state: RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<ResumeDispatch, RuntimeError> {
    let message = bounded_tool_message(message, request.limits.max_tool_output_bytes);
    state.messages.push(message.clone());
    emitter.emit(RuntimeEventKind::MessageCommitted {
        message: message.clone(),
    })?;
    dispatch_after_tool_message(continuation, request, state, emitter)
}

fn dispatch_after_tool_message(
    continuation: &RunToolContinuation,
    request: &RunRequest,
    state: RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<ResumeDispatch, RuntimeError> {
    match continuation {
        RunToolContinuation::ExecuteTools { calls } => Ok(ResumeDispatch {
            state,
            active_turn: false,
            initial_calls: Some(calls.clone()),
            immediate_outcome: None,
        }),
        RunToolContinuation::RejectTools {
            calls,
            code,
            message,
        } => prepare_rejection_resume(calls, code, message, request, state, emitter),
        RunToolContinuation::StartTurn { turn } => prepare_start_turn_after_tool(*turn, state),
        RunToolContinuation::Finish { outcome } => Ok(ResumeDispatch {
            state,
            active_turn: false,
            initial_calls: None,
            immediate_outcome: Some(outcome.clone()),
        }),
    }
}

fn prepare_start_turn_after_tool(
    turn: u32,
    state: RunState,
) -> Result<ResumeDispatch, RuntimeError> {
    if turn != state.turns.saturating_add(1) {
        return Err(RuntimeError::Protocol(
            "resume turn after Tool message disagrees with the durable Run prefix".into(),
        ));
    }
    Ok(ResumeDispatch {
        state,
        active_turn: false,
        initial_calls: None,
        immediate_outcome: None,
    })
}

fn bounded_tool_message(message: &Message, max_output_bytes: usize) -> Message {
    let Message::Tool {
        call_id,
        name,
        output,
        is_error,
        truncated,
    } = message
    else {
        return message.clone();
    };
    let output = truncate_output(
        ToolOutput {
            content: output.clone(),
            truncated: *truncated,
        },
        max_output_bytes,
    );
    Message::Tool {
        call_id: call_id.clone(),
        name: name.clone(),
        output: output.content,
        is_error: *is_error,
        truncated: output.truncated,
    }
}

fn validate_resume_request(
    request: &RunRequest,
    inspection: &RunInspection,
) -> Result<(), RuntimeError> {
    if request.session_id != inspection.run.conversation_id
        || request.run_id != inspection.run.run_id
    {
        return Err(RuntimeError::Protocol(
            "resume request is not bound to the inspected Run".into(),
        ));
    }
    if inspection.run.protocol_version != PROTOCOL_VERSION {
        return Err(RuntimeError::Protocol(
            "resume Run uses an unsupported event protocol".into(),
        ));
    }
    validate_execution_binding(request, &inspection.run.execution)?;
    let prompt = durable_resume_prompt(inspection)?;
    if request.prompt != prompt {
        return Err(RuntimeError::Protocol(
            "resume prompt disagrees with the durable Run prefix".into(),
        ));
    }
    Ok(())
}

fn validate_execution_binding(
    request: &RunRequest,
    execution: &RunExecution,
) -> Result<(), RuntimeError> {
    let capabilities = if execution.allowed_read_paths.is_empty() {
        Vec::new()
    } else {
        vec![Capability::WorkspaceRead]
    };
    if request.system_prompt != execution.system_prompt
        || request.limits != execution.limits
        || request.allowed_capabilities != capabilities
    {
        return Err(RuntimeError::Protocol(
            "resume request disagrees with the persisted execution configuration".into(),
        ));
    }
    Ok(())
}

fn durable_resume_prompt(inspection: &RunInspection) -> Result<&str, RuntimeError> {
    match inspection.events.first().map(|event| &event.kind) {
        Some(RuntimeEventKind::RunStarted { prompt }) => Ok(prompt),
        Some(_) => Err(RuntimeError::Protocol(
            "resume journal does not start with durable prompt evidence".into(),
        )),
        None => Err(RuntimeError::Protocol(
            "resume requires a non-empty durable Run prefix".into(),
        )),
    }
}

fn next_resume_sequence(inspection: &RunInspection) -> Result<u64, RuntimeError> {
    inspection.events.last().map_or(Ok(1), |event| {
        event
            .seq
            .checked_add(1)
            .ok_or_else(|| RuntimeError::Protocol("resume event sequence is exhausted".into()))
    })
}

#[derive(Default)]
struct ReplayCounters {
    turns: u32,
    tool_calls: u32,
    model_output_bytes: usize,
    model_events: u32,
}

fn replay_state(
    events: &[RuntimeEvent],
    mut messages: Vec<Message>,
) -> Result<RunState, RuntimeError> {
    let mut counters = ReplayCounters::default();
    for event in events {
        replay_event(event, &mut messages, &mut counters)?;
    }
    Ok(RunState::from_replay(
        messages,
        counters.turns,
        counters.tool_calls,
        counters.model_output_bytes,
        counters.model_events,
    ))
}

fn replay_event(
    event: &RuntimeEvent,
    messages: &mut Vec<Message>,
    counters: &mut ReplayCounters,
) -> Result<(), RuntimeError> {
    match &event.kind {
        RuntimeEventKind::TurnStarted { .. } => {
            counters.turns = counters.turns.saturating_add(1);
        }
        RuntimeEventKind::AssistantDelta { delta } => {
            counters.model_output_bytes = counters.model_output_bytes.saturating_add(delta.len());
            counters.model_events = counters.model_events.saturating_add(1);
        }
        RuntimeEventKind::MessageCommitted { message } => {
            replay_message(message, counters)?;
            messages.push(message.clone());
        }
        RuntimeEventKind::ToolStarted { .. } | RuntimeEventKind::ToolRejected { .. } => {
            counters.tool_calls = counters.tool_calls.saturating_add(1);
        }
        _ => {}
    }
    Ok(())
}

fn replay_message(message: &Message, counters: &mut ReplayCounters) -> Result<(), RuntimeError> {
    if let Message::ProviderContext { .. } = message {
        charge_replayed_bytes(
            message,
            &mut counters.model_output_bytes,
            &mut counters.model_events,
            "persisted provider context cannot be replayed",
        )?;
    }
    if let Message::Assistant { tool_calls, .. } = message {
        for call in tool_calls {
            charge_replayed_bytes(
                call,
                &mut counters.model_output_bytes,
                &mut counters.model_events,
                "persisted tool call cannot be replayed",
            )?;
        }
    }
    Ok(())
}

fn charge_replayed_bytes<T: serde::Serialize>(
    value: &T,
    bytes: &mut usize,
    events: &mut u32,
    error_message: &str,
) -> Result<(), RuntimeError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|error| RuntimeError::Protocol(format!("{error_message}: {error}")))?;
    *bytes = bytes.saturating_add(encoded.len());
    *events = events.saturating_add(1);
    Ok(())
}
