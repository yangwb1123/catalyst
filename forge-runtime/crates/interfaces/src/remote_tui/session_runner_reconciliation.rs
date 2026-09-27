use std::io::Write;

use super::state::TuiState;
use super::{RemoteClient, RemoteError, state::io_error};

/// Reads and renders one caller-supplied projection locally. The command does
/// not use the TUI's remote client or mutate session state.
pub(super) fn preview<W: Write>(argument: &str, writer: &mut W) -> Result<(), RemoteError> {
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
    match super::super::session_runner_reconciliation::read_tui_projection(input) {
        Ok(projection) => {
            super::super::session_runner_reconciliation::render_human(&projection, writer)
                .map_err(io_error)?
        }
        Err(error) => {
            writeln!(
                writer,
                "Session Runner reconciliation projection failed: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

/// Posts the complete receipt history to the authenticated Core projection
/// route. The command is explicit so the offline projection consumer above
/// cannot accidentally open a client or issue a request.
pub(super) async fn remote_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        return usage_remote(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return usage_remote(writer);
    }
    let input = suffix.trim();
    if input.is_empty() || input == "-" {
        return usage_remote(writer);
    }
    let request = match super::super::session_runner_reconciliation::read_remote_tui_request(input)
    {
        Ok(request) => request,
        Err(error) => {
            writeln!(
                writer,
                "Session Runner reconciliation history input failed: {error}"
            )
            .map_err(io_error)?;
            return Ok(());
        }
    };
    let (conversation_id, run_id) =
        match super::super::session_runner_reconciliation::remote_conversation_and_run(&request) {
            Ok(ids) => ids,
            Err(error) => {
                writeln!(
                    writer,
                    "Session Runner reconciliation history input failed: {error}"
                )
                .map_err(io_error)?;
                return Ok(());
            }
        };
    if state.selected_id.as_deref() != Some(conversation_id) {
        writeln!(
            writer,
            "Session Runner reconciliation preview requires the selected session to match conversation {conversation_id}."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if let Some(selected_run_id) = state.selected_run_id.as_deref()
        && selected_run_id != run_id
    {
        writeln!(
            writer,
            "Session Runner reconciliation preview requires the selected Run to match Run {run_id}."
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
    let canonical_history = match client
        .preview_session_runner_receipt_history(conversation_id, run_id, &request)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Session Runner receipt history request failed: {error}"
            )
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
    if let Err(error) = super::super::session_runner_receipt_history::validate_response(
        &canonical_history,
        &request,
        conversation_id,
        run_id,
    ) {
        writeln!(
            writer,
            "Session Runner receipt history response failed validation: {error}"
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let response = match client
        .preview_session_runner_reconciliation(conversation_id, run_id, &canonical_history)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Session Runner reconciliation request failed: {error}"
            )
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
    match super::super::session_runner_reconciliation::validate_remote_response(
        &response,
        &request,
        conversation_id,
        run_id,
    ) {
        Ok(()) => {
            super::super::session_runner_reconciliation::render_remote_human(&response, writer)
                .map_err(io_error)?
        }
        Err(error) => {
            writeln!(
                writer,
                "Session Runner reconciliation response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

fn usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use session-runner-reconciliation-preview --input FILE. The projection is validated locally; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn usage_remote<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use session-runner-reconciliation-remote-preview --input FILE. The history is canonicalized once, then forwarded to the authenticated reconciliation preview; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}
