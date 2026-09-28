use std::io::Write;

use serde_json::json;

use crate::client_instance_session_scope;

use super::super::super::{RemoteClient, RemoteError};
use super::super::state::{TuiState, io_error, json_text};

/// Refreshes the explicitly selected inventory/resource projection immediately
/// before an opt-in, read-only planning request or storage-only write.
/// The TUI normally keeps candidate reads opt-in; once a caller selected a
/// client instance and opened both observations, refresh the owner-bound pair
/// atomically. A missing or one-sided observation remains request-free, while
/// a failed or drifting refresh stops the pending operation before its POST.
pub(in super::super) async fn refresh_explicit_inventory_resource_observations<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    operation: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    if state.client_instance_filter.is_none()
        || state.device_inventory_v2_observed.is_none()
        || state.client_instance_resource_view_observed.is_none()
    {
        return Ok(true);
    }

    let response = match client.read_converged_inventory_resource_view().await {
        Ok(response) => response,
        Err(error) => {
            return report_inventory_resource_refresh_failure(state, &error, operation, writer);
        }
    };
    let Some(inventory) = response.get("inventory") else {
        writeln!(
            writer,
            "{operation} inventory/resource refresh returned an invalid envelope. No request was sent."
        )
        .map_err(io_error)?;
        return Ok(false);
    };
    let Some(resource_view) = response.get("resource_view") else {
        writeln!(
            writer,
            "{operation} inventory/resource refresh returned an invalid envelope. No request was sent."
        )
        .map_err(io_error)?;
        return Ok(false);
    };

    // `read_converged_inventory_resource_view` validates both source
    // observations before returning. Commit them together so a later local
    // visibility check cannot mix a new inventory image with an old resource
    // image. This remains process-local display state and never grants
    // placement or execution authority.
    state.device_inventory_v2_observed = Some(inventory.clone());
    state.client_instance_resource_view_observed = Some(resource_view.clone());
    state.refresh_client_instance_observation_status();
    Ok(true)
}

/// Keeps an explicitly selected client-instance projection from sending a
/// Prompt to a conversation outside its locally declared session IDs. The
/// declaration remains display-only: this is a local UX/write guard and does
/// not change the authenticated request or grant instance authority.
pub(super) fn ensure_prompt_visible_to_client_instance<W: Write>(
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
            "Prompt blocked by client-instance display filter: no validated view is available for instance {}. No request was sent.",
            json_text(instance_id)
        )
        .map_err(io_error)?;
        return Ok(false);
    };
    let conversation = json!({"id": conversation_id});
    if client_instance_session_scope::matches_conversation(
        &conversation,
        Some(view),
        Some(instance_id),
    ) {
        return Ok(true);
    }
    writeln!(
        writer,
        "Prompt blocked by client-instance display filter: conversation {} is not declared for instance {}. No request was sent.",
        json_text(conversation_id),
        json_text(instance_id)
    )
    .map_err(io_error)?;
    Ok(false)
}

/// Keeps an explicitly selected client-instance projection from sending a
/// pending Run-intent to a conversation outside its locally declared session
/// IDs. This is a local UX/write guard only; it does not grant instance or
/// execution authority.
pub(in super::super) fn ensure_pending_run_intent_visible_to_client_instance<W: Write>(
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
            "Pending Run-intent blocked by client-instance display filter: no validated view is available for instance {}. No request was sent.",
            json_text(instance_id)
        )
        .map_err(io_error)?;
        return Ok(false);
    };
    let conversation = json!({"id": conversation_id});
    if client_instance_session_scope::matches_conversation(
        &conversation,
        Some(view),
        Some(instance_id),
    ) {
        return Ok(true);
    }
    writeln!(
        writer,
        "Pending Run-intent blocked by client-instance display filter: conversation {} is not declared for instance {}. No request was sent.",
        json_text(conversation_id),
        json_text(instance_id)
    )
    .map_err(io_error)?;
    Ok(false)
}

/// Keeps an explicit pair of inventory and resource observations from being
/// used as if it were one snapshot after it has drifted. A single observation
/// remains compatible with the existing display-only flow; once both are
/// present, writes must see the same owner, device, lifecycle, capacity, GPU,
/// and observation-time image. This is a local UX guard and never grants
/// inventory, placement, lease, or execution authority.
pub(in super::super) fn ensure_inventory_resource_converged<W: Write>(
    state: &TuiState,
    operation: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let (Some(inventory), Some(resource)) = (
        state.device_inventory_v2_observed.as_ref(),
        state.client_instance_resource_view_observed.as_ref(),
    ) else {
        return Ok(true);
    };
    if super::super::inventory_convergence::observations_converged(inventory, resource) {
        return Ok(true);
    }
    writeln!(
        writer,
        "{operation} blocked by inventory/resource observation drift. No request was sent."
    )
    .map_err(io_error)?;
    Ok(false)
}

fn report_inventory_resource_refresh_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    operation: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let pending_prompt = state.pending_prompt.clone();
    let pending_run_intent = state.pending_run_intent.clone();
    let client_instance_filter = state.client_instance_filter.clone();
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    writeln!(
        writer,
        "{operation} inventory/resource refresh failed: {error}. No request was sent."
    )
    .map_err(io_error)?;
    if cleared {
        // The owner view is revoked, but keep the caller's pending
        // storage-only write and selected display boundary in memory.
        // With no replacement observations the next retry remains
        // fail-closed until the user explicitly reopens the view.
        state.pending_prompt = pending_prompt;
        state.pending_run_intent = pending_run_intent;
        state.client_instance_filter = client_instance_filter;
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    }
    Ok(false)
}
