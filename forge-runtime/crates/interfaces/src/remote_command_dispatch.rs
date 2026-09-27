use std::error::Error;

use serde_json::Value;

use crate::args::RemoteCommand;
use crate::client_instance_session_scope;

use super::{RemoteClient, RemoteError, required_idempotency_key};

#[path = "remote_command_dispatch/changes.rs"]
mod changes;
use changes::execute_changes_command;
#[cfg(test)]
use changes::project_change_feed_response;
#[path = "remote_command_dispatch/sessions.rs"]
mod sessions;
use sessions::execute_session_command;
#[path = "remote_command_dispatch/runs.rs"]
mod runs;
use runs::execute_run_command;
#[path = "remote_command_dispatch/pending_intents.rs"]
mod pending_intents;
use pending_intents::execute_pending_run_intent_command;
#[path = "remote_command_dispatch/prompts.rs"]
mod prompts;
use prompts::{ensure_online_inventory_resource_convergence, execute_prompt_command};
#[path = "remote_command_dispatch/consent.rs"]
mod consent;
use consent::execute_execution_consent_preview;
#[path = "remote_command_dispatch/scheduler.rs"]
mod scheduler;
use scheduler::{
    execute_scheduler_selection_lease, execute_scheduler_selection_lease_release,
    execute_scheduler_selection_lease_renewal, execute_scheduler_selection_preview,
};
#[path = "remote_command_dispatch/runner_projection.rs"]
mod runner_projection;
use runner_projection::{
    ensure_runner_admission_instance_projection, ensure_runner_metadata_instance_projection,
};

#[path = "remote_command_dispatch/runner_boundaries.rs"]
mod runner_boundaries;

#[path = "remote_command_dispatch/execution_previews.rs"]
mod execution_previews;
#[path = "remote_command_dispatch/placement_previews.rs"]
mod placement_previews;
#[path = "remote_command_dispatch/preview_routes.rs"]
mod preview_routes;
#[path = "remote_command_dispatch/runner_metadata.rs"]
mod runner_metadata;
#[path = "remote_command_dispatch/scheduler_routes.rs"]
mod scheduler_routes;
#[path = "remote_command_dispatch/session_evidence.rs"]
mod session_evidence;

pub(crate) async fn execute(
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    execute_inner(command, idempotency_key, None).await
}

pub(crate) async fn execute_with_resolved_prompt(
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    content: &str,
) -> Result<Value, Box<dyn Error>> {
    execute_inner(command, idempotency_key, Some(content)).await
}

async fn execute_inner(
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    if matches!(command, RemoteCommand::Login) {
        return Err(RemoteError("use the remote login entry point".into()).into());
    }
    if let RemoteCommand::SessionRunnerReconciliationPreview { input } = command {
        return Ok(super::session_runner_reconciliation::read_value(input)?);
    }
    let client = RemoteClient::from_env().await?;
    execute_remote(&client, command, idempotency_key, resolved_prompt).await
}

async fn execute_remote(
    client: &RemoteClient,
    command: &RemoteCommand,
    idempotency_key: Option<&str>,
    resolved_prompt: Option<&str>,
) -> Result<Value, Box<dyn Error>> {
    match command {
        RemoteCommand::SessionsList { .. }
        | RemoteCommand::SessionsShow { .. }
        | RemoteCommand::SessionsCreate { .. }
        | RemoteCommand::SessionsImport { .. } => {
            execute_session_command(client, command, idempotency_key).await
        }
        RemoteCommand::RunsList { .. }
        | RemoteCommand::RunObserved { .. }
        | RemoteCommand::RunTimeline { .. } => execute_run_command(client, command).await,
        RemoteCommand::PendingRunIntentsList { .. }
        | RemoteCommand::PendingRunIntentSubmit { .. }
        | RemoteCommand::PendingRunIntentTimeline { .. } => {
            execute_pending_run_intent_command(client, command, idempotency_key, resolved_prompt)
                .await
        }
        RemoteCommand::PromptsList { .. }
        | RemoteCommand::PromptsAdd { .. }
        | RemoteCommand::PromptsReceipt { .. } => {
            execute_prompt_command(client, command, idempotency_key, resolved_prompt).await
        }
        RemoteCommand::ChangesList { .. }
        | RemoteCommand::ChangesWatch { .. }
        | RemoteCommand::ChangesStream { .. } => execute_changes_command(client, command).await,
        _ => preview_routes::execute(client, command, idempotency_key).await,
    }
}

#[cfg(test)]
#[path = "remote_command_dispatch_tests.rs"]
mod tests;
