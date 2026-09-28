use std::io::Write;

use serde_json::Value;

use super::super::super::{RemoteClient, RemoteError};
use super::super::state::{PendingPrompt, TuiState, io_error, new_idempotency_key};
use super::history::{refresh_after_conflict, refresh_after_storage_write, update_prompt_version};
use super::projection::{
    ensure_inventory_resource_converged, ensure_prompt_visible_to_client_instance,
    refresh_explicit_inventory_resource_observations,
};
use super::{pending_write_blocks, retry_pending};

pub(in super::super) async fn send_new_prompt<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    content: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if pending_write_blocks(state, writer)? {
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
    if content.trim().is_empty() || content.len() > super::super::MAX_TUI_INPUT_BYTES {
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

pub(super) async fn retry_prompt<W: Write>(
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
    let Some((version, replayed)) = prompt_receipt_metadata(state, result, writer)? else {
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
    refresh_after_storage_write(client, state, &pending.conversation_id, "Prompt", writer).await?;
    Ok(())
}

fn prompt_receipt_metadata<W: Write>(
    state: &mut TuiState,
    result: &Value,
    writer: &mut W,
) -> Result<Option<(u64, bool)>, RemoteError> {
    let Some(receipt) = result.get("receipt").and_then(Value::as_object) else {
        report_missing_receipt_field(state, "content-free receipt", writer)?;
        return Ok(None);
    };
    let Some(version) = receipt.get("aggregate_version").and_then(Value::as_u64) else {
        report_missing_receipt_field(state, "new version", writer)?;
        return Ok(None);
    };
    let Some(replayed) = receipt.get("replayed").and_then(Value::as_bool) else {
        report_missing_receipt_field(state, "replay marker", writer)?;
        return Ok(None);
    };
    Ok(Some((version, replayed)))
}

fn report_missing_receipt_field<W: Write>(
    state: &mut TuiState,
    field: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    state.selected_id = None;
    state.selected_entry = None;
    state.clear_prompt_history();
    state.clear_run_timeline();
    writeln!(
        writer,
        "Prompt response omitted the {field}; refresh before sending another prompt."
    )
    .map_err(io_error)
}

async fn handle_prompt_error<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    pending: &PendingPrompt,
    error: &RemoteError,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if super::super::response_status(error).is_some_and(|status| matches!(status, 401 | 403)) {
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
    match super::super::response_status(error) {
        Some(409) => {
            state.pending_prompt = None;
            writeln!(writer, "Prompt was rejected: {error}").map_err(io_error)?;
            refresh_after_conflict(client, state, &pending.conversation_id, "prompt", writer).await
        }
        Some(status) if super::super::definitive_client_rejection(status) => {
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

pub(super) async fn retry_selected_prompt<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !refresh_explicit_inventory_resource_observations(client, state, "Prompt", writer).await? {
        return Ok(());
    }
    if !ensure_prompt_visible_to_client_instance(state, conversation_id, writer)? {
        return Ok(());
    }
    if !ensure_inventory_resource_converged(state, "Prompt retry", writer)? {
        return Ok(());
    }
    retry_prompt(client, state, writer).await
}
