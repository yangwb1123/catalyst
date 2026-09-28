use std::io::Write;

use super::super::{RemoteClient, RemoteError};
use super::state::{TuiState, io_error};

#[path = "writes/create.rs"]
mod create;
#[path = "writes/history.rs"]
mod history;
#[path = "writes/pending_intents.rs"]
mod pending_intents;
#[path = "writes/projection.rs"]
mod projection;
#[path = "writes/prompts.rs"]
mod prompts;

pub(super) use create::create_session;
pub(super) use pending_intents::send_new_pending_run_intent;
pub(super) use projection::{
    ensure_inventory_resource_converged, ensure_pending_run_intent_visible_to_client_instance,
    refresh_explicit_inventory_resource_observations,
};
pub(super) use prompts::send_new_prompt;

use create::retry_create;

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
        return prompts::retry_selected_prompt(client, state, &conversation_id, writer).await;
    }
    if let Some(conversation_id) = state
        .pending_run_intent
        .as_ref()
        .map(|pending| pending.conversation_id.clone())
    {
        return pending_intents::retry_selected_pending_run_intent(
            client,
            state,
            &conversation_id,
            writer,
        )
        .await;
    }
    if state.pending_create.is_some() {
        return retry_create(client, state, writer).await;
    }
    writeln!(writer, "There is no unconfirmed write to retry.").map_err(io_error)?;
    Ok(())
}

fn pending_write_blocks<W: Write>(state: &TuiState, writer: &mut W) -> Result<bool, RemoteError> {
    if state.has_pending_write() {
        writeln!(
            writer,
            "Resolve the pending write with retry before starting another write."
        )
        .map_err(io_error)?;
        return Ok(true);
    }
    Ok(false)
}
