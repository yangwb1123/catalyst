use std::io::Write;

use serde_json::Value;

use crate::args::{RemoteConversationScope, parse_scope};
use crate::client_instance_session_scope;

use super::super::super::{OwnedConversationEntry, RemoteClient, RemoteError};
use super::super::state::{
    PendingCreate, TuiState, io_error, json_text, new_idempotency_key, refresh_sessions,
};
use super::{pending_write_blocks, retry_pending};

pub(in super::super) async fn create_session<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if pending_write_blocks(state, writer)? {
        return Ok(());
    }
    let (scope, title) = parse_create_argument(argument)?;
    if title.trim().is_empty() || title.len() > 256 {
        writeln!(writer, "Title must contain 1..256 bytes.").map_err(io_error)?;
        return Ok(());
    }
    if let Err(error) = super::super::state::ensure_converged_client_instance_projection(state) {
        writeln!(
            writer,
            "Create blocked by client-instance display filter: {error}. No request was sent."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    state.pending_create = Some(PendingCreate {
        title,
        scope,
        idempotency_key: new_idempotency_key()?,
    });
    retry_pending(client, state, writer).await
}

fn parse_create_argument(argument: &str) -> Result<(RemoteConversationScope, String), RemoteError> {
    if let Some(options) = argument.strip_prefix("--scope ") {
        let (selector, title) = options
            .split_once(' ')
            .ok_or_else(|| RemoteError("create --scope requires a scope and title".into()))?;
        let scope = parse_scope(selector).map_err(RemoteError)?;
        return Ok((scope, title.trim_start().to_owned()));
    }
    Ok((RemoteConversationScope::Global, argument.to_owned()))
}

pub(super) async fn retry_create<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(pending) = state.pending_create.clone() else {
        return Ok(());
    };
    if !refresh_client_instance_before_create(client, state, writer).await? {
        return Ok(());
    }
    let result = client
        .create_conversation(&pending.title, &pending.scope, &pending.idempotency_key)
        .await;
    match result {
        Ok(created) => complete_create(client, state, created, writer).await?,
        Err(error) => report_create_error(state, &error, writer)?,
    }
    Ok(())
}

/// Refreshes the selected client-instance pair before an owner-wide create.
/// The pair remains a display projection; this guard only prevents a stale
/// instance declaration from being used as the basis for the write.
async fn refresh_client_instance_before_create<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let Some(instance_id) = state.client_instance_filter.clone() else {
        return Ok(true);
    };
    let response = match client.read_converged_client_instance_views().await {
        Ok(response) => response,
        Err(error) => return report_create_projection_refresh_failure(state, &error, writer),
    };
    let session_view = response
        .get("session_view")
        .cloned()
        .ok_or_else(|| RemoteError("Forge API returned an invalid client-instance pair".into()))?;
    let resource_view = response
        .get("resource_view")
        .cloned()
        .ok_or_else(|| RemoteError("Forge API returned an invalid client-instance pair".into()))?;
    if client_instance_session_scope::scope_from_view(&session_view, &instance_id).is_err() {
        state.clear_client_instance_private_projection();
        state.reconcile_client_instance_selection();
        writeln!(
            writer,
            "Create blocked: client-instance {instance_id:?} is not declared by the converged session/resource pair. No request was sent."
        )
        .map_err(io_error)?;
        return Ok(false);
    }
    state.client_instance_session_view_observed = Some(session_view);
    state.client_instance_resource_view_observed = Some(resource_view);
    state.mark_client_instance_observations_converged();
    state.reconcile_client_instance_selection();
    Ok(true)
}

fn report_create_projection_refresh_failure<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let pending_create = state.pending_create.clone();
    let client_instance_filter = state.client_instance_filter.clone();
    let cleared = super::super::clear_session_view_after_authorization_error(state, error);
    if !cleared {
        state.mark_client_instance_observations_not_converged();
    }
    writeln!(
        writer,
        "Create client-instance projection refresh failed: {error}. No request was sent."
    )
    .map_err(io_error)?;
    if cleared {
        state.pending_create = pending_create;
        state.client_instance_filter = client_instance_filter;
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    }
    Ok(false)
}

async fn complete_create<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    created: Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    state.pending_create = None;
    let id = created.get("id").and_then(Value::as_str).map(str::to_owned);
    writeln!(
        writer,
        "Created session {}.",
        id.as_deref()
            .map_or_else(|| "(id unavailable)".into(), json_text)
    )
    .map_err(io_error)?;
    if let Some(id) = id {
        let visible =
            super::super::state::conversation_visible_to_selected_client_instance(state, &created);
        if visible {
            state.selected_entry = Some(OwnedConversationEntry {
                conversation: created,
                aggregate_version: 1,
            });
            state.clear_prompt_history();
            state.clear_run_timeline();
            state.selected_id = Some(id);
        } else {
            writeln!(
                writer,
                "Created session is outside the selected client-instance display projection; it remains unselected."
            )
            .map_err(io_error)?;
        }
    }
    if let Err(error) = refresh_sessions(client, state, false).await {
        let cleared = super::super::clear_session_view_after_authorization_error(state, &error);
        writeln!(writer, "Session was created, but refresh failed: {error}").map_err(io_error)?;
        if cleared {
            writeln!(
                writer,
                "Local session view cleared after authorization failure."
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

fn report_create_error<W: Write>(
    state: &mut TuiState,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if super::super::response_status(error).is_some_and(|status| matches!(status, 401 | 403)) {
        state.pending_create = None;
        state.clear_remote_session_view();
        writeln!(writer, "Create was rejected: {error}").map_err(io_error)?;
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    if super::super::response_status(error).is_some_and(super::super::definitive_client_rejection) {
        state.pending_create = None;
        writeln!(writer, "Create was rejected: {error}").map_err(io_error)
    } else {
        writeln!(
            writer,
            "{error}. Enter retry to reuse the same idempotency key."
        )
        .map_err(io_error)
    }
}
