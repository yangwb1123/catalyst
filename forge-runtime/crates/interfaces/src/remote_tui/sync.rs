use std::io::Write;

use serde_json::Value;

use super::super::changes::OwnedConversationChange;
use super::super::{RemoteClient, RemoteError};
use super::state::{io_error, refresh_sessions};
use super::{load_session_history, state::TuiState};

pub(super) async fn sync_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let page = client
        .conversation_changes_after(state.change_cursor)
        .await?;
    for change in &page.changes {
        apply_conversation_change(state, change);
    }
    let change_count = page.changes.len();
    let next_cursor = page.scanned_through_cursor;
    let has_more = page.has_more;
    if let Err(error) = refresh_sessions(client, state, false).await {
        writeln!(
            writer,
            "Sync session refresh failed: {error}. The change cursor was not advanced."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if let Some(selected_id) = state.selected_id.clone() {
        let history = match load_session_history(client, &selected_id).await {
            Ok(history) => history,
            Err(error) => {
                writeln!(writer, "Sync Prompt history refresh failed: {error}. The change cursor was not advanced.")
                    .map_err(io_error)?;
                return Ok(());
            }
        };
        state.record_prompt_history(&selected_id, &history);
        writeln!(writer, "Prompt history refreshed for the selected session.").map_err(io_error)?;
    }
    client.persist_change_cursor(next_cursor)?;
    state.change_cursor = next_cursor;
    writeln!(
        writer,
        "Synced {change_count} owner-visible changes through cursor {next_cursor}{}.",
        if has_more { "; more remain" } else { "" }
    )
    .map_err(io_error)?;
    Ok(())
}

fn apply_conversation_change(state: &mut TuiState, change: &OwnedConversationChange) {
    for entry in state
        .conversations
        .iter_mut()
        .chain(state.selected_entry.iter_mut())
    {
        if entry.conversation.get("id").and_then(Value::as_str)
            == Some(change.conversation_id.as_str())
        {
            entry.aggregate_version = entry.aggregate_version.max(change.aggregate_version);
        }
    }
}
