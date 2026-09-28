use std::io::Write;

use serde_json::Value;

use crate::client_instance_session_scope;

use super::super::OwnedConversationEntry;
use super::state::{
    TuiState, io_error, json_text, new_idempotency_key, refresh_sessions, scope_filter_label,
    write_help,
};
use super::{RemoteClient, RemoteError, state};
#[path = "commands/dispatch.rs"]
mod dispatch;
pub(super) use dispatch::dispatch_command;

#[path = "changes_watch.rs"]
mod changes_watch;
#[path = "commands_client_instances.rs"]
mod client_instance_views;
#[path = "credential_candidate.rs"]
mod credential_candidate;
#[path = "commands_import.rs"]
mod import_command;
#[path = "commands_inventory.rs"]
mod inventory;
#[path = "commands_lifecycle_registry.rs"]
mod lifecycle_registry;

#[path = "commands/placement.rs"]
mod placement;
use placement::{placement_preview, placement_registry_preview};
#[path = "commands/scheduler.rs"]
mod scheduler;
use scheduler::{
    scheduler_selection_lease, scheduler_selection_lease_release, scheduler_selection_lease_renew,
    scheduler_selection_preview,
};
#[path = "commands/session_filter.rs"]
mod session_filter;
use session_filter::{filter_sessions, instance_filter_command};
#[path = "commands/session_navigation.rs"]
mod session_navigation;
use session_navigation::{older_history, open_session, refresh_command, show_session_detail};
#[path = "commands/run_intent_preview.rs"]
mod run_intent_preview;
use run_intent_preview::show_run_intent_preview;

pub(super) fn write_remote_inventory_v2<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    inventory::write_remote_inventory_v2(value, writer)
}

pub(super) fn write_remote_inventory<W: Write>(
    value: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    inventory::write_remote_inventory(value, writer)
}

fn handle_exit<W: Write>(
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.has_pending_write() {
        if argument == "--discard-pending" {
            state::write_pending_recovery(state, writer)?;
            state.pending_prompt = None;
            state.pending_run_intent = None;
            state.pending_create = None;
            return Ok(true);
        }
        writeln!(writer, "A write may have been accepted. Enter retry, or quit --discard-pending to print recovery data and exit.")
            .map_err(io_error)?;
        return Ok(false);
    }
    if argument.is_empty() {
        return Ok(true);
    }
    writeln!(
        writer,
        "Use quit with no arguments, or quit --discard-pending."
    )
    .map_err(io_error)?;
    Ok(false)
}

/// Keeps owner Conversation and Prompt reads inside the caller-declared local
/// client-instance projection. The projection is process-local display state:
/// it never changes the authenticated request or grants authorization.
pub(super) fn ensure_conversation_visible_to_client_instance<W: Write>(
    state: &TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let Some(instance_id) = state.client_instance_filter.as_deref() else {
        return Ok(true);
    };
    let Some(view) = state.active_client_instance_view() else {
        writeln!(
            writer,
            "Conversation read blocked by client-instance display filter: no validated view is available for instance {instance_id:?}. No request was sent."
        )
        .map_err(io_error)?;
        return Ok(false);
    };
    let conversation = serde_json::json!({"id": conversation_id});
    if client_instance_session_scope::matches_conversation(
        &conversation,
        Some(view),
        Some(instance_id),
    ) {
        return Ok(true);
    }
    writeln!(
        writer,
        "Conversation read blocked by client-instance display filter: conversation {} is not declared for instance {instance_id:?}. No request was sent.",
        json_text(conversation_id)
    )
    .map_err(io_error)?;
    Ok(false)
}

/// A private owner read that receives a deterministic client rejection no
/// longer proves the cached selected Conversation. Drop only that row and its
/// private projection; transient reads keep the existing stale/error path.
pub(super) fn drop_selected_session_after_read_rejection(
    state: &mut TuiState,
    conversation_id: &str,
    error: &RemoteError,
) -> bool {
    if !(super::response_status(error).is_some_and(super::definitive_client_rejection)
        || matches!(
            error.0.as_str(),
            "Forge API returned duplicate JSON keys"
                | "Forge API returned invalid JSON"
                | "Forge API returned an invalid prompt page"
                | "Forge API returned an invalid Run page"
                | "Forge API returned an invalid Run timeline"
                | "Forge API returned an invalid Run observation"
                | "Forge API returned a Run observation with mismatched binding"
                | "Forge API returned another conversation"
        ))
        || state.selected_id.as_deref() != Some(conversation_id)
    {
        return false;
    }
    state.drop_selected_session(conversation_id);
    true
}
