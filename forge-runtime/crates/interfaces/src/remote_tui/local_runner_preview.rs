use std::io::Write;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one bounded local Runner execution-readiness preview after requiring
/// the input Conversation to be the selected owner-scoped session.
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
    let request = match super::super::local_runner_preview::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Local Runner preview input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    let (conversation_id, intent_id) =
        match super::super::local_runner_preview::conversation_and_intent(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(writer, "Local Runner preview input failed: {error}").map_err(io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id) {
        writeln!(
            writer,
            "Local Runner preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let response = match client
        .preview_local_runner_execution_readiness(conversation_id, intent_id, &request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Local Runner preview request failed: {error}").map_err(io_error)?;
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
    match super::super::local_runner_preview::validate_response(
        &response,
        &request,
        conversation_id,
        intent_id,
    ) {
        Ok(()) => {
            super::super::local_runner_preview::render_human(&response, writer).map_err(io_error)?
        }
        Err(error) => {
            writeln!(
                writer,
                "Local Runner preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

fn write_remote_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use runner-execution-readiness-preview --input FILE. The file is posted to the authenticated test-only local Runner preview route; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
