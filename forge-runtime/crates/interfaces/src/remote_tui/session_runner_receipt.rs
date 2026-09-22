use std::io::Write;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one bounded canonical receipt observation to the authenticated
/// session preview route after binding it to the selected Conversation/Run.
pub(super) async fn preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_remote_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_remote_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_remote_usage(writer);
    }
    let request = match super::super::session_runner_receipt::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Session Runner receipt input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    let (conversation_id, run_id) =
        match super::super::session_runner_receipt::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(writer, "Session Runner receipt input failed: {error}")
                    .map_err(io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id) {
        writeln!(
            writer,
            "Session Runner receipt preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let response = match client
        .preview_session_runner_receipt_observation(conversation_id, run_id, &request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Session Runner receipt request failed: {error}").map_err(io_error)?;
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
    match super::super::session_runner_receipt::validate_response(
        &response,
        &request,
        conversation_id,
        run_id,
    ) {
        Ok(()) => super::super::session_runner_receipt::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(
                writer,
                "Session Runner receipt response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

/// Shows one bounded session-bound Runner receipt contract file locally.
/// Paths are required so interactive stdin remains available to the TUI.
pub(super) fn offline_preview<W: Write>(argument: &str, writer: &mut W) -> Result<(), RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        return usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return usage(writer);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        return usage(writer);
    }
    let command = crate::args::DeviceCommand::SessionRunnerReceiptPreview {
        input: input.to_owned(),
    };
    match crate::device_session_runner_receipt_command::execute(&command) {
        Ok(output) => {
            crate::device_session_runner_receipt_command::write_output(&output, false, writer)
                .map_err(io_error)?
        }
        Err(error) => {
            writeln!(writer, "Session Runner receipt preview failed: {error}").map_err(io_error)?
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use session-runner-receipt-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn write_remote_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use session-runner-receipt-preview --input FILE. The file is posted to the authenticated session preview route; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
