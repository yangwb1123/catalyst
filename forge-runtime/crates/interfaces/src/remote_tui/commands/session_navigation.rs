use super::{
    OwnedConversationEntry, RemoteClient, RemoteError, TuiState,
    drop_selected_session_after_read_rejection, ensure_conversation_visible_to_client_instance,
    io_error, json_text, refresh_sessions,
};
use serde_json::Value;
use std::io::Write;

pub(super) async fn refresh_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    next_page: bool,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match refresh_sessions(client, state, next_page).await {
        Ok(()) if next_page => Ok(()),
        Ok(()) => writeln!(writer, "Sessions refreshed.").map_err(io_error),
        Err(error) => {
            let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
            let action = if next_page {
                "Next page failed"
            } else {
                "Refresh failed"
            };
            writeln!(writer, "{action}: {error}").map_err(io_error)?;
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

pub(super) async fn open_session<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !ensure_conversation_visible_to_client_instance(state, conversation_id, writer)? {
        return Ok(());
    }
    if let Err(error) = resolve_session_entry(client, state, conversation_id).await {
        let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
        writeln!(writer, "Conversation detail request failed: {error}").map_err(io_error)?;
        if cleared {
            writeln!(
                writer,
                "Local session view cleared after authorization failure."
            )
            .map_err(io_error)?;
        }
        return Ok(());
    }
    if state.selected_id.as_deref() != Some(conversation_id) {
        state.clear_prompt_history();
        state.clear_run_timeline();
    }
    state.selected_id = Some(conversation_id.to_owned());
    match super::super::load_session_history(client, conversation_id).await {
        Ok(prompts) => {
            state.record_prompt_history(conversation_id, &prompts);
            writeln!(
                writer,
                "Opened session {} and refreshed Prompt history.",
                json_text(conversation_id)
            )
            .map_err(io_error)?;
        }
        Err(error) => {
            report_history_failure(state, conversation_id, &error, "History", writer)?;
        }
    }
    Ok(())
}

pub(super) async fn show_session_detail<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !ensure_conversation_visible_to_client_instance(state, conversation_id, writer)? {
        return Ok(());
    }
    let detail = match resolve_session_entry(client, state, conversation_id).await {
        Ok(Some(detail)) => detail,
        Ok(None) => match client.get_conversation(conversation_id).await {
            Ok(detail) => detail,
            Err(error) => {
                report_request_failure(state, &error, "Conversation detail", writer)?;
                return Ok(());
            }
        },
        Err(error) => {
            report_request_failure(state, &error, "Conversation detail", writer)?;
            return Ok(());
        }
    };
    if state.selected_id.as_deref() != Some(conversation_id) {
        state.clear_run_timeline();
    }
    state.selected_id = Some(conversation_id.to_owned());
    state.selected_entry = Some(detail.clone());
    writeln!(
        writer,
        "Conversation detail: {}",
        serde_json::to_string(&detail).unwrap_or_default()
    )
    .map_err(io_error)?;
    Ok(())
}

/// Returns a fetched detail only when the requested session is outside the
/// loaded page. The caller can keep the existing fast path for loaded rows.
pub(super) async fn resolve_session_entry(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
) -> Result<Option<OwnedConversationEntry>, RemoteError> {
    if state
        .conversations
        .iter()
        .chain(state.selected_entry.iter())
        .any(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(conversation_id))
    {
        return Ok(None);
    }
    let detail = client.get_conversation(conversation_id).await?;
    state.selected_entry = Some(detail.clone());
    Ok(Some(detail))
}

pub(super) async fn older_history<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !argument.trim().is_empty() {
        writeln!(
            writer,
            "Use older with no arguments to load the next Prompt page."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(conversation_id) = state.selected_id.clone() else {
        writeln!(
            writer,
            "Open a session before loading older Prompt history."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    if !ensure_conversation_visible_to_client_instance(state, &conversation_id, writer)? {
        return Ok(());
    }
    if state.history_loaded_for.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Open the selected session before loading older Prompt history."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(before) = state.history_before.clone() else {
        writeln!(writer, "No older Prompt history is available.").map_err(io_error)?;
        return Ok(());
    };
    match client.list_prompts(&conversation_id, Some(&before)).await {
        Ok(prompts) => {
            state.record_prompt_history(&conversation_id, &prompts);
            writeln!(writer, "Older Prompt history loaded.").map_err(io_error)?;
        }
        Err(error) => {
            report_history_failure(state, &conversation_id, &error, "Older history", writer)?;
        }
    }
    Ok(())
}

fn report_history_failure<W: Write>(
    state: &mut TuiState,
    conversation_id: &str,
    error: &RemoteError,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    let dropped = if cleared {
        false
    } else {
        drop_selected_session_after_read_rejection(state, conversation_id, error)
    };
    writeln!(writer, "{operation} request failed: {error}").map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    } else if dropped {
        writeln!(
            writer,
            "Selected session was removed after its owner history read was rejected."
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn report_request_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    operation: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(writer, "{operation} request failed: {error}").map_err(io_error)?;
    if cleared {
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    }
    Ok(())
}
