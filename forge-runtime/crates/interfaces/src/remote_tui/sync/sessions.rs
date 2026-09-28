use super::{RemoteClient, RemoteError, TuiState, io_error};
use super::{load_session_history, refresh_sessions};
use std::io::Write;

pub(super) async fn sync_sessions<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if let Err(error) = refresh_sessions(client, state, false).await {
        let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
        writeln!(
            writer,
            "Sync session refresh failed: {error}. The change cursor was not advanced."
        )
        .map_err(io_error)?;
        if cleared {
            writeln!(
                writer,
                "Local session view cleared after authorization failure."
            )
            .map_err(io_error)?;
        }
        return Ok(false);
    }
    if let Some(selected_id) = state.selected_id.clone()
        && super::super::commands::ensure_conversation_visible_to_client_instance(
            state,
            &selected_id,
            writer,
        )?
    {
        let history = match load_session_history(client, &selected_id).await {
            Ok(history) => history,
            Err(error) => {
                report_history_error(state, &selected_id, &error, writer)?;
                return Ok(false);
            }
        };
        state.record_prompt_history(&selected_id, &history);
        writeln!(writer, "Prompt history refreshed for the selected session.").map_err(io_error)?;
    }
    Ok(true)
}

pub(super) fn report_history_error<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
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
        "Sync Prompt history refresh failed: {error}. The change cursor was not advanced."
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
