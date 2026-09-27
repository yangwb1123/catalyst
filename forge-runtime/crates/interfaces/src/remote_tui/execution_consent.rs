use std::io::Write;

use super::{RemoteClient, RemoteError, state::TuiState};

/// Reads the selected session's server-resolved execution profile preview.
/// The TUI deliberately uses the selected owner-visible session and accepts
/// no alternate path argument, preventing a stale or arbitrary Conversation
/// from being presented as the active session's preflight.
pub(super) async fn preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !argument.trim().is_empty() {
        return write_usage(writer);
    }
    let Some(conversation_id) = selected_conversation_id(state, writer)? else {
        return Ok(());
    };
    if !super::commands::ensure_conversation_visible_to_client_instance(
        state,
        conversation_id,
        writer,
    )? {
        return Ok(());
    }
    let response = match client.preview_execution_consent(conversation_id).await {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Execution-consent preview request failed: {error}")
                .map_err(super::state::io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(super::state::io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::execution_consent_preview::render_human(&response, conversation_id, writer)
    {
        Ok(()) => {}
        Err(error) => {
            writeln!(
                writer,
                "Execution-consent preview response failed validation: {error}"
            )
            .map_err(super::state::io_error)?;
        }
    }
    Ok(())
}

fn selected_conversation_id<'a, W: Write>(
    state: &'a TuiState,
    writer: &mut W,
) -> Result<Option<&'a str>, RemoteError> {
    let Some(conversation_id) = state.selected_id.as_deref() else {
        writeln!(
            writer,
            "Open a session before viewing its execution-consent preview."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    };
    if state.selected_conversation().is_none() {
        writeln!(
            writer,
            "Refresh or open the selected session before viewing its execution-consent preview."
        )
        .map_err(super::state::io_error)?;
        return Ok(None);
    }
    Ok(Some(conversation_id))
}

fn write_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use execution-consent-preview with the selected session and no arguments. It is a read-only candidate and does not grant consent or start a Run."
    )
    .map_err(super::state::io_error)
}
