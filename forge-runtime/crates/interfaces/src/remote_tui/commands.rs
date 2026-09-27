use std::{io::Write, path::Path};

use serde_json::Value;

use crate::args::{DeviceCommand, DevicePlacementCommand, parse_scope};
use crate::client_instance_session_scope;

use super::super::OwnedConversationEntry;
use super::state::{
    TuiState, io_error, json_text, new_idempotency_key, refresh_sessions, scope_filter_label,
    write_help,
};
use super::{RemoteClient, RemoteError, pending_run_intents, runs, state, sync, writes};

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

pub(super) async fn dispatch_command<W: Write>(
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
    match verb {
        "help" | "?" => write_help(writer)?,
        "filter" => filter_sessions(state, argument, writer)?,
        "instance" => instance_filter_command(state, argument, writer)?,
        "import" => {
            import_command::import_session(client, state, state_dir, argument, writer).await?
        }
        "changes" => changes_watch::changes_command(client, state, argument, writer).await?,
        "watch" => changes_watch::watch_command(client, state, argument, writer).await?,
        "sync" => sync::sync_command(client, state, writer).await?,
        "runs" => runs::runs_command(client, state, argument, writer).await?,
        "timeline" => runs::timeline_command(client, state, argument, writer).await?,
        "run-intents" => pending_run_intents::command(client, state, argument, writer).await?,
        "placement-preview" => placement_preview(client, state, argument, writer).await?,
        "placement-registry-preview" => {
            placement_registry_preview(client, state, argument, writer).await?
        }
        "scheduler-selection-preview" => {
            scheduler_selection_preview(client, state, argument, writer).await?
        }
        "scheduler-selection-lease" => {
            scheduler_selection_lease(client, state, argument, writer).await?
        }
        "scheduler-selection-lease-renew" => {
            scheduler_selection_lease_renew(client, state, argument, writer).await?
        }
        "scheduler-selection-lease-release" => {
            scheduler_selection_lease_release(client, state, argument, writer).await?
        }
        "attempt-request-preview" => super::attempt_request::preview(argument, writer)?,
        "pending-run-intent-preview" => super::pending_run_intent::preview(argument, writer)?,
        "session-observation-preview" => {
            super::session_observation::preview(client, state, argument, writer).await?
        }
        "runner-receipt-preview" => super::runner_receipt::preview(argument, writer)?,
        "runner-lease-fencing-preview" => super::runner_lease_fencing::preview(argument, writer)?,
        "execution-lease-checkpoint-preview" => {
            super::execution_lease_checkpoint::preview(argument, writer)?
        }
        "client-session-view-preview" => super::client_session_view::preview(argument, writer)?,
        "client-instance-resource-view-preview" => {
            super::client_instance_resource_view::preview(argument, writer)?
        }
        "credential-candidate-preview" => credential_candidate::preview(argument, writer)?,
        "runner-execution-intent-preview" => {
            super::runner_execution_intent::preview(argument, writer)?
        }
        "session-runner-receipt-preview" => {
            super::session_runner_receipt::preview(client, state, argument, writer).await?
        }
        "session-runner-receipt-history-preview" => {
            super::session_runner_receipt_history::preview(client, state, argument, writer).await?
        }
        "session-runner-reconciliation-preview" => {
            super::session_runner_reconciliation::preview(argument, writer)?
        }
        "session-runner-reconciliation-remote-preview" => {
            super::session_runner_reconciliation::remote_preview(client, state, argument, writer)
                .await?
        }
        "runner-execution-readiness-preview" => {
            super::local_runner_preview::preview(client, state, argument, writer).await?
        }
        "execution-consent-preview" => {
            super::execution_consent::preview(client, state, argument, writer).await?
        }
        "execution-reconciliation-preview" => {
            super::execution_reconciliation::preview(client, state, argument, writer).await?
        }
        "session-runner-receipt-offline-preview" => {
            super::session_runner_receipt::offline_preview(argument, writer)?
        }
        "session-runner-receipt-history-offline-preview" => {
            super::session_runner_receipt::offline_history_preview(argument, writer)?
        }
        "run-execution-evidence-preview" => {
            super::run_execution_evidence::preview(argument, writer)?
        }
        "run-execution-evidence-remote-preview" => {
            super::run_execution_evidence::remote_preview(client, state, argument, writer).await?
        }
        "run-attempt-lease-dispatch-preflight-preview" => {
            super::run_attempt_lease_dispatch_preflight::preview(argument, writer)?
        }
        "run-attempt-lease-dispatch-preflight-remote-preview" => {
            super::run_attempt_lease_dispatch_preflight::remote_preview(
                client, state, argument, writer,
            )
            .await?
        }
        "runner-dispatch-plan-preview" | "runner-dispatch-plan-remote-preview" => {
            super::runner_dispatch_plan_preview::remote_preview(client, state, argument, writer)
                .await?
        }
        "runner-attempt-boundary-preview" => {
            super::runner_attempt_boundary::preview(argument, writer)?
        }
        "runner-attempt-boundary-remote-preview" => {
            super::runner_attempt_boundary_remote::remote_preview(client, state, argument, writer)
                .await?
        }
        "runner-dispatch-admission-preview" | "runner-dispatch-admission-remote-preview" => {
            super::runner_dispatch_admission::remote_preview(client, state, argument, writer)
                .await?
        }
        "runner-transport-admission-preview" | "runner-transport-admission-remote-preview" => {
            super::runner_transport_admission::remote_preview(client, state, argument, writer)
                .await?
        }
        "runner-execution-boundary-preview" | "runner-execution-boundary-remote-preview" => {
            super::runner_execution_boundary::remote_preview(client, state, argument, writer)
                .await?
        }
        "runner-execution-intent-remote-preview" => {
            super::runner_execution_intent_remote::remote_preview(client, state, argument, writer)
                .await?
        }
        "run-observed" | "observed" => {
            super::run_observed::show(client, state, argument, writer).await?
        }
        "run-observed-preview" => super::run_observed::preview(argument, writer)?,
        "heartbeat-persistence-preview" => super::heartbeat_persistence::preview(argument, writer)?,
        "identity-proof-preview" => super::identity_proof::preview(argument, writer)?,
        "inventory" => inventory::show_inventory(client, state, argument, writer).await?,
        "lifecycle-registry" => lifecycle_registry::show(client, argument, writer).await?,
        "credential-candidate" => {
            credential_candidate::remote_preview(client, argument, writer).await?
        }
        "client-instances" => client_instance_views::show(client, state, argument, writer).await?,
        "run-intent-preview" => show_run_intent_preview(argument, writer)?,
        "older" | "history" => older_history(client, state, argument, writer).await?,
        "list" | "refresh" | "r" => refresh_command(client, state, false, writer).await?,
        "next" | "n" => refresh_command(client, state, true, writer).await?,
        "detail" | "show" => show_session_detail(client, state, argument, writer).await?,
        "open" | "o" => open_session(client, state, argument, writer).await?,
        "create" | "c" => writes::create_session(client, state, argument, writer).await?,
        "prompt" | "p" => writes::send_new_prompt(client, state, argument, writer).await?,
        "retry" => writes::retry_pending(client, state, writer).await?,
        "" => {}
        other => writeln!(writer, "Unknown command: {other}. Enter help for commands.")
            .map_err(io_error)?,
    }
    Ok(false)
}

