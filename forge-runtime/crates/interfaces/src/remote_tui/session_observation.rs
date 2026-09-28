use std::io::Write;

use serde_json::Value;

use super::state::{TuiState, io_error};
use super::{RemoteClient, RemoteError};

pub(super) async fn preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(request) = read_request(argument, writer)? else {
        return Ok(());
    };
    let request_object = request
        .as_object()
        .ok_or_else(|| RemoteError("remote session observation input is invalid".into()))?;
    let conversation_id = request_object
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote session observation conversation is invalid".into()))?;
    let run_id = request_object
        .get("run_id")
        .and_then(Value::as_str)
        .ok_or_else(|| RemoteError("remote session observation Run is invalid".into()))?;
    if state.selected_id.as_deref() != Some(conversation_id) {
        writeln!(
            writer,
            "Session observation preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        conversation_id,
        writer,
    )? {
        return Ok(());
    }
    post_and_render(client, state, &request, conversation_id, run_id, writer).await
}

fn write_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use session-observation-preview --input FILE. The request is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn read_request<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<serde_json::Value>, RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        write_usage(writer)?;
        return Ok(None);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        write_usage(writer)?;
        return Ok(None);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        write_usage(writer)?;
        return Ok(None);
    }
    let request = match super::super::session_observation::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Session observation input failed: {error}").map_err(io_error)?;
            return Ok(None);
        }
    };
    Ok(Some(request))
}

async fn post_and_render<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    request: &serde_json::Value,
    conversation_id: &str,
    run_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let response = match client
        .preview_session_device_observation(conversation_id, run_id, request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Session observation request failed: {error}").map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::session_observation::validate_response(&response, request) {
        Ok(()) => {
            super::super::session_observation::render_human(&response, writer).map_err(io_error)?;
        }
        Err(error) => {
            writeln!(
                writer,
                "Session observation response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}
