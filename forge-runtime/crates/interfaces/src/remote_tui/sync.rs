use std::io::Write;

use serde_json::Value;

use super::super::changes::OwnedConversationChange;
use super::super::{RemoteClient, RemoteError};
use super::state::{io_error, refresh_sessions};
use super::{load_session_history, state::TuiState};

#[path = "sync/changes.rs"]
mod changes;
#[path = "sync/observations.rs"]
mod observations;
#[path = "sync/sessions.rs"]
mod sessions;
#[path = "sync/timeline.rs"]
mod timeline;
use changes::drain_changes;
use observations::{sync_device_inventory, sync_observations};
use sessions::sync_sessions;
pub(super) use timeline::sync_selected_run_timeline;

pub(super) async fn sync_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(ChangeProgress {
        next_cursor,
        change_count,
        has_more,
    }) = drain_changes(client, state, writer).await?
    else {
        return Ok(());
    };
    let previous_resource_view = state.client_instance_resource_view_observed.clone();
    let client_instance_refreshed_before_owner_reads = state.client_instance_filter.is_some();
    if client_instance_refreshed_before_owner_reads
        && !super::sync_client_instances::sync_client_instance_views(client, state, writer).await?
    {
        return Ok(());
    }
    if !sync_sessions(client, state, writer).await? {
        return Ok(());
    }
    if !sync_selected_run_timeline(client, state, writer).await?
        || !sync_device_inventory(client, state, writer).await?
    {
        return Ok(());
    }
    if !sync_observations(
        client,
        state,
        writer,
        previous_resource_view,
        client_instance_refreshed_before_owner_reads,
    )
    .await?
    {
        return Ok(());
    }
    finish_sync(
        client,
        state,
        writer,
        ChangeProgress {
            next_cursor,
            change_count,
            has_more,
        },
    )
}

fn finish_sync<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
    progress: ChangeProgress,
) -> Result<(), RemoteError> {
    let ChangeProgress {
        next_cursor,
        change_count,
        has_more,
    } = progress;
    state.reconcile_client_instance_selection();
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

pub(super) fn apply_conversation_change(state: &mut TuiState, change: &OwnedConversationChange) {
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

#[derive(Clone, Copy)]
struct ChangeProgress {
    next_cursor: u64,
    change_count: usize,
    has_more: bool,
}