async fn placement_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_placement_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_placement_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_placement_usage(writer);
    }
    let request = match super::super::placement::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Placement preview input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    let response = match client.preview_device_placement(&request).await {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Placement preview request failed: {error}").map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::placement::validate_response(&response, &request) {
        Ok(result) => super::super::placement::render_human(&result, writer).map_err(io_error)?,
        Err(error) => {
            writeln!(
                writer,
                "Placement preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

async fn placement_registry_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_placement_registry_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_placement_registry_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_placement_registry_usage(writer);
    }
    let request = match super::super::placement_registry::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Registry placement preview input failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    let response = match client.preview_device_placement_registry(&request).await {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Registry placement preview request failed: {error}")
                .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::placement_registry::validate_response(&response) {
        Ok(result) => {
            super::super::placement_registry::render_human(&result, writer).map_err(io_error)?
        }
        Err(error) => {
            writeln!(
                writer,
                "Registry placement preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

async fn scheduler_selection_preview<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_usage(writer);
    }
    let request = match super::super::scheduler_selection::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler selection preview input failed: {error}")
                .map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler selection preview",
        writer,
    )? {
        return Ok(());
    }
    // A selected instance with both owner-bound observations explicitly open
    // must not plan against a stale local image. This remains opt-in: without
    // that complete display pair, preserve the existing request-free path.
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Scheduler selection preview",
        writer,
    )
    .await?
    {
        return Ok(());
    }
    // The refresh may revoke or otherwise change the selected instance row.
    // Re-run the display-only membership fence after committing the new pair;
    // the pre-refresh check only decides whether it is safe to read the
    // opt-in inventory/resource candidates.
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler selection preview",
        writer,
    )? {
        return Ok(());
    }
    if !super::writes::ensure_inventory_resource_converged(
        state,
        "Scheduler selection preview",
        writer,
    )? {
        return Ok(());
    }
    let response = match client.preview_scheduler_selection(&request).await {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(
                writer,
                "Scheduler selection preview request failed: {error}"
            )
            .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::scheduler_selection::validate_response_for_request(&response, &request) {
        Ok(result) => {
            super::super::scheduler_selection::render_human(&result, writer).map_err(io_error)?
        }
        Err(error) => {
            writeln!(
                writer,
                "Scheduler selection preview response failed validation: {error}"
            )
            .map_err(io_error)?;
        }
    }
    Ok(())
}

async fn scheduler_selection_lease<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_lease_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_lease_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_lease_usage(writer);
    }
    let request = match super::super::scheduler_lease::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler lease input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler lease",
        writer,
    )? {
        return Ok(());
    }
    // An explicit selected instance with both inventory/resource observations
    // open must refresh the pair immediately before the candidate lease POST.
    // The first visibility check only fences the caller-supplied request; the
    // refresh can revoke that Conversation, so check the selected projection
    // again before the local convergence guard and request.
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Scheduler lease",
        writer,
    )
    .await?
    {
        return Ok(());
    }
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler lease",
        writer,
    )? {
        return Ok(());
    }
    if !super::writes::ensure_inventory_resource_converged(state, "Scheduler lease", writer)? {
        return Ok(());
    }
    let key = new_idempotency_key()?;
    let response = match client.claim_scheduler_selection_lease(&request, &key).await {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Scheduler lease request failed: {error}").map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::scheduler_lease::validate_response_for_request(&response, &request) {
        Ok(result) => {
            super::super::scheduler_lease::render_human(&result, writer).map_err(io_error)?
        }
        Err(error) => writeln!(
            writer,
            "Scheduler lease response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

async fn scheduler_selection_lease_renew<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_lease_renewal_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_lease_renewal_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_lease_renewal_usage(writer);
    }
    let request = match super::super::scheduler_lease_renew::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler lease renewal input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler lease renewal",
        writer,
    )? {
        return Ok(());
    }
    // Keep renewal behind the same explicit inventory/resource freshness
    // boundary as claim. A refresh may revoke the selected Conversation, so
    // repeat the display-only visibility fence before the candidate POST.
    if !super::writes::refresh_explicit_inventory_resource_observations(
        client,
        state,
        "Scheduler lease renewal",
        writer,
    )
    .await?
    {
        return Ok(());
    }
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler lease renewal",
        writer,
    )? {
        return Ok(());
    }
    if !super::writes::ensure_inventory_resource_converged(
        state,
        "Scheduler lease renewal",
        writer,
    )? {
        return Ok(());
    }
    let key = new_idempotency_key()?;
    let response = match client.renew_scheduler_selection_lease(&request, &key).await {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Scheduler lease renewal request failed: {error}")
                .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::scheduler_lease_renew::validate_response_for_request(&response, &request) {
        Ok(()) => super::super::scheduler_lease_renew::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Scheduler lease renewal response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

async fn scheduler_selection_lease_release<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let Some(input) = argument.strip_prefix("--input") else {
        return write_scheduler_selection_lease_release_usage(writer);
    };
    if !input.chars().next().is_some_and(char::is_whitespace) {
        return write_scheduler_selection_lease_release_usage(writer);
    }
    let input = input.trim();
    if input.is_empty() || input == "-" {
        return write_scheduler_selection_lease_release_usage(writer);
    }
    let request = match super::super::scheduler_lease_release::read_tui_request(input) {
        Ok(request) => request,
        Err(error) => {
            writeln!(writer, "Scheduler lease release input failed: {error}").map_err(io_error)?;
            return Ok(());
        }
    };
    if !ensure_scheduler_request_visible_to_selected_instance(
        state,
        &request,
        "Scheduler lease release",
        writer,
    )? {
        return Ok(());
    }
    let key = new_idempotency_key()?;
    let response = match client
        .release_scheduler_selection_lease(&request, &key)
        .await
    {
        Ok(response) => response,
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Scheduler lease release request failed: {error}")
                .map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    match super::super::scheduler_lease_release::validate_response_for_request(&response, &request)
    {
        Ok(()) => super::super::scheduler_lease_release::render_human(&response, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(
            writer,
            "Scheduler lease release response failed validation: {error}"
        )
        .map_err(io_error)?,
    }
    Ok(())
}

fn write_placement_registry_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use placement-registry-preview --input FILE. The request contains only placement requirements; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn write_scheduler_selection_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-preview --input FILE. An active client-instance filter must declare the request Conversation; the request binds Conversation, Run, Attempt, and requirements; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

/// Keeps scheduler previews and lease lifecycle requests inside an active
/// client-instance display projection. Without an explicit instance filter,
/// the existing caller-supplied request behavior remains unchanged; once a
/// filter is active, a stale or foreign placement file cannot cross it.
fn ensure_scheduler_request_visible_to_selected_instance<W: Write>(
    state: &TuiState,
    request: &Value,
    operation: &str,
    writer: &mut W,
) -> Result<bool, RemoteError> {
    let Some(conversation_id) = request.get("conversation_id").and_then(Value::as_str) else {
        writeln!(
            writer,
            "{operation} input is missing conversation_id; no request was sent."
        )
        .map_err(io_error)?;
        return Ok(false);
    };
    if state.client_instance_filter.is_none() {
        return Ok(true);
    }
    ensure_conversation_visible_to_client_instance(state, conversation_id, writer)
}

fn write_scheduler_selection_lease_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-lease --input FILE. This creates one fenced lease and uses a fresh idempotency key; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn write_scheduler_selection_lease_renewal_usage<W: Write>(
    writer: &mut W,
) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-lease-renew --input FILE. This renews one current fenced lease with a fresh idempotency key; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn write_scheduler_selection_lease_release_usage<W: Write>(
    writer: &mut W,
) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use scheduler-selection-lease-release --input FILE. This releases one current fenced lease with a fresh idempotency key; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn write_placement_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use placement-preview --input FILE. The file is a caller-supplied offline declaration; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

/// Renders two bounded local contract files without consuming the interactive
/// input stream or contacting a device endpoint.
fn show_run_intent_preview<W: Write>(argument: &str, writer: &mut W) -> Result<(), RemoteError> {
    let Some(suffix) = argument.strip_prefix("--input") else {
        return write_run_intent_usage(writer);
    };
    if !suffix.chars().next().is_some_and(char::is_whitespace) {
        return write_run_intent_usage(writer);
    }
    let Some((input, placement_input)) = suffix.trim().split_once(" --placement-input ") else {
        return write_run_intent_usage(writer);
    };
    let input = input.trim();
    let placement_input = placement_input.trim();
    if input.is_empty() || placement_input.is_empty() || input == "-" || placement_input == "-" {
        return write_run_intent_usage(writer);
    }
    let command = DeviceCommand::Placement(DevicePlacementCommand::RunIntentPreview {
        input: input.to_owned(),
        placement_input: placement_input.to_owned(),
    });
    match crate::device_run_intent_command::execute(&command) {
        Ok(output) => crate::device_run_intent_command::write_output(&output, false, writer)
            .map_err(io_error)?,
        Err(error) => writeln!(writer, "Run-intent preview failed: {error}").map_err(io_error)?,
    }
    Ok(())
}

fn write_run_intent_usage<W: Write>(writer: &mut W) -> Result<(), RemoteError> {
    writeln!(
        writer,
        "Use run-intent-preview --input RUN_FILE --placement-input SESSION_FILE. Both inputs are offline files; '-' is reserved for the standalone CLI."
    )
    .map_err(io_error)
}

fn filter_sessions<W: Write>(
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let selector = argument.trim();
    if selector == "clear" {
        let client_instance_filter_changed = state.client_instance_filter.is_some();
        state.scope_filter = None;
        state.client_instance_filter = None;
        if client_instance_filter_changed {
            state.clear_client_instance_private_projection();
        }
        writeln!(writer, "Scope filter cleared.").map_err(io_error)?;
        return Ok(());
    }
    if let Some(instance_id) = selector
        .strip_prefix("instance:")
        .or_else(|| selector.strip_prefix("client-instance:"))
    {
        let instance_id = instance_id.trim();
        if client_instance_session_scope::validate_instance_id(instance_id).is_err() {
            writeln!(
                writer,
                "Invalid client-instance filter. Use filter instance:INSTANCE_ID after opening client-instances session-view or resource-view."
            )
            .map_err(io_error)?;
            return Ok(());
        }
        let Some(view) = state.active_client_instance_view() else {
            writeln!(
                writer,
                "Open client-instances session-view or resource-view before setting an instance filter."
            )
            .map_err(io_error)?;
            return Ok(());
        };
        if client_instance_session_scope::scope_from_view(view, instance_id).is_err() {
            writeln!(
                writer,
                "Unknown client-instance filter {instance_id:?}; use an instance declared by the observed view."
            )
            .map_err(io_error)?;
            return Ok(());
        }
        let client_instance_filter_changed =
            state.client_instance_filter.as_deref() != Some(instance_id);
        state.scope_filter = None;
        state.client_instance_filter = Some(instance_id.to_owned());
        if client_instance_filter_changed {
            state.clear_client_instance_private_projection();
        }
        state.reconcile_client_instance_selection();
        writeln!(
            writer,
            "Client-instance filter set to {instance_id:?}; this is a local display projection over unverified session_ids, not authorization or device identity."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Ok(scope_filter) = parse_scope(selector) else {
        writeln!(
            writer,
            "Invalid scope filter. Use filter global|project:ID|group:ID or filter clear."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    let label = scope_filter_label(&scope_filter);
    let client_instance_filter_changed = state.client_instance_filter.is_some();
    state.scope_filter = Some(scope_filter);
    state.client_instance_filter = None;
    if client_instance_filter_changed {
        state.clear_client_instance_private_projection();
    }
    writeln!(
        writer,
        "Scope filter set to {label}; this only organizes the displayed session list, not authorization or device identity."
    )
    .map_err(io_error)
}

fn instance_filter_command<W: Write>(
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    let argument = argument.trim();
    if argument == "list" {
        let Some(view) = state.active_client_instance_view() else {
            writeln!(
                writer,
                "No client-instance view is open. Use client-instances session-view or resource-view first."
            )
            .map_err(io_error)?;
            return Ok(());
        };
        let active = state.client_instance_filter.as_deref();
        let mut count = 0usize;
        for instance_id in client_instance_session_scope::declared_instance_ids(view) {
            count += 1;
            let marker = if active == Some(instance_id) {
                "*"
            } else {
                " "
            };
            writeln!(writer, " {marker} {instance_id}").map_err(io_error)?;
        }
        if count == 0 {
            writeln!(
                writer,
                "The open client-instance view declares no instances."
            )
            .map_err(io_error)?;
        }
        return Ok(());
    }
    if argument == "clear" {
        return filter_sessions(state, "clear", writer);
    }
    if argument.is_empty() {
        writeln!(
            writer,
            "Use instance INSTANCE_ID, instance list, or instance clear after opening a client-instance view."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let selector = if argument.starts_with("instance:") || argument.starts_with("client-instance:")
    {
        argument.to_owned()
    } else {
        format!("instance:{argument}")
    };
    filter_sessions(state, &selector, writer)
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

async fn refresh_command<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    next_page: bool,
    writer: &mut W,
) -> Result<(), RemoteError> {
    match refresh_sessions(client, state, next_page).await {
        Ok(()) if next_page => Ok(()),
        Ok(()) => writeln!(writer, "Sessions refreshed.").map_err(io_error),
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            let action = if next_page {
                "Next page failed"
            } else {
                "Refresh failed"
            };
            writeln!(writer, "{action}: {error}").map_err(io_error)?;
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

async fn open_session<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !ensure_conversation_visible_to_client_instance(state, conversation_id, writer)? {
        return Ok(());
    }
    if let Err(error) = resolve_session_entry(client, state, conversation_id).await {
        let cleared = super::clear_session_view_after_authorization_error(state, &error);
        writeln!(writer, "Conversation detail request failed: {error}").map_err(io_error)?;
        if cleared {
            writeln!(
                writer,
                "Local session view cleared after authorization failure."
            )
            .map_err(io_error)?;
        }
        return Ok(());
    }
    if state.selected_id.as_deref() != Some(conversation_id) {
        state.clear_prompt_history();
        state.clear_run_timeline();
    }
    state.selected_id = Some(conversation_id.to_owned());
    match super::load_session_history(client, conversation_id).await {
        Ok(prompts) => {
            state.record_prompt_history(conversation_id, &prompts);
            writeln!(
                writer,
                "Opened session {} and refreshed Prompt history.",
                json_text(conversation_id)
            )
            .map_err(io_error)?;
        }
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            let dropped = if cleared {
                false
            } else {
                drop_selected_session_after_read_rejection(state, conversation_id, &error)
            };
            writeln!(writer, "History request failed: {error}").map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            } else if dropped {
                writeln!(
                    writer,
                    "Selected session was removed after its owner history read was rejected."
                )
                .map_err(io_error)?;
            }
        }
    }
    Ok(())
}

async fn show_session_detail<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !ensure_conversation_visible_to_client_instance(state, conversation_id, writer)? {
        return Ok(());
    }
    let detail = match resolve_session_entry(client, state, conversation_id).await {
        Ok(Some(detail)) => detail,
        Ok(None) => match client.get_conversation(conversation_id).await {
            Ok(detail) => detail,
            Err(error) => {
                let cleared = super::clear_session_view_after_authorization_error(state, &error);
                writeln!(writer, "Conversation detail request failed: {error}")
                    .map_err(io_error)?;
                if cleared {
                    writeln!(
                        writer,
                        "Local session view cleared after authorization failure."
                    )
                    .map_err(io_error)?;
                }
                return Ok(());
            }
        },
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            writeln!(writer, "Conversation detail request failed: {error}").map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            }
            return Ok(());
        }
    };
    if state.selected_id.as_deref() != Some(conversation_id) {
        state.clear_run_timeline();
    }
    state.selected_id = Some(conversation_id.to_owned());
    state.selected_entry = Some(detail.clone());
    writeln!(
        writer,
        "Conversation detail: {}",
        serde_json::to_string(&detail).unwrap_or_default()
    )
    .map_err(io_error)?;
    Ok(())
}

/// Returns a fetched detail only when the requested session is outside the
/// loaded page. The caller can keep the existing fast path for loaded rows.
async fn resolve_session_entry(
    client: &RemoteClient,
    state: &mut TuiState,
    conversation_id: &str,
) -> Result<Option<OwnedConversationEntry>, RemoteError> {
    if state
        .conversations
        .iter()
        .chain(state.selected_entry.iter())
        .any(|entry| entry.conversation.get("id").and_then(Value::as_str) == Some(conversation_id))
    {
        return Ok(None);
    }
    let detail = client.get_conversation(conversation_id).await?;
    state.selected_entry = Some(detail.clone());
    Ok(Some(detail))
}

async fn older_history<W: Write>(
    client: &RemoteClient,
    state: &mut TuiState,
    argument: &str,
    writer: &mut W,
) -> Result<(), RemoteError> {
    if !argument.trim().is_empty() {
        writeln!(
            writer,
            "Use older with no arguments to load the next Prompt page."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(conversation_id) = state.selected_id.clone() else {
        writeln!(
            writer,
            "Open a session before loading older Prompt history."
        )
        .map_err(io_error)?;
        return Ok(());
    };
    if !ensure_conversation_visible_to_client_instance(state, &conversation_id, writer)? {
        return Ok(());
    }
    if state.history_loaded_for.as_deref() != Some(conversation_id.as_str()) {
        writeln!(
            writer,
            "Open the selected session before loading older Prompt history."
        )
        .map_err(io_error)?;
        return Ok(());
    }
    let Some(before) = state.history_before.clone() else {
        writeln!(writer, "No older Prompt history is available.").map_err(io_error)?;
        return Ok(());
    };
    match client.list_prompts(&conversation_id, Some(&before)).await {
        Ok(prompts) => {
            state.record_prompt_history(&conversation_id, &prompts);
            writeln!(writer, "Older Prompt history loaded.").map_err(io_error)?;
        }
        Err(error) => {
            let cleared = super::clear_session_view_after_authorization_error(state, &error);
            let dropped = if cleared {
                false
            } else {
                drop_selected_session_after_read_rejection(state, &conversation_id, &error)
            };
            writeln!(writer, "Older history request failed: {error}").map_err(io_error)?;
            if cleared {
                writeln!(
                    writer,
                    "Local session view cleared after authorization failure."
                )
                .map_err(io_error)?;
            } else if dropped {
                writeln!(
                    writer,
                    "Selected session was removed after its owner history read was rejected."
                )
                .map_err(io_error)?;
            }
        }
    }
    Ok(())
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
