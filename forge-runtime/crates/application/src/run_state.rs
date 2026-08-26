use forge_runtime_domain::{
    Cancellation, LimitKind, Message, ModelFinishReason, RunOutcome, RunRequest, RunResult,
    ToolCall, Usage,
};

use crate::RuntimeError;

pub(crate) struct RunState {
    pub(crate) messages: Vec<Message>,
    pub(crate) usage: Usage,
    pub(crate) turns: u32,
    tool_calls: u32,
    model_output_bytes: usize,
    model_events: u32,
}

pub(crate) enum AssistantAction {
    Execute(Vec<ToolCall>),
    Reject(Vec<ToolCall>, &'static str),
    Finish,
    Limit(LimitKind),
}

impl AssistantAction {
    pub(crate) fn tool_calls(&self) -> &[ToolCall] {
        match self {
            Self::Execute(calls) | Self::Reject(calls, _) => calls,
            Self::Finish | Self::Limit(_) => &[],
        }
    }
}

impl RunState {
    pub(crate) fn new(mut messages: Vec<Message>, prompt: String) -> Self {
        messages.push(Message::User { text: prompt });
        Self::with_counters(messages, 0, 0, 0, 0)
    }

    pub(crate) fn from_replay(
        messages: Vec<Message>,
        turns: u32,
        tool_calls: u32,
        model_output_bytes: usize,
        model_events: u32,
    ) -> Self {
        Self::with_counters(
            messages,
            turns,
            tool_calls,
            model_output_bytes,
            model_events,
        )
    }

    fn with_counters(
        messages: Vec<Message>,
        turns: u32,
        tool_calls: u32,
        model_output_bytes: usize,
        model_events: u32,
    ) -> Self {
        Self {
            messages,
            usage: Usage::default(),
            turns,
            tool_calls,
            model_output_bytes,
            model_events,
        }
    }

    pub(crate) fn result(self, outcome: RunOutcome) -> RunResult {
        RunResult {
            outcome,
            messages: self.messages,
            usage: self.usage,
        }
    }

    pub(crate) fn charge_tool_calls(&mut self, count: usize, max: u32) -> bool {
        let count = u32::try_from(count).unwrap_or(u32::MAX);
        self.tool_calls = self.tool_calls.saturating_add(count);
        self.tool_calls > max
    }

    pub(crate) fn remaining_model_bytes(&self, request: &RunRequest) -> usize {
        request
            .limits
            .max_model_output_bytes
            .saturating_sub(self.model_output_bytes)
    }

    pub(crate) fn remaining_model_events(&self, request: &RunRequest) -> u32 {
        request
            .limits
            .max_model_events
            .saturating_sub(self.model_events)
    }

    pub(crate) fn charge_model_output(&mut self, bytes: usize, events: u32) {
        self.model_output_bytes = self.model_output_bytes.saturating_add(bytes);
        self.model_events = self.model_events.saturating_add(events);
    }
}

pub(crate) fn pre_turn_outcome(
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

pub(crate) fn finish_without_tools(state: RunState) -> Result<RunResult, RuntimeError> {
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

pub(crate) fn classify_assistant_turn(
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

pub(crate) fn limit_outcome(kind: LimitKind) -> RunOutcome {
    RunOutcome::LimitExceeded { kind }
}
