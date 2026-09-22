use std::{
    io::{self, BufRead, IsTerminal, Write},
    path::Path,
};

use serde_json::Value;

#[cfg(test)]
use super::OwnedConversationEntry;
use super::{RemoteClient, RemoteError};

#[path = "remote_tui/attempt_request.rs"]
mod attempt_request;
#[path = "remote_tui/client_instance_resource_view.rs"]
mod client_instance_resource_view;
#[path = "remote_tui/client_session_view.rs"]
mod client_session_view;
#[path = "remote_tui/commands.rs"]
mod commands;
#[path = "remote_tui/execution_consent.rs"]
mod execution_consent;
#[path = "remote_tui/execution_lease_checkpoint.rs"]
mod execution_lease_checkpoint;
#[path = "remote_tui/execution_reconciliation.rs"]
mod execution_reconciliation;
#[path = "remote_tui/heartbeat_persistence.rs"]
mod heartbeat_persistence;
#[path = "remote_tui/identity_proof.rs"]
mod identity_proof;
#[path = "remote_tui/input.rs"]
mod input;
#[path = "remote_tui/local_runner_preview.rs"]
mod local_runner_preview;
#[path = "remote_tui/pending_run_intent.rs"]
mod pending_run_intent;
#[path = "remote_tui/pending_run_intents.rs"]
mod pending_run_intents;
#[path = "remote_tui/run_attempt_lease_dispatch_preflight.rs"]
mod run_attempt_lease_dispatch_preflight;
#[path = "remote_tui/run_execution_evidence.rs"]
mod run_execution_evidence;
#[path = "remote_tui/run_observed.rs"]
mod run_observed;
#[path = "remote_tui/runner_dispatch_plan_preview.rs"]
mod runner_dispatch_plan_preview;
#[path = "remote_tui/runner_execution_intent.rs"]
mod runner_execution_intent;
#[path = "remote_tui/runner_lease_fencing.rs"]
mod runner_lease_fencing;
#[path = "remote_tui/runner_receipt.rs"]
mod runner_receipt;
#[path = "remote_tui/runs.rs"]
mod runs;
#[path = "remote_tui/session_observation.rs"]
mod session_observation;
#[path = "remote_tui/session_runner_receipt.rs"]
mod session_runner_receipt;
#[path = "remote_tui/state.rs"]
mod state;
#[path = "remote_tui/sync.rs"]
mod sync;
#[path = "remote_tui/writes.rs"]
mod writes;

const MAX_TUI_INPUT_BYTES: usize = 256 * 1024;
const MAX_TUI_INPUT_READ_BYTES: u64 = 256 * 1024 + 1;

/// Starts a TUI after confirming both process streams are interactive terminals.
///
/// # Errors
///
/// Returns an error when terminal access, configuration, or the remote API fails.
pub(super) async fn run(state_dir: Option<&Path>) -> Result<(), RemoteError> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(RemoteError(
            "remote tui requires an interactive terminal".into(),
        ));
    }
    let client = RemoteClient::from_env().await?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    if state_dir.is_none() {
        run_with_io(&client, &mut stdin.lock(), &mut stdout.lock()).await
    } else {
        run_with_io_and_state_dir(&client, state_dir, &mut stdin.lock(), &mut stdout.lock()).await
    }
}

async fn run_with_io<R: BufRead, W: Write>(
    client: &RemoteClient,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), RemoteError> {
    run_with_io_and_state_dir(client, None, reader, writer).await
}

async fn run_with_io_and_state_dir<R: BufRead, W: Write>(
    client: &RemoteClient,
    state_dir: Option<&Path>,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), RemoteError> {
    input::run_with_io(client, state_dir, reader, writer).await
}

async fn load_session_history(
    client: &RemoteClient,
    conversation_id: &str,
) -> Result<Value, RemoteError> {
    client.list_prompts(conversation_id, None).await
}

fn response_status(error: &RemoteError) -> Option<u16> {
    error
        .0
        .strip_prefix("Forge API returned HTTP ")
        .and_then(|value| value.get(..3))
        .and_then(|value| value.parse::<u16>().ok())
}

fn definitive_client_rejection(status: u16) -> bool {
    (400..500).contains(&status) && !matches!(status, 408 | 425 | 429)
}

fn clear_session_view_after_authorization_error(
    state: &mut state::TuiState,
    error: &RemoteError,
) -> bool {
    if response_status(error).is_some_and(|status| matches!(status, 401 | 403)) {
        state.clear_remote_session_view();
        true
    } else {
        false
    }
}

/// Clears a stale client-instance candidate after its explicit reader fails.
/// Owner authorization failures invalidate the complete owner view; other
/// failures revoke only the affected candidate while retaining the active
/// local instance filter so the projection remains empty until a new reader
/// succeeds.
fn clear_client_instance_view_after_failure(
    state: &mut state::TuiState,
    kind: &str,
    error: &RemoteError,
) -> (bool, bool) {
    if clear_session_view_after_authorization_error(state, error) {
        return (true, false);
    }
    (false, state.clear_client_instance_view(kind))
}

#[cfg(test)]
#[path = "remote_tui_tests.rs"]
mod tests;
