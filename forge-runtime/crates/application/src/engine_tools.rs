use forge_runtime_domain::{Message, RuntimeEventKind, ToolCall, ToolOutput};

use crate::{
    RuntimeError, emitter::EventEmitter, output_limit::truncate_output, run_state::RunState,
};

pub(super) fn reject_calls(
    calls: &[ToolCall],
    code: &str,
    max_output_bytes: usize,
    state: &mut RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<(), RuntimeError> {
    reject_calls_with_message(
        calls,
        code,
        "tool call was not executed",
        max_output_bytes,
        state,
        emitter,
    )
}

pub(super) fn reject_calls_with_message(
    calls: &[ToolCall],
    code: &str,
    message: &str,
    max_output_bytes: usize,
    state: &mut RunState,
    emitter: &mut EventEmitter<'_>,
) -> Result<(), RuntimeError> {
    for call in calls {
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

pub(super) fn commit_tool_result(
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
