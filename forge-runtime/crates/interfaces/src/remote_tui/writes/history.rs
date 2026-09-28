use std::io::Write;

use serde_json::Value;

use super::super::super::{RemoteClient, RemoteError};
use super::super::state::{TuiState, io_error, refresh_sessions};

pub(super) fn update_prompt_version(state: &mut TuiState, conversation_id: &str, version: u64) {
    if let Some(entry) = state
        .conversations
        .iter_mut()
        .chain(state.selected_entry.iter_mut())
        .find(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(conversation_id))
    {
        entry.aggregate_version = entry.aggregate_version.max(version);
    }
}

pub(super) async fn refresh_after_conflict<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match refresh_sessions(client, state, false).await {
        Ok(()) => {
            let session_is_loaded = state.conversations.iter().any(|entry| {
                entry.conversation.get("id").and_then(Value::as_str) == Some(conversation_id)
            });
            if session_is_loaded {
                writeln!(
                    writer,
                    "The session was refreshed; submit the {operation} again."
                )
                .map_err(io_error)
            } else {
                state.selected_id = None;
                state.selected_entry = None;
                state.clear_prompt_history();
                state.clear_run_timeline();
                writeln!(writer, "The session is outside the refreshed page. Use next until it appears, then open it before submitting again.")
                    .map_err(io_error)
            }
        }
        Err(refresh_error) => {
            let cleared =
                super::super::clear_session_view_after_authorization_error(state, &refresh_error);
            writeln!(
                writer,
                "Session refresh failed: {refresh_error}. Refresh before submitting again."
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            Ok(())
        }
    }
}

pub(super) async fn refresh_after_storage_write<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match super::super::load_session_history(client, conversation_id).await {
        Ok(page) => {
            state.record_prompt_history(conversation_id, &page);
            writeln!(writer, "Prompt history refreshed.").map_err(io_error)?;
        }
        Err(error) => {
            report_stored_history_refresh_error(state, conversation_id, operation, &error, writer)?;
        }
    }
    Ok(())
}

fn report_stored_history_refresh_error<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
    operation: &str,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    let dropped = if cleared {
        false
    } else {
        super::super::commands::drop_selected_session_after_read_rejection(
            state,
            conversation_id,
            error,
        )
    };
    writeln!(
        writer,
        "{operation} was stored, but history refresh failed: {error}. Use sync to refresh it."
    )
    .map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    } else if dropped {
        writeln!(
            writer,
            "Selected session was removed after its owner Prompt read was rejected."
        )
        .map_err(io_error)?;
    }
    Ok(())
}
