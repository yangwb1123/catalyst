use std::io::Write;

use serde_json::{json, Value};

use crate::args::{parse_scope, RemoteConversationScope};
use crate::client_instance_session_scope;

use super::super::{OwnedConversationEntry, RemoteClient, RemoteError};
use super::state::{
    io_error, json_text, new_idempotency_key, refresh_sessions, PendingCreate, PendingPrompt,
    PendingRunIntent, TuiState,
};

pub(super) async fn create_session<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if state.has_pending_write() {
        writeln!(
            writer,
            "Resolve the pending write with retry before starting another write."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let (scope, title) = parse_create_argument(argument)?;
    if title.trim().is_empty() || title.len() > 256 {
        writeln!(writer, "Title must contain 1..256 bytes.").map_err(io_error)?;
        return Ok(());
    }
    if let Err(error) = super::state::ensure_converged_client_instance_projection(state) {
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

pub(super) async fn send_new_pending_run_intent<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    content: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if state.has_pending_write() {
        writeln!(
            writer,
            "Resolve the pending write with retry before starting another write."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(conversation_id) = state.selected_id.clone() else {
        writeln!(
            writer,
            "Open a session before submitting a pending Run-intent."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    let Some(entry) = state.selected_conversation() else {
        writeln!(
            writer,
            "Refresh the selected session before submitting a pending Run-intent."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    if !ensure_pending_run_intent_visible_to_client_instance(state, &conversation_id, writer)? {
        return Ok(());
    }
    if !ensure_inventory_resource_converged(state, "Pending Run-intent", writer)? {
        return Ok(());
    }
    if content.trim().is_empty() || content.len() > super::MAX_TUI_INPUT_BYTES {
        writeln!(
            writer,
            "Pending Run-intent content must contain 1..256 KiB."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    state.pending_run_intent = Some(PendingRunIntent {
        conversation_id,
        expected_version: entry.aggregate_version,
        content: content.to_owned(),
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

pub(super) async fn send_new_prompt<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    content: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if state.has_pending_write() {
        writeln!(
            writer,
            "Resolve the pending write with retry before starting another write."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(conversation_id) = state.selected_id.clone() else {
        writeln!(writer, "Open a session before sending a prompt.").map_err(io_error)?;
        return Ok(());
    };
    let Some(entry) = state.selected_conversation() else {
        writeln!(
            writer,
            "Refresh the selected session before sending a prompt."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    if !ensure_prompt_visible_to_client_instance(state, &conversation_id, writer)? {
        return Ok(());
    }
    if !ensure_inventory_resource_converged(state, "Prompt", writer)? {
        return Ok(());
    }
    if content.trim().is_empty() || content.len() > super::MAX_TUI_INPUT_BYTES {
        writeln!(writer, "Prompt must contain 1..256 KiB.").map_err(io_error)?;
        return Ok(());
    }
    let expected_version = entry.aggregate_version;
    state.pending_prompt = Some(PendingPrompt {
        conversation_id,
        expected_version,
        content: content.to_owned(),
        idempotency_key: new_idempotency_key()?,
    });
    retry_pending(client, state, writer).await
}

pub(super) async fn retry_pending<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if let Some(conversation_id) = state
        .pending_prompt
        .as_ref()
        .map(|pending| pending.conversation_id.clone())
    {
        if !refresh_explicit_inventory_resource_observations(client, state, "Prompt", writer)
            .await?
        {
            return Ok(());
        }
        if !ensure_prompt_visible_to_client_instance(state, &conversation_id, writer)? {
            return Ok(());
        }
        if !ensure_inventory_resource_converged(state, "Prompt retry", writer)? {
            return Ok(());
        }
        return retry_prompt(client, state, writer).await;
    }
    if let Some(conversation_id) = state
        .pending_run_intent
        .as_ref()
        .map(|pending| pending.conversation_id.clone())
    {
        if !refresh_explicit_inventory_resource_observations(
            client,
            state,
            "Pending Run-intent",
            writer,
        )
        .await?
        {
            return Ok(());
        }
        if !ensure_pending_run_intent_visible_to_client_instance(state, &conversation_id, writer)? {
            return Ok(());
        }
        if !ensure_inventory_resource_converged(state, "Pending Run-intent retry", writer)? {
            return Ok(());
        }
        return retry_pending_run_intent(client, state, writer).await;
    }
    if state.pending_create.is_some() {
        return retry_create(client, state, writer).await;
    }
    writeln!(writer, "There is no unconfirmed write to retry.").map_err(io_error)?;
    Ok(())
}

/// Refreshes the explicitly selected inventory/resource projection immediately
/// before an opt-in, read-only planning request or storage-only write.
/// The TUI normally keeps candidate reads opt-in; once a caller selected a
/// client instance and opened both observations, refresh the owner-bound pair
/// atomically. A missing or one-sided observation remains request-free, while
/// a failed or drifting refresh stops the pending operation before its POST.
pub(super) async fn refresh_explicit_inventory_resource_observations<W: Write>(
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
            let pending_prompt = state.pending_prompt.clone();
            let pending_run_intent = state.pending_run_intent.clone();
            let client_instance_filter = state.client_instance_filter.clone();
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
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
            return Ok(false);
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
fn ensure_prompt_visible_to_client_instance<W: Write>(
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
pub(super) fn ensure_pending_run_intent_visible_to_client_instance<W: Write>(
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
pub(super) fn ensure_inventory_resource_converged<W: Write>(
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
    if super::inventory_convergence::observations_converged(inventory, resource) {
        return Ok(true);
    }
    writeln!(
        writer,
        "{operation} blocked by inventory/resource observation drift. No request was sent."
    )
    .map_err(io_error)?;
    Ok(false)
}

async fn retry_prompt<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(pending) = state.pending_prompt.clone() else {
        return Ok(());
    };
    let result = client
        .append_prompt_receipt(
            &pending.conversation_id,
            pending.expected_version,
            &pending.content,
            &pending.idempotency_key,
        )
        .await;
    match result {
        Ok(result) => report_prompt_accepted(client, state, &pending, &result, writer).await?,
        Err(error) => {
            handle_prompt_error(client, state, &pending, &error, writer).await?;
        }
    }
    Ok(())
}

async fn report_prompt_accepted<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingPrompt,
    result: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
    state.pending_prompt = None;
    let Some(receipt) = result.get("receipt").and_then(Value::as_object) else {
        state.selected_id = None;
        state.selected_entry = None;
        state.clear_prompt_history();
        state.clear_run_timeline();
        writeln!(
            writer,
            "Prompt response omitted the content-free receipt; refresh before sending another prompt."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    let Some(version) = receipt.get("aggregate_version").and_then(Value::as_u64) else {
        state.selected_id = None;
        state.selected_entry = None;
        state.clear_prompt_history();
        state.clear_run_timeline();
        writeln!(
            writer,
            "Prompt response omitted the new version; refresh before sending another prompt."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    let Some(replayed) = receipt.get("replayed").and_then(Value::as_bool) else {
        state.selected_id = None;
        state.selected_entry = None;
        state.clear_prompt_history();
        state.clear_run_timeline();
        writeln!(
            writer,
            "Prompt response omitted the replay marker; refresh before sending another prompt."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    update_prompt_version(state, &pending.conversation_id, version);
    if replayed {
        writeln!(
            writer,
            "Prompt retry replayed the existing message. No Run was started."
        )
        .map_err(io_error)?;
    } else {
        writeln!(writer, "Prompt stored. No Run was started.").map_err(io_error)?;
    }
    match super::load_session_history(client, &pending.conversation_id).await {
        Ok(page) => {
            state.record_prompt_history(&pending.conversation_id, &page);
            writeln!(writer, "Prompt history refreshed.").map_err(io_error)?;
        }
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            let dropped = if cleared {
                false
            } else {
                super::commands::drop_selected_session_after_read_rejection(
                    state,
                    &pending.conversation_id,
                    &error,
                )
            };
            writeln!(
                writer,
                "Prompt was stored, but history refresh failed: {error}. Use sync to refresh it."
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
        }
    }
    Ok(())
}

async fn retry_pending_run_intent<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(pending) = state.pending_run_intent.clone() else {
        return Ok(());
    };
    let result = client
        .submit_pending_run_intent(
            &pending.conversation_id,
            pending.expected_version,
            &pending.content,
            &pending.idempotency_key,
        )
        .await;
    match result {
        Ok(result) => {
            state.pending_run_intent = None;
            if let Some(version) = result
                .get("intent")
                .and_then(|intent| intent.get("aggregate_version"))
                .and_then(Value::as_u64)
            {
                update_prompt_version(state, &pending.conversation_id, version);
            }
            let intent_id = result
                .get("intent")
                .and_then(|intent| intent.get("intent_id"))
                .and_then(Value::as_str)
                .unwrap_or("(id unavailable)");
            writeln!(
                writer,
                "Pending Run-intent {} stored. No Run was started.",
                json_text(intent_id)
            )
            .map_err(io_error)?;
            match super::load_session_history(client, &pending.conversation_id).await {
                Ok(page) => {
                    state.record_prompt_history(&pending.conversation_id, &page);
                    writeln!(writer, "Prompt history refreshed.").map_err(io_error)?;
                }
                Err(error) => {
                    let cleared =
                        super::clear_session_view_after_authorization_error(state, &error);
                    let dropped = if cleared {
                        false
                    } else {
                        super::commands::drop_selected_session_after_read_rejection(
                            state,
                            &pending.conversation_id,
                            &error,
                        )
                    };
                    writeln!(
                        writer,
                        "Pending Run-intent was stored, but history refresh failed: {error}. Use sync to refresh it."
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
                }
            }
        }
        Err(error) => {
            if super::response_status(&error).is_some_and(|status| matches!(status, 401 | 403)) {
                state.pending_run_intent = None;
                state.clear_remote_session_view();
                writeln!(writer, "Pending Run-intent was rejected: {error}").map_err(io_error)?;
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            } else if super::response_status(&error) == Some(409) {
                state.pending_run_intent = None;
                writeln!(writer, "Pending Run-intent was rejected: {error}").map_err(io_error)?;
                refresh_after_conflict(
                    client,
                    state,
                    &pending.conversation_id,
                    "pending Run-intent",
                    writer,
                )
                .await?;
            } else if super::response_status(&error).is_some_and(super::definitive_client_rejection)
            {
                state.pending_run_intent = None;
                writeln!(writer, "Pending Run-intent was rejected: {error}").map_err(io_error)?;
            } else {
                writeln!(
                    writer,
                    "{error}. Enter retry to reuse the same version and idempotency key."
                )
                .map_err(io_error)?;
            }
        }
    }
    Ok(())
}

fn update_prompt_version(state: &mut TuiState, conversation_id: &str, version: u64) {
    if let Some(entry) = state
        .conversations
        .iter_mut()
        .chain(state.selected_entry.iter_mut())
        .find(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(conversation_id))
    {
        entry.aggregate_version = entry.aggregate_version.max(version);
    }
}

async fn handle_prompt_error<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingPrompt,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if super::response_status(error).is_some_and(|status| matches!(status, 401 | 403)) {
        state.pending_prompt = None;
        state.clear_remote_session_view();
        writeln!(writer, "Prompt was rejected: {error}").map_err(io_error)?;
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    match super::response_status(error) {
        Some(409) => {
            state.pending_prompt = None;
            writeln!(writer, "Prompt was rejected: {error}").map_err(io_error)?;
            refresh_after_conflict(client, state, &pending.conversation_id, "prompt", writer).await
        }
        Some(status) if super::definitive_client_rejection(status) => {
            state.pending_prompt = None;
            writeln!(writer, "Prompt was rejected: {error}").map_err(io_error)
        }
        _ => writeln!(
            writer,
            "{error}. Enter retry to reuse the same version and idempotency key."
        )
        .map_err(io_error),
    }
}

async fn refresh_after_conflict<W: Write>(
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
                super::clear_session_view_after_authorization_error(state, &refresh_error);
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

async fn retry_create<W: Write>(
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
        Err(error) => return report_create_projection_refresh_failure(state, error, writer),
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
    error: RemoteError,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let pending_create = state.pending_create.clone();
    let client_instance_filter = state.client_instance_filter.clone();
    let cleared = super::clear_session_view_after_authorization_error(state, &error);
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
            super::state::conversation_visible_to_selected_client_instance(state, &created)?;
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
        let cleared = super::clear_session_view_after_authorization_error(state, &error);
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
    if super::response_status(error).is_some_and(|status| matches!(status, 401 | 403)) {
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
    if super::response_status(error).is_some_and(super::definitive_client_rejection) {
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
