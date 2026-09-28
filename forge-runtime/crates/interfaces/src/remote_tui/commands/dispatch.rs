use super::super::{pending_run_intents, runs, sync, writes};
use super::{
    RemoteClient, RemoteError, TuiState, changes_watch, client_instance_views,
    credential_candidate, filter_sessions, handle_exit, import_command, instance_filter_command,
    inventory, io_error, lifecycle_registry, older_history, open_session, placement_preview,
    placement_registry_preview, refresh_command, scheduler_selection_lease,
    scheduler_selection_lease_release, scheduler_selection_lease_renew,
    scheduler_selection_preview, show_run_intent_preview, show_session_detail, write_help,
};
use crate::remote_command::tui;
use std::{io::Write, path::Path};

pub(in super::super) async fn dispatch_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    state_dir: Option<&Path>,
    command: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let (verb, argument) = command
        .split_once(' ')
        .map_or((command, ""), |(verb, rest)| (verb, rest.trim_start()));
    if matches!(verb, "q" | "quit" | "exit") {
        return handle_exit(state, argument, writer);
    }
    if session_reads(state_dir, client, state, verb, argument, writer).await? {
        return Ok(false);
    }
    if session_actions(client, state, verb, argument, writer).await? {
        return Ok(false);
    }
    if scheduling_commands(client, state, verb, argument, writer).await? {
        return Ok(false);
    }
    if execution_previews(client, state, verb, argument, writer).await? {
        return Ok(false);
    }
    if observation_previews(client, state, verb, argument, writer).await? {
        return Ok(false);
    }
    if offline_execution(verb, argument, writer)? {
        return Ok(false);
    }
    if offline_observation(verb, argument, writer)? {
        return Ok(false);
    }
    if !verb.is_empty() {
        writeln!(writer, "Unknown command: {verb}. Enter help for commands.").map_err(io_error)?;
    }
    Ok(false)
}

async fn session_reads<W: Write>(
    state_dir: Option<&Path>,
    client: &RemoteClient,
    state: &mut TuiState,
    verb: &str,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    match verb {
        "help" | "?" => write_help(writer)?,
        "filter" => filter_sessions(state, argument, writer)?,
        "instance" => instance_filter_command(state, argument, writer)?,
        "import" => {
            import_command::import_session(client, state, state_dir, argument, writer).await?;
        }
        "changes" => changes_watch::changes_command(client, state, argument, writer).await?,
        "watch" => changes_watch::watch_command(client, state, argument, writer).await?,
        "sync" => sync::sync_command(client, state, writer).await?,
        "runs" => runs::runs_command(client, state, argument, writer).await?,
        "timeline" => runs::timeline_command(client, state, argument, writer).await?,
        "run-intents" => pending_run_intents::command(client, state, argument, writer).await?,
        "inventory" => inventory::show_inventory(client, state, argument, writer).await?,
        _ => return Ok(false),
    }
    Ok(true)
}

async fn session_actions<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    verb: &str,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    match verb {
        "lifecycle-registry" => lifecycle_registry::show(client, argument, writer).await?,
        "credential-candidate" => {
            credential_candidate::remote_preview(client, argument, writer).await?;
        }
        "client-instances" => client_instance_views::show(client, state, argument, writer).await?,
        "older" | "history" => older_history(client, state, argument, writer).await?,
        "list" | "refresh" | "r" => refresh_command(client, state, false, writer).await?,
        "next" | "n" => refresh_command(client, state, true, writer).await?,
        "detail" | "show" => show_session_detail(client, state, argument, writer).await?,
        "open" | "o" => open_session(client, state, argument, writer).await?,
        "create" | "c" => writes::create_session(client, state, argument, writer).await?,
        "prompt" | "p" => writes::send_new_prompt(client, state, argument, writer).await?,
        "retry" => writes::retry_pending(client, state, writer).await?,
        _ => return Ok(false),
    }
    Ok(true)
}

