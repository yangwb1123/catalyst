use std::io::Write;

use super::{
    RemoteClient, RemoteError,
    state::{TuiState, io_error},
};

/// Posts one owner-bound, metadata-only Runner execution-intent preview for
/// the selected session. It never selects a target or opens Runner transport.
pub(super) async fn remote_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
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
    let request = match super::super::runner_execution_intent::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Runner execution-intent input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    let (conversation_id, run_id) =
        match super::super::runner_execution_intent::conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(writer, "Runner execution-intent input failed: {error}")
                    .map_err(io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id.as_str()) {
        writeln!(writer, "Runner execution-intent preview requires the selected session to match conversation {conversation_id}.").map_err(io_error)?;
        return Ok(());
    }
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        return Ok(());
    }
    // Once the caller has selected an instance and opened both owner-bound
    // observations, refresh that pair immediately before the candidate POST.
    // The refresh can revoke the selected Conversation, so repeat the local
    // visibility fence before checking convergence and posting.
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Runner execution-intent",
        writer,
    )
    .await?
    {
        return Ok(());
    }
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        &conversation_id,
        writer,
    )? {
        return Ok(());
    }
    if state.client_instance_filter.is_some()
        && !super::writes::ensure_inventory_resource_converged(
            state,
            "Runner execution-intent",
            writer,
        )?
    {
        return Ok(());
    }
    let response = match client
        .preview_runner_execution_intent(&conversation_id, &run_id, &request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Runner execution-intent request failed: {error}")
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
    match super::super::runner_execution_intent::validate_response(
        &response,
        &request,
        &conversation_id,
        &run_id,
    ) {
        Ok(()) => super::super::runner_execution_intent::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Runner execution-intent response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(writer, "Use runner-execution-intent-remote-preview --input FILE. The request is metadata-only; '-' is reserved for the standalone CLI.").map_err(io_error)
}
