use std::io::Write;

use serde_json::Value;

use super::super::super::{RemoteClient, RemoteError};
use super::super::state::{PendingRunIntent, TuiState, io_error, json_text, new_idempotency_key};
use super::history::{refresh_after_conflict, refresh_after_storage_write, update_prompt_version};
use super::projection::{
    ensure_inventory_resource_converged, ensure_pending_run_intent_visible_to_client_instance,
    refresh_explicit_inventory_resource_observations,
};
use super::{pending_write_blocks, retry_pending};

pub(in super::super) async fn send_new_pending_run_intent<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    content: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if pending_write_blocks(state, writer)? {
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
    if content.trim().is_empty() || content.len() > super::super::MAX_TUI_INPUT_BYTES {
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

pub(super) async fn retry_pending_run_intent<W: Write>(
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
            report_pending_run_intent_accepted(client, state, &pending, &result, writer).await?;
        }
        Err(error) => {
            handle_pending_run_intent_error(client, state, &pending, &error, writer).await?;
        }
    }
    Ok(())
}

pub(super) async fn retry_selected_pending_run_intent<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
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
    if !ensure_pending_run_intent_visible_to_client_instance(state, conversation_id, writer)? {
        return Ok(());
    }
    if !ensure_inventory_resource_converged(state, "Pending Run-intent retry", writer)? {
        return Ok(());
    }
    retry_pending_run_intent(client, state, writer).await
}

async fn report_pending_run_intent_accepted<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingRunIntent,
    result: &Value,
    writer: &mut W,
) -> Result<(), RemoteError> {
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
    refresh_after_storage_write(
        client,
        state,
        &pending.conversation_id,
        "Pending Run-intent",
        writer,
    )
    .await?;
    Ok(())
}

async fn handle_pending_run_intent_error<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingRunIntent,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if super::super::response_status(error).is_some_and(|status| matches!(status, 401 | 403)) {
        state.pending_run_intent = None;
        state.clear_remote_session_view();
        writeln!(writer, "Pending Run-intent was rejected: {error}").map_err(io_error)?;
        writeln!(
            writer,
            "Local session view cleared after authorization failure."
        )
        .map_err(io_error)?;
    } else if super::super::response_status(error) == Some(409) {
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
    } else if super::super::response_status(error)
        .is_some_and(super::super::definitive_client_rejection)
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
    Ok(())
}