async fn scheduling_commands<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    verb: &str,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    match verb {
        "placement-preview" => placement_preview(client, state, argument, writer).await?,
        "placement-registry-preview" => {
            placement_registry_preview(client, state, argument, writer).await?;
        }
        "scheduler-selection-preview" => {
            scheduler_selection_preview(client, state, argument, writer).await?;
        }
        "scheduler-selection-lease" => {
            scheduler_selection_lease(client, state, argument, writer).await?;
        }
        "scheduler-selection-lease-renew" => {
            scheduler_selection_lease_renew(client, state, argument, writer).await?;
        }
        "scheduler-selection-lease-release" => {
            scheduler_selection_lease_release(client, state, argument, writer).await?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

async fn execution_previews<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    verb: &str,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    match verb {
        "runner-execution-readiness-preview" => {
            tui::local_runner_preview::preview(client, state, argument, writer).await?;
        }
        "execution-consent-preview" => {
            tui::execution_consent::preview(client, state, argument, writer).await?;
        }
        "execution-reconciliation-preview" => {
            tui::execution_reconciliation::preview(client, state, argument, writer).await?;
        }
        "run-attempt-lease-dispatch-preflight-remote-preview" => {
            tui::run_attempt_lease_dispatch_preflight::remote_preview(
                client, state, argument, writer,
            )
            .await?;
        }
        "runner-dispatch-plan-preview" | "runner-dispatch-plan-remote-preview" => {
            tui::runner_dispatch_plan_preview::remote_preview(client, state, argument, writer)
                .await?;
        }
        "runner-attempt-boundary-remote-preview" => {
            tui::runner_attempt_boundary_remote::remote_preview(client, state, argument, writer)
                .await?;
        }
        "runner-dispatch-admission-preview" | "runner-dispatch-admission-remote-preview" => {
            tui::runner_dispatch_admission::remote_preview(client, state, argument, writer).await?;
        }
        "runner-transport-admission-preview" | "runner-transport-admission-remote-preview" => {
            tui::runner_transport_admission::remote_preview(client, state, argument, writer)
                .await?;
        }
        "runner-execution-boundary-preview" | "runner-execution-boundary-remote-preview" => {
            tui::runner_execution_boundary::remote_preview(client, state, argument, writer).await?;
        }
        "runner-execution-intent-remote-preview" => {
            tui::runner_execution_intent_remote::remote_preview(client, state, argument, writer)
                .await?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

async fn observation_previews<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    verb: &str,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    match verb {
        "session-observation-preview" => {
            tui::session_observation::preview(client, state, argument, writer).await?;
        }
        "session-runner-receipt-preview" => {
            tui::session_runner_receipt::preview(client, state, argument, writer).await?;
        }
        "session-runner-receipt-history-preview" => {
            tui::session_runner_receipt_history::preview(client, state, argument, writer).await?;
        }
        "session-runner-reconciliation-remote-preview" => {
            tui::session_runner_reconciliation::remote_preview(client, state, argument, writer)
                .await?;
        }
        "run-execution-evidence-remote-preview" => {
            tui::run_execution_evidence::remote_preview(client, state, argument, writer).await?;
        }
        "run-observed" | "observed" => {
            tui::run_observed::show(client, state, argument, writer).await?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn offline_execution<W: Write>(
    verb: &str,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    match verb {
        "attempt-request-preview" => tui::attempt_request::preview(argument, writer)?,
        "runner-receipt-preview" => tui::runner_receipt::preview(argument, writer)?,
        "runner-lease-fencing-preview" => {
            tui::runner_lease_fencing::preview(argument, writer)?;
        }
        "execution-lease-checkpoint-preview" => {
            tui::execution_lease_checkpoint::preview(argument, writer)?;
        }
        "runner-execution-intent-preview" => {
            tui::runner_execution_intent::preview(argument, writer)?;
        }
        "run-execution-evidence-preview" => {
            tui::run_execution_evidence::preview(argument, writer)?;
        }
        "run-attempt-lease-dispatch-preflight-preview" => {
            tui::run_attempt_lease_dispatch_preflight::preview(argument, writer)?;
        }
        "runner-attempt-boundary-preview" => {
            tui::runner_attempt_boundary::preview(argument, writer)?;
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn offline_observation<W: Write>(
    verb: &str,
    argument: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    match verb {
        "pending-run-intent-preview" => {
            tui::pending_run_intent::preview(argument, writer)?;
        }
        "client-session-view-preview" => {
            tui::client_session_view::preview(argument, writer)?;
        }
        "client-instance-resource-view-preview" => {
            tui::client_instance_resource_view::preview(argument, writer)?;
        }
        "credential-candidate-preview" => credential_candidate::preview(argument, writer)?,
        "session-runner-reconciliation-preview" => {
            tui::session_runner_reconciliation::preview(argument, writer)?;
        }
        "session-runner-receipt-offline-preview" => {
            tui::session_runner_receipt::offline_preview(argument, writer)?;
        }
        "session-runner-receipt-history-offline-preview" => {
            tui::session_runner_receipt::offline_history_preview(argument, writer)?;
        }
        "run-observed-preview" => tui::run_observed::preview(argument, writer)?,
        "heartbeat-persistence-preview" => {
            tui::heartbeat_persistence::preview(argument, writer)?;
        }
        "identity-proof-preview" => tui::identity_proof::preview(argument, writer)?,
        "run-intent-preview" => show_run_intent_preview(argument, writer)?,
        _ => return Ok(false),
    }
    Ok(true)
}
