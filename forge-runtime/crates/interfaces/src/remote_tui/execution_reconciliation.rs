use std::io::Write;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one bounded restart image to the authenticated reconciliation
/// candidate after binding it to the selected owner-scoped session. The
/// response is metadata-only and never becomes a retry or dispatch command.
pub(super) async fn preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(request) = read_request(argument, writer)? else {
        return Ok(());
    };
    let (conversation_id, run_id) =
        match super::super::execution_reconciliation::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(writer, "Execution reconciliation input failed: {error}")
                    .map_err(io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Execution reconciliation preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        return Ok(());
    }
    post_and_render(client, state, &request, &conversation_id, &run_id, writer).await
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use execution-reconciliation-preview --input FILE. The file is posted to the authenticated test-only candidate; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn read_request<W: Write>(
    argument: &str,
    writer: &mut W,
) -> Result<Option<serde_json::Value>, RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        usage(writer)?;
        return Ok(None);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        usage(writer)?;
        return Ok(None);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        usage(writer)?;
        return Ok(None);
    }
    let request = match super::super::execution_reconciliation::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Execution reconciliation input failed: {error}").map_err(io_error)?;
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
        .preview_execution_reconciliation(conversation_id, run_id, request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Execution reconciliation request failed: {error}")
                .map_err(io_error)?;
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
    match super::super::execution_reconciliation::validate_response(
        &response,
        request,
        conversation_id,
        run_id,
    ) {
        Ok(()) => super::super::execution_reconciliation::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => {
            writeln!(
                writer,
                "Execution reconciliation response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}
